#include "special_input.h"
#include "key_policy.h"
#include <algorithm>
#include <cwctype>
namespace {
const std::wstring symbols = L"，。？！；：、（）「」『』【】《》〈〉—…～·「」＋－×÷＝≠≤≥±∞√％‰℃℉°′″→←↑↓★☆○●□■△▲◎※〒＄€£¥";
}
bool SpecialInput::starts(const ShanjieKey& key,const Preferences& p) const {
    return key.kind == 1 && ((key.ch == '`' && key.modifiers == 0 && (p.backquoteSymbols || p.codeInput)) || (p.symbolKeyboard && key.ch == ',' && key.modifiers == 6));
}
shanjie::Snapshot SpecialInput::view() const {
    shanjie::Snapshot out; out.handled = true;
    if (mode_ == Mode::Symbols) {
        out.preedit = L"符號"; out.cursor = 2;
        out.first = expanded_ ? 0 : selected_ / 9 * 9; out.total = (unsigned)symbols.size();
        unsigned end = std::min<unsigned>(out.first + (expanded_ ? 81 : 9),out.total);
        for (unsigned i = out.first; i < end; ++i) out.candidates.push_back(std::wstring(1,symbols[i]));
        out.selected = (int)(selected_ - out.first); out.columns = expanded_ ? 9 : 0;
    } else if (mode_ != Mode::None) {
        out.preedit = L"`";
        if (mode_ == Mode::Unicode) out.preedit += L"U";
        if (mode_ == Mode::Big5) out.preedit += L"B";
        out.preedit += digits_; out.cursor = (unsigned)out.preedit.size();
    }
    return out;
}
shanjie::Snapshot SpecialInput::pick(unsigned index) {
    auto frame = view(); shanjie::Snapshot out; out.handled = true;
    if (mode_ == Mode::Symbols && index < frame.candidates.size()) { out.commit = frame.candidates[index]; clear(); }
    return out;
}
shanjie::Snapshot SpecialInput::key(const ShanjieKey& key,const Preferences& p) {
    if (!active()) { mode_ = key.ch == ',' ? Mode::Symbols : Mode::Prefix; return view(); }
    if (key.kind == 6) { clear(); return view(); }
    if (mode_ == Mode::Symbols) {
        if (key.kind == 13 && key.modifiers == 0 && !p.tabExpands) { clear(); auto out = view(); out.handled = false; return out; }
        if (key.modifiers == 0) {
            if (key.kind == 1 && key.ch >= '1' && key.ch <= '9') return pick(selected_ / 9 * 9 - view().first + key.ch - '1');
            if (key.kind == 3) return pick(selected_ - view().first);
            if (key.kind == 10) selected_ = std::min<unsigned>((unsigned)symbols.size()-1,selected_ + (expanded_ ? 9 : 1));
            if (key.kind == 9) {
                if (expanded_ && selected_ < 9 && p.gridUpCollapses) expanded_ = false;
                else selected_ = selected_ >= (expanded_ ? 9u : 1u) ? selected_ - (expanded_ ? 9 : 1) : 0;
            }
            if (key.kind == 2 && p.spacePages) {
                unsigned next = selected_ / 9 * 9 + 9;
                if (next < symbols.size()) selected_ = next;
                else if (p.wrapPages) selected_ = 0;
            }
            if ((key.kind == 13 && p.tabExpands) || (key.kind == 8 && p.rightExpands)) { if (!expanded_) expanded_ = true; else selected_ = std::min<unsigned>(selected_ + 1,(unsigned)symbols.size()-1); }
            else if (key.kind == 8) selected_ = std::min<unsigned>(selected_ + 1,(unsigned)symbols.size()-1);
            if (key.kind == 7) selected_ = selected_ ? selected_-1 : 0;
            if (key.kind == 4) clear();
        }
        return view();
    }
    if (key.kind == 4) { if (!digits_.empty()) digits_.pop_back(); else if (mode_ != Mode::Prefix) mode_ = Mode::Prefix; else clear(); return view(); }
    if (mode_ == Mode::Prefix && key.kind == 1 && !(key.modifiers & 30)) {
        wchar_t ch = (wchar_t)towlower((wchar_t)key.ch);
        if (p.codeInput && (ch == L'u' || ch == L'b')) { mode_ = ch == L'u' ? Mode::Unicode : Mode::Big5; return view(); }
        if (p.backquoteSymbols) {
            auto text = shanjie::windowsPunctuationText(key);
            if (!text.empty()) { clear(); auto out = view(); out.commit = text; return out; }
        }
    }
    if ((mode_ == Mode::Unicode || mode_ == Mode::Big5) && key.kind == 1 && !(key.modifiers & 30) && iswxdigit((wchar_t)key.ch)) {
        if (digits_.size() < (mode_ == Mode::Unicode ? 6u : 4u)) digits_ += (wchar_t)key.ch;
        return view();
    }
    if ((key.kind == 2 || key.kind == 3) && !digits_.empty()) {
        unsigned code = std::stoul(digits_,nullptr,16); std::wstring text;
        if (mode_ == Mode::Unicode && code > 0 && code <= 0x10ffff && !(code >= 0xd800 && code <= 0xdfff)) {
            if (code < 0x10000) text += (wchar_t)code;
            else { code -= 0x10000; text += (wchar_t)(0xd800 + (code >> 10)); text += (wchar_t)(0xdc00 + (code & 1023)); }
        } else if (mode_ == Mode::Big5 && digits_.size() == 4) {
            char bytes[]{(char)(code >> 8),(char)code}; wchar_t wide[2]{};
            int length = MultiByteToWideChar(950,MB_ERR_INVALID_CHARS,bytes,2,wide,2);
            if (length > 0) text.assign(wide,length);
        }
        if (!text.empty()) { clear(); auto out = view(); out.commit = text; return out; }
        return view();
    }
    return view();
}
