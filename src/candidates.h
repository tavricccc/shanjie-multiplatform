#pragma once
#include <msctf.h>
#include <wrl/client.h>
#include <functional>
#include "engine.h"
class Candidates final : public ITfCandidateListUIElementBehavior {
    long refs_ = 1;
    HWND window_ = nullptr;
    DWORD id_ = TF_INVALID_UIELEMENTID;
    Microsoft::WRL::ComPtr<ITfUIElementMgr> manager_;
    Microsoft::WRL::ComPtr<ITfDocumentMgr> document_;
    shanjie::Snapshot state_;
    bool native_ = true;
    int cell_ = 100, row_ = 36, pad_ = 8, columns_ = 9;
    HFONT font_ = nullptr;
    std::function<void(unsigned)> pick_;
    static LRESULT CALLBACK procedure(HWND, UINT, WPARAM, LPARAM);
    void paint();
public:
    Candidates();
    ~Candidates();
    void initialize(ITfThreadMgr*, std::function<void(unsigned)>);
    void update(ITfContext*, TfEditCookie, ITfRange*, const shanjie::Snapshot&);
    void close();
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID, void**) override;
    ULONG STDMETHODCALLTYPE AddRef() override;
    ULONG STDMETHODCALLTYPE Release() override;
    HRESULT STDMETHODCALLTYPE GetDescription(BSTR*) override;
    HRESULT STDMETHODCALLTYPE GetGUID(GUID*) override;
    HRESULT STDMETHODCALLTYPE Show(BOOL) override;
    HRESULT STDMETHODCALLTYPE IsShown(BOOL*) override;
    HRESULT STDMETHODCALLTYPE GetUpdatedFlags(DWORD*) override;
    HRESULT STDMETHODCALLTYPE GetDocumentMgr(ITfDocumentMgr**) override;
    HRESULT STDMETHODCALLTYPE GetCount(UINT*) override;
    HRESULT STDMETHODCALLTYPE GetSelection(UINT*) override;
    HRESULT STDMETHODCALLTYPE GetString(UINT, BSTR*) override;
    HRESULT STDMETHODCALLTYPE GetPageIndex(UINT*, UINT, UINT*) override;
    HRESULT STDMETHODCALLTYPE SetPageIndex(UINT*, UINT) override;
    HRESULT STDMETHODCALLTYPE GetCurrentPage(UINT*) override;
    HRESULT STDMETHODCALLTYPE SetSelection(UINT) override;
    HRESULT STDMETHODCALLTYPE Finalize() override;
    HRESULT STDMETHODCALLTYPE Abort() override;
};
