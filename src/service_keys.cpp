#include "service.h"
#include "service_util.h"
#include "key_policy.h"
#include <cwchar>
using Microsoft::WRL::ComPtr;
bool Service::wants(ITfContext* context, WPARAM vk, LPARAM flags) {
    refreshPreferences();
    if (!writable(context)) return false;
    if ((GetKeyState(VK_LWIN) | GetKeyState(VK_RWIN)) & 0x8000) return false;
    auto physical = vk == VK_SHIFT ? MapVirtualKeyW((UINT)((flags >> 16) & 255),MAPVK_VSC_TO_VK_EX) : (UINT)vk;
    if (preferences_.shiftToggle && shiftKey(vk) && (!preferences_.shiftSide || physical == (UINT)(preferences_.shiftSide == 1 ? VK_LSHIFT : VK_RSHIFT))
        && !(GetKeyState(VK_CONTROL) & 0x8000) && !(GetKeyState(VK_MENU) & 0x8000)) return true;
    if (preferences_.ctrlSpaceToggle && vk == VK_SPACE && (GetKeyState(VK_CONTROL) & 0x8000)
        && !(GetKeyState(VK_SHIFT) & 0x8000) && !(GetKeyState(VK_MENU) & 0x8000)) return true;
    if (preferences_.widthToggle && vk == VK_SPACE && (GetKeyState(VK_SHIFT) & 0x8000)
        && !(GetKeyState(VK_CONTROL) & 0x8000) && !(GetKeyState(VK_MENU) & 0x8000)) return true;
    if (fullWidth_ && !compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)
        && !(GetKeyState(VK_CONTROL) & 0x8000) && !(GetKeyState(VK_MENU) & 0x8000)) {
        ShanjieKey key{};
        return shanjie::translate(vk,flags,key) && (key.kind == 1 || key.kind == 2);
    }
    if (!enabled(context)) return false;
    ShanjieKey key{}; if (!shanjie::translate(vk,flags,key)) return false;
    if (!preferences_.capsLockEnglish) key.modifiers &= ~16u;
    if (special_.starts(key,preferences_)) return true;
    if (key.modifiers & 12) return false;
    if (key.modifiers & 2) {
        if (special_.active()) return false;
        return (preferences_.ctrlPunctuation && shanjie::windowsPunctuation(key))
            || (preferences_.ctrlBackslash && key.kind == 1 && key.ch == '\\' && key.modifiers == 2)
            || (preferences_.forgetCandidate && preferences_.learning && key.kind == 4 && key.modifiers == 3 && state_.selected >= 0 && !state_.candidates.empty());
    }
    if (associated_.handles(key)) return true;
    if (pinyin_.handles(key)) return true;
    if (special_.active() || special_.starts(key,preferences_)) return true;
    if (!state_.preedit.empty()) return true;
    return key.kind == 1 && (!(key.modifiers & 30) || (preferences_.capsLockEnglish && (key.modifiers & 16) && !(key.modifiers & 14)) || (preferences_.ctrlPunctuation && shanjie::windowsPunctuation(key)));
}
HRESULT Service::OnKeyDown(ITfContext* context, WPARAM vk, LPARAM flags, BOOL* eaten) {
    if (!shiftKey(vk)) { ++modeNotificationRevision_; modeIndicator_.hide(); }
    if (!shiftKey(vk) && shiftTap_) shiftUsed_ = true;
    *eaten = FALSE; if (!wants(context,vk,flags)) return S_OK;
    if (preferences_.shiftToggle && shiftKey(vk)) {
        if (!(flags & (1L << 30))) { shiftTap_ = true; shiftUsed_ = false; }
        *eaten = TRUE; return S_OK;
    }
    if (preferences_.ctrlSpaceToggle && vk == VK_SPACE && (GetKeyState(VK_CONTROL) & 0x8000)
        && !(GetKeyState(VK_SHIFT) & 0x8000) && !(GetKeyState(VK_MENU) & 0x8000)) {
        if (flags & (1L << 30)) { *eaten = TRUE; return S_OK; }
        HRESULT hr = toggleMode(context); *eaten = SUCCEEDED(hr); return hr;
    }
    if (preferences_.widthToggle && vk == VK_SPACE && (GetKeyState(VK_SHIFT) & 0x8000)
        && !(GetKeyState(VK_CONTROL) & 0x8000) && !(GetKeyState(VK_MENU) & 0x8000)) {
        if (flags & (1L << 30)) { *eaten = TRUE; return S_OK; }
        fullWidth_ = !fullWidth_; setInputMode(compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)); *eaten = TRUE; return S_OK;
    }
    if (FAILED(watch(context))) return S_OK;
    ShanjieKey key{}; shanjie::translate(vk,flags,key);
    if (!preferences_.capsLockEnglish) key.modifiers &= ~16u;
    bool conversionClosed = state_.candidates.empty() || (state_.prediction && state_.selected < 0);
    if (key.kind == 6 && conversionClosed && !associated_.active() && !special_.active()) {
        if (preferences_.escape == EscapeAction::Ignore) { *eaten = TRUE; return S_OK; }
        if (preferences_.escape == EscapeAction::Clear) {
            auto hr = request(context,[this,context](TfEditCookie cookie) {
                pinyin_.clear(); rawReading_.clear(); shanjie::Snapshot out;
                if (!engine_.reset(true,out)) return E_FAIL;
                out.handled = true; return apply(context,cookie,out);
            },TF_ES_SYNC | TF_ES_READWRITE);
            *eaten = SUCCEEDED(hr); return S_OK;
        }
    }
    if (preferences_.ctrlPunctuation && shanjie::windowsPunctuation(key)) {
        auto mark = shanjie::windowsPunctuationText(key);
        auto hr = request(context,[this,context,mark](TfEditCookie cookie) {
            shanjie::Snapshot converted;
            if (pinyin_.active()) {
                ShanjieKey space{}; space.kind = 2;
                if (!pinyin_.key(space,engine_,state_,converted,preferences_.alphanumeric,true)) return E_FAIL;
                if (pinyin_.active()) return apply(context,cookie,converted);
            }
            rawReading_.clear(); associated_.clear();
            shanjie::Snapshot out; if (!engine_.punctuation(mark,out)) return E_FAIL;
            out.commit = converted.commit + out.commit;
            return apply(context,cookie,out);
        },TF_ES_SYNC | TF_ES_READWRITE);
        *eaten = SUCCEEDED(hr); return S_OK;
    }
    if (associated_.handles(key)) {
        if (key.kind == 2 && (!preferences_.spacePages || (!preferences_.wrapPages && state_.first + 9 >= state_.total))) { *eaten = TRUE; return S_OK; }
        HRESULT hr = request(context,[this,context,key](TfEditCookie cookie) {
            shanjie::Snapshot out;
            if (key.kind == 1) { if (!pickAssociated(key.ch - '1',out)) return E_FAIL; }
            else out = associated_.key(key);
            return apply(context,cookie,out);
        },TF_ES_SYNC | TF_ES_READWRITE);
        *eaten = SUCCEEDED(hr); return S_OK;
    }
    if (associated_.active()) { associated_.clear(); associationCaret_.Reset(); state_.associated = false; state_.candidates.clear(); state_.columns = 0; candidates_->close(); }
    if (pinyin_.handles(key)) {
        HRESULT hr = request(context,[this,context,key](TfEditCookie cookie) {
            shanjie::Snapshot out;
            if (!pinyin_.key(key,engine_,state_,out,preferences_.alphanumeric,preferences_.pinyinSpaceConverts)) return E_FAIL;
            return apply(context,cookie,out);
        },TF_ES_SYNC | TF_ES_READWRITE);
        *eaten = SUCCEEDED(hr); return S_OK;
    }
    if (special_.active() || special_.starts(key,preferences_)) {
        HRESULT hr = request(context,[this,context,key](TfEditCookie cookie) {
            shanjie::Snapshot pending;
            if (!special_.active() && !engine_.reset(false,pending)) return E_FAIL;
            auto out = special_.key(key,preferences_); out.commit = pending.commit + out.commit;
            return apply(context,cookie,out);
        },TF_ES_SYNC | TF_ES_READWRITE);
        *eaten = SUCCEEDED(hr); return S_OK;
    }
    bool selectedCandidate = !state_.candidates.empty() && state_.selected >= 0 && key.modifiers == 0
        && (key.kind == 3 || (key.kind == 1 && key.ch >= '1' && key.ch <= '9'));
    key = shanjie::windowsKey(key,state_,preferences_);
    if (preferences_.forgetCandidate && preferences_.learning && key.kind == 4 && key.modifiers == 3 && state_.selected >= 0) key.modifiers = 8;
    bool capsEnglish = preferences_.capsLockEnglish && (key.modifiers & 16) && !(key.modifiers & 14) && (key.kind == 1 || key.kind == 2);
    bool shiftAscii = key.kind == 1 && key.modifiers == 1 && (!preferences_.shiftPunctuation || !std::wcschr(L",./1;[]90`\\'=",(wchar_t)key.ch));
    if ((fullWidth_ && !compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE)) || capsEnglish || shiftAscii) {
        BYTE keyboard[256]; GetKeyboardState(keyboard); WCHAR text[4]{};
        int length = ToUnicodeEx((UINT)vk,(UINT)((flags >> 16) & 255),keyboard,text,4,4,GetKeyboardLayout(0));
        if (length != 1 || text[0] < 0x20 || text[0] > 0x7e) return S_OK;
        wchar_t character = text[0];
        HRESULT hr = request(context,[this,context,character](TfEditCookie cookie) {
            shanjie::Snapshot out; if (!engine_.reset(false,out)) return E_FAIL;
            out.handled = true; out.commit += character; return apply(context,cookie,out);
        },TF_ES_SYNC | TF_ES_READWRITE);
        *eaten = SUCCEEDED(hr); return S_OK;
    }
    bool handled = false;
    if (!state_.prediction && key.kind == 2 && !state_.candidates.empty() && (!preferences_.spacePages
        || (!preferences_.wrapPages && state_.first + (unsigned)(std::max(0,state_.selected) / 9 + 1) * 9 >= state_.total))) { *eaten = TRUE; return S_OK; }
    bool deleteCandidate = preferences_.backspaceDeletes && !state_.prediction && !state_.candidates.empty() && key.kind == 4 && key.modifiers == 0;
    bool bypassOpen = key.modifiers == 0 && conversionClosed && !state_.preedit.empty() && !shanjie::hasPendingReading(state_)
        && ((key.kind == 2 && preferences_.trigger == CandidateTrigger::Down) || (key.kind == 10 && preferences_.trigger == CandidateTrigger::Space));
    bool cycle = preferences_.upCycles && key.kind == 9 && key.modifiers == 0 && conversionClosed && !state_.preedit.empty() && !shanjie::hasPendingReading(state_);
    HRESULT hr = request(context,[this,context,key,cycle,bypassOpen,selectedCandidate,deleteCandidate,&handled](TfEditCookie cookie) {
        if (state_.preedit.empty()) {
            TF_SELECTION selection{}; ULONG fetched = 0;
            if (SUCCEEDED(context->GetSelection(cookie,TF_DEFAULT_SELECTION,1,&selection,&fetched)) && fetched) {
                ComPtr<ITfRange> left; selection.range->Clone(&left); selection.range->Release();
                left->Collapse(cookie,TF_ANCHOR_START); LONG moved = 0; left->ShiftStart(cookie,-8,&moved,nullptr);
                WCHAR text[8]; ULONG count = 0;
                if (preferences_.useContext && SUCCEEDED(left->GetText(cookie,0,text,8,&count))) engine_.context(std::wstring(text,count));
            }
        }
        shanjie::Snapshot out;
        if (bypassOpen) {
            if (!engine_.reset(false,out)) return E_FAIL;
            out.handled = key.kind == 2 && !preferences_.spaceAfterCommit;
        } else if (cycle) {
            if (!engine_.cycleCandidate(out)) return E_FAIL;
        } else {
            auto raw = rawReading_;
            if (key.kind == 4 && !raw.empty()) raw.pop_back();
            if (key.kind == 1 && key.modifiers == 0) raw += (char)key.ch;
            else if (key.kind == 2 && key.modifiers == 0) raw += ' ';
            if (!engine_.key(key,out)) {
                if (!preferences_.alphanumeric || raw.empty() || !shanjie::hasPendingReading(state_)) return E_FAIL;
                out.handled = true; out.commit = state_.preedit;
                auto end = std::min<size_t>(state_.cursor,out.commit.size()), start = end;
                while (start > 0 && ((out.commit[start-1] >= 0x3105 && out.commit[start-1] <= 0x312f) || out.commit[start-1] == L'ˊ' || out.commit[start-1] == L'ˇ' || out.commit[start-1] == L'ˋ' || out.commit[start-1] == L'˙')) --start;
                out.commit.replace(start,end-start,shanjie::wide(raw.c_str()));
            }
            rawReading_ = shanjie::hasPendingReading(out) ? raw : std::string{};
        }
        if (deleteCandidate && !engine_.key(key,out)) return E_FAIL;
        if (preferences_.enterCommits && selectedCandidate && key.kind == 3 && key.modifiers == 0 && out.candidates.empty() && !shanjie::hasPendingReading(out) && !out.preedit.empty()) {
            auto committed = out.commit;
            if (!engine_.key(key,out)) return E_FAIL;
            out.commit = committed + out.commit;
        }
        // Enter finishes confirmed text; only number/mouse selection may prompt associated words.
        HRESULT result = apply(context,cookie,out,selectedCandidate && !state_.prediction && key.kind != 3); handled = SUCCEEDED(result) && out.handled; return result;
    },TF_ES_SYNC | TF_ES_READWRITE);
    if (FAILED(hr)) { cancel(); return S_OK; }
    *eaten = handled; return S_OK;
}
HRESULT Service::OnTestKeyDown(ITfContext* c, WPARAM k, LPARAM f, BOOL* out) { if (!shiftKey(k) && shiftTap_) shiftUsed_ = true; *out = wants(c,k,f); return S_OK; }
HRESULT Service::OnTestKeyUp(ITfContext*, WPARAM vk, LPARAM, BOOL* out) { *out = shiftKey(vk) && shiftTap_; return S_OK; }
HRESULT Service::OnKeyUp(ITfContext* context, WPARAM vk, LPARAM, BOOL* out) {
    *out = FALSE;
    if (shiftKey(vk) && shiftTap_) {
        bool toggle = !shiftUsed_; shiftTap_ = false; shiftUsed_ = false;
        *out = TRUE; if (toggle) return toggleMode(context);
    }
    return S_OK;
}
HRESULT Service::OnPreservedKey(ITfContext*, REFGUID, BOOL* out) { *out = FALSE; return S_OK; }
