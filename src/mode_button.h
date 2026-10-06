#pragma once
#include <ctfutb.h>
#include <wrl/client.h>
#include <functional>
class ModeButton final : public ITfLangBarItemButton, public ITfSource {
    long refs_ = 1;
    bool shown_ = true;
    Microsoft::WRL::ComPtr<ITfLangBarItemSink> sink_;
    std::function<bool()> chinese_;
    std::function<HRESULT()> toggle_;
    std::function<void()> settings_;
public:
    ModeButton(std::function<bool()>,std::function<HRESULT()>,std::function<void()>);
    ~ModeButton();
    void refresh();
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID,void**) override;
    ULONG STDMETHODCALLTYPE AddRef() override;
    ULONG STDMETHODCALLTYPE Release() override;
    HRESULT STDMETHODCALLTYPE GetInfo(TF_LANGBARITEMINFO*) override;
    HRESULT STDMETHODCALLTYPE GetStatus(DWORD*) override;
    HRESULT STDMETHODCALLTYPE Show(BOOL) override;
    HRESULT STDMETHODCALLTYPE GetTooltipString(BSTR*) override;
    HRESULT STDMETHODCALLTYPE OnClick(TfLBIClick,POINT,const RECT*) override;
    HRESULT STDMETHODCALLTYPE InitMenu(ITfMenu*) override;
    HRESULT STDMETHODCALLTYPE OnMenuSelect(UINT) override;
    HRESULT STDMETHODCALLTYPE GetIcon(HICON*) override;
    HRESULT STDMETHODCALLTYPE GetText(BSTR*) override;
    HRESULT STDMETHODCALLTYPE AdviseSink(REFIID,IUnknown*,DWORD*) override;
    HRESULT STDMETHODCALLTYPE UnadviseSink(DWORD) override;
};
