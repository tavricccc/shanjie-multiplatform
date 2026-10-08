#include "service.h"
#include "identity.h"
#include "key_policy.h"
#include "service_util.h"
#include <functional>
#include <shellapi.h>
using Microsoft::WRL::ComPtr;
namespace {
class Edit final : public ITfEditSession {
    long refs_ = 1;
    ComPtr<IUnknown> owner_;
    std::function<HRESULT(TfEditCookie)> operation_;
public:
    Edit(IUnknown* owner, std::function<HRESULT(TfEditCookie)> operation) : owner_(owner), operation_(std::move(operation)) { InterlockedIncrement(&objects); }
    ~Edit() { InterlockedDecrement(&objects); }
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override { *out = nullptr; if (iid != IID_IUnknown && iid != IID_ITfEditSession) return E_NOINTERFACE; *out = this; AddRef(); return S_OK; }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE DoEditSession(TfEditCookie cookie) override { return operation_(cookie); }
};
}
Service::Service() { InterlockedIncrement(&objects); }
Service::~Service() { Deactivate(); InterlockedDecrement(&objects); }
HRESULT Service::QueryInterface(REFIID iid, void** out) {
    *out = nullptr;
    if (iid == IID_IUnknown || iid == IID_ITfTextInputProcessor || iid == IID_ITfTextInputProcessorEx) *out = static_cast<ITfTextInputProcessorEx*>(this);
    else if (iid == IID_ITfKeyEventSink) *out = static_cast<ITfKeyEventSink*>(this);
    else if (iid == IID_ITfCompositionSink) *out = static_cast<ITfCompositionSink*>(this);
    else if (iid == IID_ITfThreadMgrEventSink) *out = static_cast<ITfThreadMgrEventSink*>(this);
    else if (iid == IID_ITfTextLayoutSink) *out = static_cast<ITfTextLayoutSink*>(this);
    else if (iid == IID_ITfTextEditSink) *out = static_cast<ITfTextEditSink*>(this);
    else if (iid == IID_ITfThreadFocusSink) *out = static_cast<ITfThreadFocusSink*>(this);
    if (!*out) return E_NOINTERFACE; AddRef(); return S_OK;
}
ULONG Service::AddRef() { return InterlockedIncrement(&refs_); }
ULONG Service::Release() { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
HRESULT Service::Activate(ITfThreadMgr* thread, TfClientId client) { return ActivateEx(thread,client,0); }
HRESULT Service::ActivateEx(ITfThreadMgr* thread, TfClientId client, DWORD) {
    thread_ = thread; client_ = client;
    preferences_ = loadPreferences();
    preferencesStamp_ = preferencesStamp();
    wchar_t path[32768]; auto length = GetModuleFileNameW(module,path,32768);
    if (!length || length == 32768) { Deactivate(); return E_FAIL; }
    unsigned layout = 0;
    ComPtr<ITfInputProcessorProfileMgr> profiles;
    TF_INPUTPROCESSORPROFILE active{};
    if (SUCCEEDED(CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&profiles))) && SUCCEEDED(profiles->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD,&active)) && active.guidProfile == EtenProfile) layout = 1;
    auto binaryRoot = std::filesystem::path(path).parent_path().parent_path();
    auto root = binaryRoot;
    if (root.parent_path().filename() == L"versions") root = root.parent_path().parent_path();
    profileLayout_ = layout; dataRoot_ = root / L"data";
    if (preferences_.keyboard) layout = preferences_.keyboard == 2 ? 1 : 0;
    if (!engine_.open(root / L"data",layout,preferences_.ranking,preferences_.rightAnchor,preferences_.learning,preferences_.big5,preferences_.dictionary,preferences_.model,preferences_.prediction)) { Deactivate(); return E_FAIL; }
    shanjie::Snapshot configured;
    if (!engine_.inputOptions(preferences_.shiftPunctuation,preferences_.plainPunctuation,preferences_.standaloneTones)
        || !engine_.demote(preferences_.demote,configured)) { Deactivate(); return E_FAIL; }
    if (preferences_.keyboard >= 3 && !pinyin_.load(root / L"data",preferences_.keyboard == 4)) { Deactivate(); return E_FAIL; }
    fullWidth_ = preferences_.initialFullWidth;
    setInputMode(preferences_.initialChinese);
    candidates_.Attach(new Candidates);
    candidates_->initialize(thread,preferences_,[this](unsigned index) {
        if (!context_) return;
        auto revision = candidates_->revision();
        ComPtr<ITfContext> context = context_;
        request(context.Get(),[this,context,index,revision](TfEditCookie cookie) {
            if (!candidates_ || candidates_->revision() != revision || context_ != context) return S_FALSE;
            shanjie::Snapshot out;
            bool conversion = !state_.prediction;
            if (associated_.active()) { if (!pickAssociated(index,out)) return E_FAIL; }
            else if (special_.active()) out = special_.pick(index);
            else if (!engine_.pick(index,out)) return E_FAIL;
            return apply(context.Get(),cookie,out,conversion && !out.associated && !out.preedit.empty());
        }, TF_ES_READWRITE | TF_ES_ASYNCDONTCARE);
    });
    ComPtr<ITfKeystrokeMgr> keys; ComPtr<ITfSource> source;
    HRESULT hr = thread->QueryInterface(IID_PPV_ARGS(&keys));
    if (SUCCEEDED(hr)) hr = keys->AdviseKeyEventSink(client,this,TRUE);
    if (SUCCEEDED(hr)) hr = thread->QueryInterface(IID_PPV_ARGS(&source));
    if (SUCCEEDED(hr)) hr = source->AdviseSink(IID_ITfThreadMgrEventSink,static_cast<ITfThreadMgrEventSink*>(this),&threadSink_);
    if (SUCCEEDED(hr)) hr = source->AdviseSink(IID_ITfThreadFocusSink,static_cast<ITfThreadFocusSink*>(this),&focusSink_);
    if (SUCCEEDED(hr) && !profileWatcher_.start([this] { refreshPreferences(); })) hr = E_FAIL;
    if (SUCCEEDED(hr) && SUCCEEDED(thread->QueryInterface(IID_PPV_ARGS(&languageBar_)))) {
        modeButton_.Attach(new ModeButton([this] { return compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE); },
            [this] { return toggleMode(context_.Get()); },[binaryRoot] { auto settings = binaryRoot / L"ui" / L"Shanjie.UI.exe"; ShellExecuteW(nullptr,L"open",settings.c_str(),nullptr,settings.parent_path().c_str(),SW_SHOWNORMAL); }));
        languageBar_->AddItem(modeButton_.Get());
    }
    if (FAILED(hr)) Deactivate();
    return hr;
}
HRESULT Service::Deactivate() {
    ++modeNotificationRevision_;
    modeIndicator_.hide();
    profileWatcher_.stop();
    cancel();
    if (languageBar_ && modeButton_) languageBar_->RemoveItem(modeButton_.Get());
    modeButton_.Reset(); languageBar_.Reset();
    if (thread_) {
        ComPtr<ITfKeystrokeMgr> keys; if (SUCCEEDED(thread_.As(&keys))) keys->UnadviseKeyEventSink(client_);
        ComPtr<ITfSource> source; if (threadSink_ != TF_INVALID_COOKIE && SUCCEEDED(thread_.As(&source))) source->UnadviseSink(threadSink_);
        if (focusSink_ != TF_INVALID_COOKIE && SUCCEEDED(thread_.As(&source))) source->UnadviseSink(focusSink_);
    }
    focusSink_ = threadSink_ = TF_INVALID_COOKIE; candidates_.Reset(); thread_.Reset(); client_ = TF_CLIENTID_NULL; return S_OK;
}
HRESULT Service::request(ITfContext* context, std::function<HRESULT(TfEditCookie)> operation, DWORD flags) {
    ComPtr<ITfEditSession> edit; edit.Attach(new Edit(static_cast<ITfTextInputProcessorEx*>(this),std::move(operation)));
    HRESULT result = E_FAIL;
    HRESULT hr = context->RequestEditSession(client_,edit.Get(),flags,&result);
    return FAILED(hr) ? hr : result;
}
void Service::cancel() {
    ++modeNotificationRevision_; modeIndicator_.hide();
    if (context_) {
        ComPtr<ITfSource> source;
        if (SUCCEEDED(context_.As(&source))) {
            if (layoutSink_ != TF_INVALID_COOKIE) source->UnadviseSink(layoutSink_);
            if (textSink_ != TF_INVALID_COOKIE) source->UnadviseSink(textSink_);
        }
    }
    layoutSink_ = textSink_ = TF_INVALID_COOKIE;
    if (composition_ && context_ && client_ != TF_CLIENTID_NULL) {
        ComPtr<ITfComposition> composition = composition_; ComPtr<ITfContext> context = context_;
        shanjie::Snapshot final;
        engine_.reset(false,final);
        if (pinyin_.active() || special_.textInput()) final.commit = state_.preedit;
        request(context.Get(),[composition,text = final.commit](TfEditCookie cookie) {
            ComPtr<ITfRange> range;
            HRESULT hr = composition->GetRange(&range);
            if (SUCCEEDED(hr)) hr = range->SetText(cookie,0,text.data(),(LONG)text.size());
            if (FAILED(hr)) return hr;
            return composition->EndComposition(cookie);
        },TF_ES_READWRITE | TF_ES_ASYNCDONTCARE);
    }
    composition_.Reset(); context_.Reset(); if (candidates_) candidates_->close();
    shiftTap_ = shiftUsed_ = false;
    special_.clear();
    associated_.clear();
    associationCaret_.Reset();
    pinyin_.clear();
    rawReading_.clear();
    shanjie::Snapshot discarded; engine_.reset(true,discarded); state_ = {};
}
HRESULT Service::watch(ITfContext* context) {
    if (context_.Get() == context) return S_OK;
    cancel();
    context_ = context;
    ComPtr<ITfSource> source;
    HRESULT hr = context->QueryInterface(IID_PPV_ARGS(&source));
    if (SUCCEEDED(hr)) hr = source->AdviseSink(IID_ITfTextLayoutSink,static_cast<ITfTextLayoutSink*>(this),&layoutSink_);
    if (SUCCEEDED(hr)) hr = source->AdviseSink(IID_ITfTextEditSink,static_cast<ITfTextEditSink*>(this),&textSink_);
    if (FAILED(hr)) cancel();
    return hr;
}
bool Service::writable(ITfContext* context) {
    TF_STATUS status{};
    if (FAILED(context->GetStatus(&status)) || status.dwStaticFlags & TF_SD_READONLY) return false;
    if (compartment(context,GUID_COMPARTMENT_KEYBOARD_DISABLED) || compartment(context,GUID_COMPARTMENT_EMPTYCONTEXT)) return false;
    return true;
}
bool Service::enabled(ITfContext* context) {
    return writable(context) && compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
}
void Service::refreshPreferences() {
    auto stamp = preferencesStamp();
    if (stamp == preferencesStamp_) return;
    preferencesStamp_ = stamp;
    auto current = loadPreferences();
    bool rebuild = current.keyboard != preferences_.keyboard || current.ranking != preferences_.ranking
        || current.rightAnchor != preferences_.rightAnchor || current.learning != preferences_.learning
        || current.big5 != preferences_.big5 || current.dictionary != preferences_.dictionary || current.model != preferences_.model || current.useContext != preferences_.useContext;
    bool modeChanged = current.initialChinese != preferences_.initialChinese;
    bool widthChanged = current.initialFullWidth != preferences_.initialFullWidth;
    bool predictionChanged = current.prediction != preferences_.prediction;
    bool demoteChanged = current.demote != preferences_.demote;
    // Finish existing text before replacing its engine; cancel captures the commit for TSF.
    if (rebuild || modeChanged) cancel();
    preferences_ = current;
    if (rebuild) {
        unsigned layout = current.keyboard ? (current.keyboard == 2 ? 1 : 0) : profileLayout_;
        if (!engine_.open(dataRoot_,layout,current.ranking,current.rightAnchor,current.learning,current.big5,current.dictionary,current.model,current.prediction)) {
            setInputMode(false); return;
        }
        pinyin_ = {};
        if (current.keyboard >= 3 && !pinyin_.load(dataRoot_,current.keyboard == 4)) { setInputMode(false); return; }
    }
    if (!engine_.inputOptions(current.shiftPunctuation,current.plainPunctuation,current.standaloneTones)) { setInputMode(false); return; }
    if (rebuild || predictionChanged || demoteChanged) {
        auto configure = [this,current,rebuild,predictionChanged,demoteChanged](shanjie::Snapshot& out) {
            if ((rebuild || demoteChanged) && !engine_.demote(current.demote,out)) return false;
            return !predictionChanged || rebuild || engine_.prediction(current.prediction,out);
        };
        if (context_) {
            auto revision = preferencesStamp_; ComPtr<ITfContext> context = context_;
            request(context.Get(),[this,context,revision,configure](TfEditCookie cookie) {
                if (revision != preferencesStamp_ || context_ != context) return S_FALSE;
                shanjie::Snapshot out; if (!configure(out)) return E_FAIL;
                associated_.clear(); associationCaret_.Reset();
                return apply(context.Get(),cookie,out);
            },TF_ES_READWRITE | TF_ES_ASYNCDONTCARE);
        } else { shanjie::Snapshot out; if (!configure(out)) { setInputMode(false); return; } }
    }
    if (!current.modeIndicator) { ++modeNotificationRevision_; modeIndicator_.hide(); }
    if (widthChanged) fullWidth_ = current.initialFullWidth;
    if (modeChanged || widthChanged) setInputMode(modeChanged ? current.initialChinese : compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE));
    if (candidates_) candidates_->configure(preferences_);
    if (!current.associations && associated_.active()) {
        associated_.clear(); associationCaret_.Reset();
        state_.associated = false; state_.candidates.clear(); state_.selected = -1; state_.columns = state_.first = state_.total = 0;
        if (candidates_) candidates_->close();
    }
    if (context_ && candidates_) OnLayoutChange(context_.Get(),TF_LC_CHANGE,nullptr);
}
void Service::notifyMode(ITfContext* input,bool chinese) {
    if (!preferences_.modeIndicator) return;
    auto revision = ++modeNotificationRevision_;
    ComPtr<ITfContext> current = input;
    if (!current && thread_) {
        ComPtr<ITfDocumentMgr> document;
        if (SUCCEEDED(thread_->GetFocus(&document)) && document) document->GetTop(&current);
    }
    if (!current) return;
    request(current.Get(),[this,current,chinese,revision](TfEditCookie cookie) {
        if (!thread_ || revision != modeNotificationRevision_ || compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE) != chinese) return S_FALSE;
        TF_SELECTION selection{}; ULONG fetched = 0;
        if (FAILED(current->GetSelection(cookie,TF_DEFAULT_SELECTION,1,&selection,&fetched)) || !fetched) return S_FALSE;
        ComPtr<ITfRange> range; range.Attach(selection.range); range->Collapse(cookie,TF_ANCHOR_END);
        ComPtr<ITfContextView> view; RECT caret{}; BOOL clipped = FALSE; HWND owner = nullptr;
        if (FAILED(current->GetActiveView(&view)) || FAILED(view->GetTextExt(cookie,range.Get(),&caret,&clipped)) || clipped) return S_FALSE;
        view->GetWnd(&owner);
        if (owner && GetAwarenessFromDpiAwarenessContext(GetWindowDpiAwarenessContext(owner)) != DPI_AWARENESS_PER_MONITOR_AWARE) {
            POINT start{caret.left,caret.top},end{caret.right,caret.bottom};
            if (!LogicalToPhysicalPointForPerMonitorDPI(owner,&start) || !LogicalToPhysicalPointForPerMonitorDPI(owner,&end)) return S_FALSE;
            caret = {start.x,start.y,end.x,end.y};
        }
        modeIndicator_.show(caret,owner ? GetDpiForWindow(owner) : 96,chinese); return S_OK;
    },TF_ES_READ | TF_ES_ASYNCDONTCARE);
}
HRESULT Service::toggleMode(ITfContext* input) {
    associated_.clear(); if (candidates_) candidates_->close();
    bool chinese = !compartment(thread_.Get(),GUID_COMPARTMENT_KEYBOARD_OPENCLOSE);
    // Chromium does not necessarily grant a synchronous write lock on key-up.
    // Empty compositions need no document edit; pending text commits asynchronously.
    if (!composition_) { auto hr = setInputMode(chinese); if (SUCCEEDED(hr)) notifyMode(input,chinese); return hr; }
    ComPtr<ITfContext> current = context_;
    return request(current.Get(),[this,current,chinese](TfEditCookie cookie) {
        auto pending = pinyin_.active() || special_.textInput() ? state_.preedit : std::wstring{};
        shanjie::Snapshot out;
        if (!engine_.reset(false,out)) return E_FAIL;
        if (!pending.empty()) out.commit = pending;
        special_.clear(); pinyin_.clear(); rawReading_.clear();
        HRESULT hr = apply(current.Get(),cookie,out);
        if (FAILED(hr)) return hr;
        hr = setInputMode(chinese); if (SUCCEEDED(hr)) notifyMode(current.Get(),chinese); return hr;
    },TF_ES_ASYNCDONTCARE | TF_ES_READWRITE);
}
HRESULT Service::setInputMode(bool chinese) {
    ComPtr<ITfCompartmentMgr> manager; ComPtr<ITfCompartment> open;
    HRESULT hr = thread_.As(&manager);
    if (SUCCEEDED(hr)) hr = manager->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&open);
    if (FAILED(hr)) return hr;
    VARIANT value; VariantInit(&value); value.vt = VT_I4;
    ComPtr<ITfCompartment> conversion;
    hr = manager->GetCompartment(GUID_COMPARTMENT_KEYBOARD_INPUTMODE_CONVERSION,&conversion);
    if (FAILED(hr)) return hr;
    value.lVal = (chinese ? TF_CONVERSIONMODE_NATIVE : 0) | (fullWidth_ ? TF_CONVERSIONMODE_FULLSHAPE : 0);
    hr = conversion->SetValue(client_,&value); if (FAILED(hr)) return hr;
    value.lVal = chinese ? 1 : 0;
    hr = open->SetValue(client_,&value);
    if (modeButton_) modeButton_->refresh();
    return hr;
}
bool Service::pickAssociated(unsigned index, shanjie::Snapshot& out) {
    auto text = associated_.pick(index);
    if (text.associated) { out = text; return true; }
    if (!engine_.reset(false,out)) return false;
    out.commit.insert(std::min<size_t>(state_.cursor,out.commit.size()),text.commit);
    out.handled = true; associationCaret_.Reset(); return true;
}
HRESULT Service::apply(ITfContext* context, TfEditCookie cookie, const shanjie::Snapshot& out, bool selectedCandidate) {
    ComPtr<ITfRange> range;
    if (composition_) {
        HRESULT hr = composition_->GetRange(&range); if (FAILED(hr)) return hr;
    } else {
        ComPtr<ITfInsertAtSelection> insert;
        HRESULT hr = context->QueryInterface(IID_PPV_ARGS(&insert)); if (FAILED(hr)) return hr;
        hr = insert->InsertTextAtSelection(cookie,TF_IAS_QUERYONLY,nullptr,0,&range); if (FAILED(hr)) return hr;
    }
    if (!out.handled && out.commit.empty() && out.preedit.empty()) return S_OK;
    auto text = out.commit + out.preedit;
    if (fullWidth_) for (size_t i = 0; i < out.commit.size(); ++i) if (text[i] >= L' ' && text[i] <= L'~') text[i] = text[i] == L' ' ? L'\u3000' : (wchar_t)(text[i] + 0xfee0);
    HRESULT hr = range->SetText(cookie,0,text.data(),(LONG)text.size()); if (FAILED(hr)) return hr;
    if (!out.commit.empty()) {
        LONG moved = 0; hr = range->ShiftStart(cookie,(LONG)out.commit.size(),&moved,nullptr); if (FAILED(hr)) return hr;
        if (composition_) { ending_ = true; hr = composition_->EndComposition(cookie); ending_ = false; composition_.Reset(); if (FAILED(hr)) return hr; }
    }
    if (!out.preedit.empty() && !composition_) {
        ComPtr<ITfContextComposition> composing; hr = context->QueryInterface(IID_PPV_ARGS(&composing));
        if (SUCCEEDED(hr)) hr = composing->StartComposition(cookie,range.Get(),this,&composition_);
        if (FAILED(hr)) return hr;
    }
    if (out.preedit.empty() && composition_) { ending_ = true; hr = composition_->EndComposition(cookie); ending_ = false; composition_.Reset(); if (FAILED(hr)) return hr; }
    ComPtr<ITfRange> caret; hr = range->Clone(&caret); if (FAILED(hr)) return hr;
    caret->Collapse(cookie,TF_ANCHOR_START); LONG moved = 0;
    caret->ShiftEnd(cookie,(LONG)out.cursor,&moved,nullptr); caret->Collapse(cookie,TF_ANCHOR_END);
    TF_SELECTION selection{caret.Get(),{TF_AE_NONE,FALSE}};
    hr = context->SetSelection(cookie,1,&selection); if (FAILED(hr)) return hr;
    state_ = out; context_ = context;
    if (preferences_.associations && selectedCandidate && out.candidates.empty() && out.cursor && !out.preedit.empty()) {
        auto before = out.preedit.substr(0,out.cursor);
        auto prefix = before.substr(before.size() > 1 ? before.size()-2 : 0);
        shanjie::Snapshot suggestions;
        if (!engine_.associate(prefix,suggestions)) return E_FAIL;
        if (suggestions.candidates.empty() && prefix.size() > 1 && !engine_.associate(prefix.substr(1),suggestions)) return E_FAIL;
        associated_.open(suggestions,out); state_ = associated_.view();
        caret->Clone(&associationCaret_);
    }
    candidates_->update(context,cookie,caret.Get(),state_);
    return S_OK;
}
HRESULT Service::OnCompositionTerminated(TfEditCookie, ITfComposition* ended) {
    if (!ending_ && composition_.Get() == ended) { composition_.Reset(); cancel(); }
    return S_OK;
}
HRESULT Service::OnSetFocus(BOOL focused) { if (!focused) cancel(); return S_OK; }
HRESULT Service::OnSetThreadFocus() { return S_OK; }
HRESULT Service::OnKillThreadFocus() { ++modeNotificationRevision_; modeIndicator_.hide(); cancel(); return S_OK; }
HRESULT Service::OnInitDocumentMgr(ITfDocumentMgr*) { return S_OK; }
HRESULT Service::OnUninitDocumentMgr(ITfDocumentMgr*) { return S_OK; }
HRESULT Service::OnSetFocus(ITfDocumentMgr* current, ITfDocumentMgr* previous) { if (current != previous) cancel(); return S_OK; }
HRESULT Service::OnPushContext(ITfContext*) { return S_OK; }
HRESULT Service::OnPopContext(ITfContext* context) { if (context_.Get() == context) cancel(); return S_OK; }
HRESULT Service::OnLayoutChange(ITfContext* context, TfLayoutCode code, ITfContextView*) {
    if (context_.Get() != context || !candidates_) return S_OK;
    if (code == TF_LC_DESTROY) { cancel(); return S_OK; }
    if (state_.candidates.empty()) return S_OK;
    ComPtr<ITfContext> current = context_;
    return request(context,[this,current](TfEditCookie cookie) {
        if (context_ != current || state_.candidates.empty()) return S_OK;
        TF_SELECTION selection{}; ULONG fetched = 0;
        HRESULT hr = current->GetSelection(cookie,TF_DEFAULT_SELECTION,1,&selection,&fetched);
        if (FAILED(hr) || !fetched) return hr;
        ComPtr<ITfRange> caret; caret.Attach(selection.range);
        caret->Collapse(cookie,TF_ANCHOR_END);
        candidates_->update(current.Get(),cookie,caret.Get(),state_);
        return S_OK;
    },TF_ES_READ | TF_ES_ASYNCDONTCARE);
}
HRESULT Service::OnEndEdit(ITfContext* context, TfEditCookie cookie, ITfEditRecord* record) {
    if (context_.Get() != context) return S_OK;
    BOOL changed = FALSE;
    if (FAILED(record->GetSelectionStatus(&changed)) || !changed) return S_OK;
    TF_SELECTION selected{}; ULONG fetched = 0;
    if (FAILED(context->GetSelection(cookie,TF_DEFAULT_SELECTION,1,&selected,&fetched)) || !fetched) return S_OK;
    ComPtr<ITfRange> caret; caret.Attach(selected.range);
    if (!composition_) {
        if (associated_.active() && associationCaret_) {
            LONG start = 0, end = 0;
            caret->CompareStart(cookie,associationCaret_.Get(),TF_ANCHOR_START,&start);
            caret->CompareEnd(cookie,associationCaret_.Get(),TF_ANCHOR_END,&end);
            if (start || end) { associated_.clear(); associationCaret_.Reset(); state_ = {}; candidates_->close(); }
        }
        return S_OK;
    }
    caret->Collapse(cookie,selected.style.ase == TF_AE_START ? TF_ANCHOR_START : TF_ANCHOR_END);
    ComPtr<ITfRange> range; if (FAILED(composition_->GetRange(&range))) return S_OK;
    LONG before = 0, after = 0;
    caret->CompareStart(cookie,range.Get(),TF_ANCHOR_START,&before);
    caret->CompareStart(cookie,range.Get(),TF_ANCHOR_END,&after);
    if (before < 0 || after > 0) { cancel(); return S_OK; }
    ComPtr<ITfRange> leftRange; if (FAILED(range->Clone(&leftRange))) return S_OK;
    if (FAILED(leftRange->ShiftEndToRange(cookie,caret.Get(),TF_ANCHOR_START))) return S_OK;
    std::wstring left(state_.preedit.size(),L'\0'); ULONG target = 0;
    if (FAILED(leftRange->GetText(cookie,0,left.data(),(ULONG)left.size(),&target)) || target == state_.cursor) return S_OK;
    shanjie::Snapshot out;
    if (state_.associated) {
        associated_.clear(); associationCaret_.Reset(); state_.candidates.clear(); state_.associated = false;
    } else if (!state_.candidates.empty()) {
        ShanjieKey escape{}; escape.kind = 6;
        if (!engine_.key(escape,out)) return E_FAIL;
    }
    ShanjieKey home{}; home.kind = 11;
    if (!engine_.key(home,out)) return E_FAIL;
    ShanjieKey right{}; right.kind = 8;
    while (out.cursor < target) {
        shanjie::Snapshot next;
        if (!engine_.key(right,next)) return E_FAIL;
        if (next.cursor <= out.cursor) break;
        out = std::move(next);
    }
    state_ = std::move(out); candidates_->close();
    return S_OK;
}
