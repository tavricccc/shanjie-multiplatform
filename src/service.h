#pragma once
#include <msctf.h>
#include <wrl/client.h>
#include "candidates.h"
#include "preferences.h"
class Service final : public ITfTextInputProcessorEx, public ITfKeyEventSink,
    public ITfCompositionSink, public ITfThreadMgrEventSink, public ITfTextLayoutSink {
    long refs_ = 1;
    TfClientId client_ = TF_CLIENTID_NULL;
    DWORD threadSink_ = TF_INVALID_COOKIE;
    DWORD layoutSink_ = TF_INVALID_COOKIE;
    Microsoft::WRL::ComPtr<ITfThreadMgr> thread_;
    Microsoft::WRL::ComPtr<ITfContext> context_;
    Microsoft::WRL::ComPtr<ITfComposition> composition_;
    Microsoft::WRL::ComPtr<Candidates> candidates_;
    shanjie::Engine engine_;
    shanjie::Snapshot state_;
    Preferences preferences_;
    bool ending_ = false;
    bool shiftTap_ = false, shiftUsed_ = false, fullWidth_ = false;
    bool writable(ITfContext*);
    HRESULT toggleMode(ITfContext*);
    bool enabled(ITfContext*);
    bool wants(ITfContext*, WPARAM, LPARAM);
    HRESULT apply(ITfContext*, TfEditCookie, const shanjie::Snapshot&);
    void cancel();
    HRESULT watch(ITfContext*);
    HRESULT request(ITfContext*, std::function<HRESULT(TfEditCookie)>, DWORD);
public:
    Service(); ~Service();
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID, void**) override;
    ULONG STDMETHODCALLTYPE AddRef() override;
    ULONG STDMETHODCALLTYPE Release() override;
    HRESULT STDMETHODCALLTYPE Activate(ITfThreadMgr*, TfClientId) override;
    HRESULT STDMETHODCALLTYPE ActivateEx(ITfThreadMgr*, TfClientId, DWORD) override;
    HRESULT STDMETHODCALLTYPE Deactivate() override;
    HRESULT STDMETHODCALLTYPE OnSetFocus(BOOL) override;
    HRESULT STDMETHODCALLTYPE OnTestKeyDown(ITfContext*, WPARAM, LPARAM, BOOL*) override;
    HRESULT STDMETHODCALLTYPE OnTestKeyUp(ITfContext*, WPARAM, LPARAM, BOOL*) override;
    HRESULT STDMETHODCALLTYPE OnKeyDown(ITfContext*, WPARAM, LPARAM, BOOL*) override;
    HRESULT STDMETHODCALLTYPE OnKeyUp(ITfContext*, WPARAM, LPARAM, BOOL*) override;
    HRESULT STDMETHODCALLTYPE OnPreservedKey(ITfContext*, REFGUID, BOOL*) override;
    HRESULT STDMETHODCALLTYPE OnCompositionTerminated(TfEditCookie, ITfComposition*) override;
    HRESULT STDMETHODCALLTYPE OnInitDocumentMgr(ITfDocumentMgr*) override;
    HRESULT STDMETHODCALLTYPE OnUninitDocumentMgr(ITfDocumentMgr*) override;
    HRESULT STDMETHODCALLTYPE OnSetFocus(ITfDocumentMgr*, ITfDocumentMgr*) override;
    HRESULT STDMETHODCALLTYPE OnPushContext(ITfContext*) override;
    HRESULT STDMETHODCALLTYPE OnPopContext(ITfContext*) override;
    HRESULT STDMETHODCALLTYPE OnLayoutChange(ITfContext*, TfLayoutCode, ITfContextView*) override;
};
