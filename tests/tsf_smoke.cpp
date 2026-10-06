// Exercises the real Windows TSF context and edit locks without registration or
// opening a window by default. --visual also exercises deferred layout and
// leaves an interactive candidate window open for inspection.
#include <windows.h>
#include <msctf.h>
#include <textstor.h>
#include <psapi.h>
#include <wrl/client.h>
#include <algorithm>
#include <cstdio>
#include <string>
#include "identity.h"
using Microsoft::WRL::ComPtr;
#include "thread_proxy.h"
class Store final : public ITextStoreACP {
    long refs_ = 1;
    ComPtr<ITextStoreACPSink> sink_;
public:
    std::wstring text;
    HWND window = nullptr;
    bool layoutReady = false;
    void layoutChanged() { if (sink_) sink_->OnLayoutChange(TS_LC_CHANGE,1); }
    TS_SELECTION_ACP selection{0,0,{TS_AE_NONE,FALSE}};
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override { *out = nullptr; if (iid != IID_IUnknown && iid != IID_ITextStoreACP) return E_NOINTERFACE; *out = this; AddRef(); return S_OK; }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE AdviseSink(REFIID iid, IUnknown* sink, DWORD) override { if (iid != IID_ITextStoreACPSink) return E_NOINTERFACE; return sink->QueryInterface(IID_PPV_ARGS(&sink_)); }
    HRESULT STDMETHODCALLTYPE UnadviseSink(IUnknown*) override { sink_.Reset(); return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestLock(DWORD flags, HRESULT* result) override { if (!sink_) return E_FAIL; *result = sink_->OnLockGranted(flags); return S_OK; }
    HRESULT STDMETHODCALLTYPE GetStatus(TS_STATUS* out) override { *out = {0,0}; return S_OK; }
    HRESULT STDMETHODCALLTYPE QueryInsert(LONG start, LONG end, ULONG, LONG* a, LONG* b) override { *a = start; *b = end; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetSelection(ULONG, ULONG count, TS_SELECTION_ACP* out, ULONG* fetched) override { *fetched = count ? 1 : 0; if (count) *out = selection; return S_OK; }
    HRESULT STDMETHODCALLTYPE SetSelection(ULONG count, const TS_SELECTION_ACP* value) override { if (count != 1) return E_INVALIDARG; selection = *value; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetText(LONG start, LONG end, WCHAR* plain, ULONG capacity, ULONG* count, TS_RUNINFO* runs, ULONG runCapacity, ULONG* runCount, LONG* next) override {
        if (end == -1) end = (LONG)text.size(); if (start < 0 || end < start || end > (LONG)text.size()) return TS_E_INVALIDPOS;
        *count = std::min<ULONG>(capacity,end-start); if (*count) std::copy_n(text.data()+start,*count,plain);
        *runCount = runCapacity ? 1 : 0; if (runCapacity) runs[0] = {*count,TS_RT_PLAIN}; *next = start + *count; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetText(DWORD, LONG start, LONG end, const WCHAR* value, ULONG count, TS_TEXTCHANGE* change) override {
        if (start < 0 || end < start || end > (LONG)text.size()) return TS_E_INVALIDPOS;
        text.replace(start,end-start,value,count); *change = {start,end,start+(LONG)count};
        if (window) InvalidateRect(window,nullptr,TRUE);
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetFormattedText(LONG,LONG,IDataObject**) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE GetEmbedded(LONG,REFGUID,REFIID,IUnknown**) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE QueryInsertEmbedded(const GUID*,const FORMATETC*,BOOL* out) override { *out = FALSE; return S_OK; }
    HRESULT STDMETHODCALLTYPE InsertEmbedded(DWORD,LONG,LONG,IDataObject*,TS_TEXTCHANGE*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE InsertTextAtSelection(DWORD flags,const WCHAR* value,ULONG count,LONG* start,LONG* end,TS_TEXTCHANGE* change) override {
        if (start) *start = selection.acpStart; if (end) *end = selection.acpEnd;
        if (flags & TS_IAS_QUERYONLY) return S_OK;
        auto hr = SetText(0,selection.acpStart,selection.acpEnd,value,count,change);
        selection.acpStart = selection.acpEnd = change->acpNewEnd;
        if (end) *end = change->acpNewEnd; return hr;
    }
    HRESULT STDMETHODCALLTYPE InsertEmbeddedAtSelection(DWORD,IDataObject*,LONG*,LONG*,TS_TEXTCHANGE*) override { return E_NOTIMPL; }
    HRESULT STDMETHODCALLTYPE RequestSupportedAttrs(DWORD,ULONG,const TS_ATTRID*) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestAttrsAtPosition(LONG,ULONG,const TS_ATTRID*,DWORD) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestAttrsTransitioningAtPosition(LONG,ULONG,const TS_ATTRID*,DWORD) override { return S_OK; }
    HRESULT STDMETHODCALLTYPE FindNextAttrTransition(LONG,LONG halt,ULONG,const TS_ATTRID*,DWORD,LONG* next,BOOL* found,LONG* offset) override { *next = halt; *found = FALSE; *offset = 0; return S_OK; }
    HRESULT STDMETHODCALLTYPE RetrieveRequestedAttrs(ULONG,TS_ATTRVAL*,ULONG* fetched) override { *fetched = 0; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetEndACP(LONG* out) override { *out = (LONG)text.size(); return S_OK; }
    HRESULT STDMETHODCALLTYPE GetActiveView(TsViewCookie* out) override { *out = 1; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetACPFromPoint(TsViewCookie,const POINT*,DWORD,LONG*) override { return TS_E_NOLAYOUT; }
    HRESULT STDMETHODCALLTYPE GetTextExt(TsViewCookie,LONG start,LONG,RECT* out,BOOL* clipped) override {
        if (!layoutReady || !window) return TS_E_NOLAYOUT;
        POINT point{24 + start * 24,80}; ClientToScreen(window,&point);
        *out = {point.x,point.y,point.x+1,point.y+26}; *clipped = FALSE; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetScreenExt(TsViewCookie,RECT* out) override { *out = {}; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetWnd(TsViewCookie,HWND* out) override { *out = window; return S_OK; }
};
Store* visibleStore = nullptr;
ITfKeyEventSink* visibleKeys = nullptr;
ITfContext* visibleContext = nullptr;
LRESULT CALLBACK testProcedure(HWND window, UINT message, WPARAM key, LPARAM flags) {
    if (message == WM_KEYDOWN && visibleKeys) {
        BOOL eaten = FALSE; visibleKeys->OnKeyDown(visibleContext,key,flags,&eaten);
        if (eaten) return 0;
    }
    if (message == WM_PAINT && visibleStore) {
        PAINTSTRUCT paint{}; HDC dc = BeginPaint(window,&paint);
        SetBkMode(dc,TRANSPARENT);
        const wchar_t* label = L"善解原生 TSF 測試：方向鍵、數字、空白、Enter、Esc";
        TextOutW(dc,24,24,label,(int)wcslen(label));
        TextOutW(dc,24,80,visibleStore->text.data(),(int)visibleStore->text.size());
        EndPaint(window,&paint); return 0;
    }
    if (message == WM_CLOSE) { ShowWindow(window,SW_HIDE); PostQuitMessage(0); return 0; }
    return DefWindowProcW(window,message,key,flags);
}
void require(bool condition, int check) { if (!condition) { std::printf("TSF check %d failed\n",check); std::fflush(stdout); ExitProcess(check); } }
int wmain(int argc, wchar_t** argv) {
    require(argc == 2 || (argc == 3 && std::wstring(argv[2]) == L"--visual"),1);
    bool visual = argc == 3;
    require(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)),2);
    HMODULE dll = LoadLibraryW(argv[1]); require(dll != nullptr,3);
    auto factoryFn = (HRESULT (WINAPI*)(REFCLSID,REFIID,void**))GetProcAddress(dll,"DllGetClassObject"); require(factoryFn != nullptr,4);
    ComPtr<IClassFactory> factory; require(SUCCEEDED(factoryFn(ServiceId,IID_PPV_ARGS(&factory))),5);
    ComPtr<ITfTextInputProcessorEx> service; require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&service))),6);
    ComPtr<ITfThreadMgr> thread; require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&thread))),7);
    TfClientId client; require(SUCCEEDED(thread->Activate(&client)),8);
    ComPtr<ITfDocumentMgr> document; require(SUCCEEDED(thread->CreateDocumentMgr(&document)),9);
    ComPtr<Store> store; store.Attach(new Store);
    if (visual) {
        WNDCLASSW wc{}; wc.lpfnWndProc = testProcedure; wc.hInstance = GetModuleHandleW(nullptr);
        wc.lpszClassName = L"ShanjieVisualTest"; wc.hbrBackground = GetSysColorBrush(COLOR_WINDOW);
        RegisterClassW(&wc); visibleStore = store.Get();
        store->window = CreateWindowW(wc.lpszClassName,L"善解候選窗測試",WS_OVERLAPPEDWINDOW,120,120,1000,400,nullptr,nullptr,wc.hInstance,nullptr);
        require(store->window != nullptr,60); ShowWindow(store->window,SW_SHOW);
    }
    ComPtr<ITfContext> context; TfEditCookie cookie;
    require(SUCCEEDED(document->CreateContext(client,0,store.Get(),&context,&cookie)),10);
    require(SUCCEEDED(document->Push(context.Get())),11); require(SUCCEEDED(thread->SetFocus(document.Get())),12);
    if (visual) {
        ComPtr<ITfDocumentMgr> previous;
        require(SUCCEEDED(thread->AssociateFocus(store->window,document.Get(),&previous)),63);
        SetForegroundWindow(store->window);
    }
    ComPtr<ITfClientId> identifiers; TfClientId serviceClient = TF_CLIENTID_NULL;
    require(SUCCEEDED(thread.As(&identifiers)) && SUCCEEDED(identifiers->GetClientId(ServiceId,&serviceClient)),15);
    ComPtr<ThreadProxy> proxy; proxy.Attach(new ThreadProxy(thread.Get()));
    HRESULT activation = service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY);
    if (FAILED(activation)) std::printf("Activation HRESULT %08lx\n",(unsigned long)activation);
    require(SUCCEEDED(activation),13);
    PROCESS_MEMORY_COUNTERS_EX memory{}; memory.cb = sizeof(memory);
    if (GetProcessMemoryInfo(GetCurrentProcess(),(PROCESS_MEMORY_COUNTERS*)&memory,sizeof(memory)))
        std::printf("Harness memory after engine activation: private %.1f MiB, working set %.1f MiB\n",memory.PrivateUsage/1048576.0,memory.WorkingSetSize/1048576.0);
    ComPtr<ITfKeyEventSink> keys; require(SUCCEEDED(service.As(&keys)),14);
    auto press = [&](WPARAM vk, int check) { BOOL test = FALSE, eaten = FALSE; require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),vk,0,&test)) && test,check); require(SUCCEEDED(keys->OnKeyDown(context.Get(),vk,0,&eaten)) && eaten,check+1); };
    for (wchar_t key : std::wstring(L"SU3CL3")) press(key,20);
    require(store->text == L"你好",30); require(store->selection.acpStart == 2 && store->selection.acpEnd == 2,31);
    press(VK_RETURN,32); require(store->text == L"你好",34);
    press('S',35); press(VK_ESCAPE,37); require(store->text == L"你好",39);
    press('S',40); press('U',42); press('3',44); press(VK_DOWN,46);
    ComPtr<ITfUIElementMgr> ui; require(SUCCEEDED(thread.As(&ui)),48);
    ComPtr<IEnumTfUIElements> elements; require(SUCCEEDED(ui->EnumUIElements(&elements)),49);
    ComPtr<ITfUIElement> element; ULONG fetched = 0; require(elements->Next(1,&element,&fetched) == S_OK && fetched,50);
    ComPtr<ITfCandidateListUIElement> candidates; require(SUCCEEDED(element.As(&candidates)),51);
    UINT count = 0; candidates->GetCount(&count); require(count > 0 && count <= 9,52);
    if (!visual) {
        press(VK_DOWN,70); UINT selection = 0; candidates->GetSelection(&selection); require(selection == 1,72);
        press(VK_RETURN,73); require(store->text == L"你好妳",75);
        press(VK_RETURN,76);
        press('S',78); press('U',80); press('3',82); press(VK_DOWN,84); press('2',86);
        require(store->text == L"你好妳妳",88); press(VK_RETURN,89);
        press(VK_SHIFT,91); BOOL eaten = FALSE;
        require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_SHIFT,0,&eaten)) && eaten,93);
        BOOL test = TRUE; keys->OnTestKeyDown(context.Get(),'S',0,&test); require(!test,94);
        press(VK_SHIFT,95); require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_SHIFT,0,&eaten)) && eaten,97);
        press('S',98); press(VK_ESCAPE,100);
    }
    if (visual) {
        BOOL shown = TRUE; candidates->IsShown(&shown); require(!shown,61);
        store->layoutReady = true; store->layoutChanged();
        MSG msg{};
        for (unsigned i = 0; i < 50; ++i) {
            while (PeekMessageW(&msg,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&msg); DispatchMessageW(&msg); }
            candidates->IsShown(&shown); if (shown) break;
            Sleep(10);
        }
        require(shown,62);
        std::puts("Deferred TSF layout: native candidate window became visible"); std::fflush(stdout);
        visibleKeys = keys.Get(); visibleContext = context.Get();
        while (GetMessageW(&msg,nullptr,0,0) > 0) { TranslateMessage(&msg); DispatchMessageW(&msg); }
        visibleKeys = nullptr; visibleContext = nullptr;
    }
    service->Deactivate(); keys.Reset(); service.Reset(); candidates.Reset(); element.Reset(); elements.Reset(); ui.Reset();
    thread->SetFocus(nullptr); document->Pop(TF_POPF_ALL); context.Reset(); document.Reset();
    if (store->window) DestroyWindow(store->window); visibleStore = nullptr;
    store.Reset(); proxy.Reset(); identifiers.Reset(); thread->Deactivate(); thread.Reset(); factory.Reset();
    auto unload = (HRESULT (WINAPI*)())GetProcAddress(dll,"DllCanUnloadNow"); require(unload && unload() == S_OK,55);
    FreeLibrary(dll); CoUninitialize(); std::puts("TSF composition, caret, commit, cancel, candidates, and COM lifetime passed");
}
