// Exercises the real Windows TSF context and edit locks without registration or
// opening a window by default. --visual also exercises deferred layout and
// leaves an interactive candidate window open for inspection.
#include <windows.h>
#include <msctf.h>
#include <textstor.h>
#include <psapi.h>
#include <wrl/client.h>
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <string>
#include <thread>
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
    void externalCaret(LONG at) { selection.acpStart = selection.acpEnd = at; if (sink_) sink_->OnSelectionChange(); }
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
    require(argc == 2 || (argc == 3 && (std::wstring(argv[2]) == L"--visual" || std::wstring(argv[2]) == L"--shanjie" || std::wstring(argv[2]) == L"--cold-start")),1);
    bool visual = argc == 3 && std::wstring(argv[2]) == L"--visual";
    bool original = argc == 3 && std::wstring(argv[2]) == L"--shanjie";
    bool cold = argc == 3 && std::wstring(argv[2]) == L"--cold-start";
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
    using Clock = std::chrono::steady_clock;
    auto started = Clock::now();
    HRESULT activation = service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY);
    std::printf("TSF activation: %.3f ms\n",std::chrono::duration<double,std::milli>(Clock::now()-started).count());
    if (FAILED(activation)) std::printf("Activation HRESULT %08lx\n",(unsigned long)activation);
    if (FAILED(activation)) {
        auto error = reinterpret_cast<const char*(__cdecl*)()>(GetProcAddress(dll,"sj_error"));
        if (error) std::printf("Rust activation detail: %s\n",error());
    }
    require(SUCCEEDED(activation),13);
    auto residentStatus = (unsigned long long (__cdecl*)())GetProcAddress(dll,"sj_resident_status");
    auto liveObjects = (unsigned (__cdecl*)())GetProcAddress(dll,"sj_tsf_objects");
    require(residentStatus && liveObjects,460);
    auto waitPreload = [&]() {
        auto start = Clock::now();
        while ((residentStatus() >> 32) != (residentStatus() & 0xffffffffULL)) {
            require(Clock::now()-start < std::chrono::seconds(15),461);
            Sleep(5);
        }
        std::printf("Background preload wait: %.3f ms\n",std::chrono::duration<double,std::milli>(Clock::now()-start).count());
    };
    PROCESS_MEMORY_COUNTERS_EX memory{}; memory.cb = sizeof(memory);
    if (GetProcessMemoryInfo(GetCurrentProcess(),(PROCESS_MEMORY_COUNTERS*)&memory,sizeof(memory)))
        std::printf("Harness memory after TSF activation: private %.1f MiB, working set %.1f MiB\n",memory.PrivateUsage/1048576.0,memory.WorkingSetSize/1048576.0);
    ComPtr<ITfKeyEventSink> keys; require(SUCCEEDED(service.As(&keys)),14);
    if (!visual) {
        started = Clock::now();
        for (int i=0;i<100;++i) require(SUCCEEDED(keys->OnSetFocus(FALSE)),360);
        std::printf("Idle focus loss (100 calls): %.3f ms\n",std::chrono::duration<double,std::milli>(Clock::now()-started).count());
        BYTE previous[256], keyboard[256]{}; require(GetKeyboardState(previous),361);
        for (int win : {VK_LWIN,VK_RWIN}) {
            keyboard[win]=0x80; require(SetKeyboardState(keyboard),362);
            started = Clock::now();
            for (int i=0;i<100;++i) for (WPARAM key : {WPARAM('R'),WPARAM('E')}) {
                BOOL test=TRUE,eaten=TRUE;
                require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),key,0,&test)) && !test,363);
                require(SUCCEEDED(keys->OnKeyDown(context.Get(),key,0,&eaten)) && !eaten,364);
            }
            std::printf("Win shortcuts (%s, 400 callbacks): %.3f ms\n",win==VK_LWIN?"left":"right",std::chrono::duration<double,std::milli>(Clock::now()-started).count());
            keyboard[win]=0;
        }
        require(SetKeyboardState(previous),365);
    }
    if (cold) {
        // This DLL lives in a fixture without any lexicon or model. Activation,
        // focus loss and shortcuts must succeed even if background loading fails.
        BOOL test=FALSE; require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),'S',0,&test)) && test,366);
        waitPreload();
        BOOL eaten=TRUE; require(SUCCEEDED(keys->OnKeyDown(context.Get(),'S',0,&eaten)) && !eaten,462);
        require((residentStatus() & 0xffffffffULL)==1,463); // No synchronous retry on a key.
        require(SUCCEEDED(service->Deactivate()),367); keys.Reset(); service.Reset();
        thread->SetFocus(nullptr); document->Pop(TF_POPF_ALL); context.Reset(); document.Reset();
        store.Reset(); proxy.Reset(); identifiers.Reset(); thread->Deactivate(); thread.Reset(); factory.Reset();
        auto unload = (HRESULT (WINAPI*)())GetProcAddress(dll,"DllCanUnloadNow"); require(unload && unload()==S_FALSE && liveObjects()==0,368);
        FreeLibrary(dll); CoUninitialize(); std::puts("PASS: activation and system shortcuts require no model or lexicon"); return 0;
    }
    auto press = [&](WPARAM vk, int check) { BOOL test = FALSE, eaten = FALSE; require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),vk,0,&test)) && test,check); require(SUCCEEDED(keys->OnKeyDown(context.Get(),vk,0,&eaten)) && eaten,check+1); };
    // Release a service while its background job may still be running; the next
    // service must acquire that job, not launch a second model load.
    require(SUCCEEDED(service->Deactivate()),464); keys.Reset(); service.Reset();
    require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&service))),465);
    require(SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),466);
    require(SUCCEEDED(service.As(&keys)),467);
    waitPreload();
    require((residentStatus() & 0xffffffffULL)==1,468);
    if (GetProcessMemoryInfo(GetCurrentProcess(),(PROCESS_MEMORY_COUNTERS*)&memory,sizeof(memory)))
        std::printf("Harness memory with resident core: private %.1f MiB, working set %.1f MiB\n",memory.PrivateUsage/1048576.0,memory.WorkingSetSize/1048576.0);
    started = Clock::now();
    for (wchar_t key : std::wstring(L"SU3CL3")) press(key,20);
    std::printf("First composition (6 keys): %.3f ms\n",std::chrono::duration<double,std::milli>(Clock::now()-started).count());
    require(store->text == L"你好",30); require(store->selection.acpStart == 2 && store->selection.acpEnd == 2,31);
    if (!original && !visual) {
        press(VK_HOME,120); press(VK_RIGHT,122); press(VK_DOWN,124);
        require(store->selection.acpStart == 1 && store->selection.acpEnd == 1,126);
        press(VK_ESCAPE,127); press(VK_END,129);
        store->externalCaret(0);
        MSG pending{}; while (PeekMessageW(&pending,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&pending); DispatchMessageW(&pending); }
        press(VK_DOWN,132); require(store->selection.acpStart == 0,134);
        press(VK_ESCAPE,135); press(VK_END,137);
    }
    press(VK_RETURN,32); require(store->text == L"你好",34);
    auto residentLoads = residentStatus() & 0xffffffffULL;
    require(SUCCEEDED(service->Deactivate()),469); keys.Reset(); service.Reset();
    require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&service))),470);
    started = Clock::now();
    require(SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),471);
    require(SUCCEEDED(service.As(&keys)),472);
    press('S',473); press(VK_ESCAPE,475);
    std::printf("Resident reactivation + first key/cancel: %.3f ms\n",std::chrono::duration<double,std::milli>(Clock::now()-started).count());
    require((residentStatus() & 0xffffffffULL)==residentLoads && store->text==L"你好",477);
    press('S',35); press(VK_ESCAPE,37); require(store->text == L"你好",39);
    press('S',40); press('U',42); press('3',44); press(VK_DOWN,46);
    ComPtr<ITfUIElementMgr> ui; require(SUCCEEDED(thread.As(&ui)),48);
    ComPtr<IEnumTfUIElements> elements; require(SUCCEEDED(ui->EnumUIElements(&elements)),49);
    ComPtr<ITfUIElement> element; ULONG fetched = 0; require(elements->Next(1,&element,&fetched) == S_OK && fetched,50);
    ComPtr<ITfCandidateListUIElement> candidates; require(SUCCEEDED(element.As(&candidates)),51);
    UINT count = 0; candidates->GetCount(&count); require(count > 0 && count <= 9,52);
    if (!visual && !original) {
        press(VK_DOWN,70); UINT selection = 0; candidates->GetSelection(&selection); require(selection == 1,72);
        press(VK_RETURN,73); require(store->text == L"你好妳",75);
        ComPtr<IEnumTfUIElements> afterEnter; ui->EnumUIElements(&afterEnter);
        ComPtr<ITfUIElement> remaining; ULONG remainingCount = 0;
        require(afterEnter->Next(1,&remaining,&remainingCount) == S_FALSE && !remainingCount,76);
        BOOL enterAgain = TRUE; keys->OnTestKeyDown(context.Get(),VK_RETURN,0,&enterAgain); require(!enterAgain,77);
        press('S',78); press('U',80); press('3',82); press(VK_DOWN,84); press('2',86);
        require(store->text == L"你好妳妳",88); press(VK_RETURN,89);
        press(VK_SHIFT,91); BOOL eaten = FALSE;
        require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_SHIFT,0,&eaten)) && eaten,93);
        BOOL test = TRUE; keys->OnTestKeyDown(context.Get(),'S',0,&test); require(!test,94);
        press(VK_SHIFT,95); require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_SHIFT,0,&eaten)) && eaten,97);
        press('S',98); press(VK_ESCAPE,100);
        press(VK_LSHIFT,140); require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_LSHIFT,0,&eaten)) && eaten,142);
        keys->OnTestKeyDown(context.Get(),'S',0,&test); require(!test,143);
        press(VK_RSHIFT,144); require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_RSHIFT,0,&eaten)) && eaten,146);
        keys->OnTestKeyDown(context.Get(),'S',0,&test); require(test,147);
        auto noCandidates = [&] {
            ComPtr<IEnumTfUIElements> active; ui->EnumUIElements(&active);
            ComPtr<ITfUIElement> item; ULONG found = 0;
            return active->Next(1,&item,&found) == S_FALSE && !found;
        };
        press('S',180); press('U',182); press(VK_SPACE,184);
        require(noCandidates(),186); // First-tone space completes reading without opening a list.
        press(VK_ESCAPE,187);
        press('S',150); press('U',152); press('3',154);
        BOOL passed = TRUE;
        require(SUCCEEDED(keys->OnKeyDown(context.Get(),VK_SPACE,0,&passed)) && passed && !store->text.empty() && store->text.back() != L' ',156);
        require(noCandidates(),157); // Space commits under Down; no unsolicited association.
        wchar_t local[32768]; GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768);
        auto profile = std::wstring(local) + L"/Shanjie/profile.ini";
        require(WritePrivateProfileStringW(L"candidates",L"open",L"space",profile.c_str()),158);
        press('S',370); press('U',372); press('3',374); press(VK_SPACE,376);
        require(noCandidates(),378); // Direct INI edits have no per-key polling.
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press('S',159); press('U',161); press('3',163); press(VK_SPACE,165);
        require(!noCandidates(),167); // The same service sees the saved trigger without reactivation.
        press(VK_ESCAPE,168);
        require(WritePrivateProfileStringW(L"candidates",L"open",L"down",profile.c_str()),170);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press(VK_DOWN,171); require(!noCandidates(),173);
        ComPtr<ITfThreadFocusSink> focus; require(SUCCEEDED(service.As(&focus)),174);
        std::wstring folder = std::wstring(local) + L"\\Shanjie";
        CharUpperBuffW(folder.data(),(DWORD)folder.size()); std::replace(folder.begin(),folder.end(),L'\\',L'_');
        auto mutexName = L"Local\\ShanjieLearning-" + folder;
        HANDLE ready = CreateEventW(nullptr,TRUE,FALSE,nullptr), release = CreateEventW(nullptr,TRUE,FALSE,nullptr);
        require(ready && release,393);
        std::thread writer([&] {
            HANDLE mutex = CreateMutexW(nullptr,FALSE,mutexName.c_str());
            require(mutex && GetLastError()==ERROR_ALREADY_EXISTS,395);
            require(mutex && WaitForSingleObject(mutex,INFINITE)==WAIT_OBJECT_0,394);
            SetEvent(ready); WaitForSingleObject(release,INFINITE); ReleaseMutex(mutex); CloseHandle(mutex);
        });
        WaitForSingleObject(ready,INFINITE); auto beforeFocus = store->text; started = Clock::now();
        HRESULT lostFocus = focus->OnKillThreadFocus();
        double focusMs = std::chrono::duration<double,std::milli>(Clock::now()-started).count();
        SetEvent(release); writer.join(); CloseHandle(ready); CloseHandle(release);
        std::printf("Focus commit while learning writer holds mutex: %.3f ms\n",focusMs);
        require(SUCCEEDED(lostFocus) && noCandidates() && store->text==beforeFocus && focusMs<200,175);
        press('S',190); press('U',192); press('3',194);
        auto beforeReload = store->text;
        require(WritePrivateProfileStringW(L"engine",L"ranking",L"formal",profile.c_str()),196);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        require(store->text == beforeReload && noCandidates(),197); // Engine replacement keeps pending text.
        require(WritePrivateProfileStringW(L"engine",L"model",L"2",profile.c_str()),198);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press('S',200); press('U',202); press('3',204); press(VK_DOWN,206); press(VK_RETURN,208);
        require(noCandidates(),210);
        require(WritePrivateProfileStringW(L"engine",L"model",L"1",profile.c_str()),211);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press('G',212); press('4',214); press(VK_DOWN,216); press(VK_RIGHT,218);
        UINT tableCount = 0; candidates->GetCount(&tableCount); require(tableCount > 0 && tableCount <= 9,220);
        press(VK_UP,221); UINT afterUp = 0; candidates->GetCount(&afterUp); require(afterUp == tableCount,223);
        press(VK_ESCAPE,224); press(VK_RETURN,226);
        press('S',244); press('U',246); press('3',248); press(VK_DOWN,250); press('1',252);
        require(noCandidates(),254); // Windows defaults do not prompt associated phrases.
        press(VK_RETURN,258);
        require(WritePrivateProfileStringW(L"input",L"associations",L"1",profile.c_str()),260);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press('S',262); press('U',264); press('3',266); press(VK_DOWN,268); press('1',270);
        require(!noCandidates(),272);
        require(WritePrivateProfileStringW(L"input",L"associations",L"0",profile.c_str()),228);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        require(noCandidates(),255); press(VK_RETURN,256);
        press('S',230); press('U',232); press('3',234); press(VK_DOWN,236); press('1',238);
        require(noCandidates(),240); press(VK_RETURN,241);
        for (const auto* model : {L"3",L"4"}) {
            require(WritePrivateProfileStringW(L"engine",L"model",model,profile.c_str()),280);
            SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
            press('S',282); require(noCandidates(),284);
            require(WritePrivateProfileStringW(L"engine",L"prediction",L"1",profile.c_str()),285);
            SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
            require(!noCandidates(),286); press(VK_TAB,287);
            require(WritePrivateProfileStringW(L"engine",L"prediction",L"0",profile.c_str()),289);
            SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
            require(noCandidates(),290); press(VK_ESCAPE,291);
            press('S',293); press('U',295); press('3',297); press(VK_DOWN,299); press(VK_RETURN,301);
            require(noCandidates(),303);
        }
        require(WritePrivateProfileStringW(L"candidates",L"enter_commits",L"0",profile.c_str()),310);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press('S',311); press('U',313); press('3',315); press(VK_DOWN,317); press(VK_RETURN,319);
        BOOL enterReady = FALSE; keys->OnTestKeyDown(context.Get(),VK_RETURN,0,&enterReady); require(enterReady,321);
        press(VK_RETURN,322);
        require(WritePrivateProfileStringW(L"input",L"space_after_commit",L"0",profile.c_str()),324);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press('S',325); press('U',327); press('3',329); press(VK_SPACE,331); require(noCandidates(),333);
        require(WritePrivateProfileStringW(L"input",L"keyboard",L"3",profile.c_str()),334);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        press('Z',335); press('H',337); press('U',339); auto beforePinyinReload = store->text;
        require(WritePrivateProfileStringW(L"engine",L"model",L"3",profile.c_str()),341);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        require(store->text == beforePinyinReload && noCandidates(),342);
        require(WritePrivateProfileStringW(L"input",L"keyboard",L"1",profile.c_str()),380);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.Restart.v1"),0,0);
        require(store->text == beforePinyinReload && noCandidates(),381);
        press('S',382); press('U',384); press('3',386); press(VK_DOWN,388); press(VK_RETURN,390);
        require(noCandidates(),392); // Explicit restart reloads the directly edited keyboard and preserves pending pinyin.
        require(WritePrivateProfileStringW(L"input",L"mixed_input",L"1",profile.c_str()),400);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        auto beforeSmart = store->text;
        auto smartCaret = store->selection.acpStart;
        press('C',402); press('A',404); press('L',406); press('L',408);
        press('S',410); press('U',412); press('3',414); press(VK_RETURN,416);
        beforeSmart.insert(smartCaret,L"call你");
        require(store->text == beforeSmart,418);
        auto beforeRaw = store->text; auto rawCaret = store->selection.acpStart;
        press('G',420); press('J',422);
        beforeRaw.insert(rawCaret,L"gj"); require(store->text == beforeRaw && !noCandidates(),428);
        press(VK_RETURN,434); require(store->text == beforeRaw,435);
        require(noCandidates(),436);
        press('H',438); press('E',440); press('L',442); press('L',444); press('O',446);
        auto beforeSmartOff = store->text;
        require(WritePrivateProfileStringW(L"input",L"mixed_input",L"0",profile.c_str()),448);
        SendMessageW(HWND_BROADCAST,RegisterWindowMessageW(L"Shanjie.ProfileChanged.v1"),0,0);
        require(store->text == beforeSmartOff && noCandidates(),450);
        press('S',452); require(noCandidates(),454); press(VK_ESCAPE,456);
        std::puts("Mixed input: raw English with reading hint, tone composition, live off preserves English passed");
    }
    if (original) {
        press(VK_RIGHT,110); UINT selection = 0; candidates->GetSelection(&selection); require(selection == 1,112);
        press(VK_ESCAPE,113); press(VK_ESCAPE,115);
        BOOL test = TRUE; keys->OnTestKeyDown(context.Get(),VK_SHIFT,0,&test); require(!test,117);
        std::puts("Original Shanjie profile: horizontal navigation and unchanged Shift passed");
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
    auto unload = (HRESULT (WINAPI*)())GetProcAddress(dll,"DllCanUnloadNow"); require(unload && unload() == S_FALSE && liveObjects()==0,55);
    FreeLibrary(dll); CoUninitialize(); std::puts("TSF composition, caret, commit, cancel, candidates, and COM lifetime passed");
}
