// Private real-TSF fixture: no registration, foreground activation or desktop input.
#define wmain previous_smoke_main
#include "tsf_smoke.cpp"
#undef wmain
void pump() { MSG m; while(PeekMessageW(&m,nullptr,0,0,PM_REMOVE)){TranslateMessage(&m);DispatchMessageW(&m);} }
HWND panel() { HWND window=nullptr; while((window=FindWindowExW(nullptr,window,L"Shanjie.AddWord.Popup.v1",nullptr))) {
    DWORD pid=0;GetWindowThreadProcessId(window,&pid);if(pid==GetCurrentProcessId())return window;
} return nullptr; }
int wmain(int argc,wchar_t**argv) {
    require(argc==3,900);std::wstring scenario=argv[2];
    require(GetEnvironmentVariableW(L"SHANJIE_HEADLESS",nullptr,0)>0,901);
    // Isolate from the user's resident helper. A ready helper correctly disables
    // the legacy TSF gesture on its own desktop; that is not a popup regression.
    auto desktopName=L"Shanjie.Compat."+std::to_wstring(GetCurrentProcessId());
    auto desktop=CreateDesktopW(desktopName.c_str(),nullptr,nullptr,0,GENERIC_ALL,nullptr);
    require(desktop!=nullptr&&SetThreadDesktop(desktop),980);
    std::puts("PRIVATE DESKTOP ready (never switched)");std::fflush(stdout);
    require(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)),902);
    auto dll=LoadLibraryW(argv[1]);require(dll!=nullptr,903);
    auto get=(HRESULT(WINAPI*)(REFCLSID,REFIID,void**))GetProcAddress(dll,"DllGetClassObject");require(get!=nullptr,904);
    ComPtr<IClassFactory> factory;require(SUCCEEDED(get(ServiceId,IID_PPV_ARGS(&factory))),905);
    ComPtr<ITfTextInputProcessorEx> tip;require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&tip))),906);
    ComPtr<ITfThreadMgr> thread;require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&thread))),907);
    ComPtr<ITfThreadMgrEx> extended;thread.As(&extended);TfClientId client=0;
    require(SUCCEEDED(extended->ActivateEx(&client,TF_TMAE_NOACTIVATETIP|TF_TMAE_NOACTIVATEKEYBOARDLAYOUT)),908);
    ComPtr<ITfDocumentMgr> doc;require(SUCCEEDED(thread->CreateDocumentMgr(&doc)),909);
    ComPtr<Store> store;store.Attach(new Store);store->layoutReady=true;store->readOnly=scenario==L"word-readonly";
    WNDCLASSW wc{};wc.lpfnWndProc=DefWindowProcW;wc.hInstance=GetModuleHandleW(nullptr);wc.lpszClassName=L"Shanjie.Compat0122.Test";RegisterClassW(&wc);
    store->window=CreateWindowW(wc.lpszClassName,L"Private fixture",WS_OVERLAPPEDWINDOW,-12000,-12000,800,400,nullptr,nullptr,wc.hInstance,nullptr);
    require(store->window!=nullptr,910);ShowWindow(store->window,SW_SHOWNOACTIVATE);
    ComPtr<ITfContext> context;TfEditCookie cookie;
    require(SUCCEEDED(doc->CreateContext(client,0,store.Get(),&context,&cookie))&&SUCCEEDED(doc->Push(context.Get()))&&SUCCEEDED(thread->SetFocus(doc.Get())),911);
    ComPtr<ITfClientId> ids;thread.As(&ids);TfClientId serviceClient;require(SUCCEEDED(ids->GetClientId(ServiceId,&serviceClient)),912);
    ComPtr<ThreadProxy> proxy;proxy.Attach(new ThreadProxy(thread.Get(),true));
    require(SUCCEEDED(tip->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),913);
    ComPtr<ITfKeyEventSink> keys;tip.As(&keys);ComPtr<ITfThreadMgrEventSink> focus;tip.As(&focus);
    ComPtr<ITfCompartmentMgr> mgr;proxy.As(&mgr);ComPtr<ITfCompartment> mode;mgr->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&mode);
    VARIANT v{};v.vt=VT_I4;v.lVal=1;require(SUCCEEDED(mode->SetValue(serviceClient,&v)),914);
    auto chinese=[&]{VARIANT v{};require(SUCCEEDED(mode->GetValue(&v)),915);bool on=v.vt==VT_I4&&v.lVal;VariantClear(&v);return on;};
    auto keyboard=[&](int key,int extra=0){BYTE state[256]{};if(key)state[key]=0x80;if(key==VK_RSHIFT||key==VK_LSHIFT)state[VK_SHIFT]=0x80;if(key==VK_LCONTROL)state[VK_CONTROL]=0x80;if(extra)state[extra]=0x80;require(SetKeyboardState(state),916);};
    auto flags=[](int key,bool up){return LPARAM(1)|(LPARAM(MapVirtualKeyW(key,MAPVK_VK_TO_VSC))<<16)|(up?(LPARAM(3)<<30):0);};
    auto down=[&](int key){BOOL test=FALSE,eaten=FALSE;auto f=flags(key,false);require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),key,f,&test)),917);if(test)require(SUCCEEDED(keys->OnKeyDown(context.Get(),key,f,&eaten)),918);};
    auto up=[&](int key){BOOL e=FALSE;auto f=flags(key,true);require(SUCCEEDED(keys->OnTestKeyUp(context.Get(),key,f,&e)),919);require(SUCCEEDED(keys->OnKeyUp(context.Get(),key,f,&e)),920);};
    auto compositions=[&]{ComPtr<ITfContextComposition> c;context.As(&c);ComPtr<IEnumITfCompositionView> e;c->EnumCompositions(&e);ComPtr<ITfCompositionView> first;ULONG count=0;e->Next(1,&first,&count);return count;};
    if(scenario.starts_with(L"shift-")) {
        int side=scenario==L"shift-left"?VK_LSHIFT:VK_RSHIFT;
        keyboard(side);down(side);
        if(scenario==L"shift-enter"){keyboard(side,VK_RETURN);down(VK_RETURN);keyboard(side);up(VK_RETURN);}
        int proxyKey=scenario==L"shift-unknown"?0:scenario==L"shift-proxy-enter"?VK_RETURN:side;
        for(int i=0;i<3;i++){auto f=proxyKey?flags(proxyKey,i==2):0;require(PostMessageW(store->window,i==2?WM_KEYUP:WM_KEYDOWN,VK_PROCESSKEY,f),921);pump();}
        keyboard(0);up(side);
        bool blocked=scenario==L"shift-unknown"||scenario==L"shift-proxy-enter"||scenario==L"shift-enter";
        require(chinese()==blocked,922);require(store->text.empty(),923);
    } else if(scenario.starts_with(L"handoff-")) {
        keyboard(0);down('1');require(store->text==L"1"&&compositions()==1,930);
        require(SUCCEEDED(focus->OnSetFocus(nullptr,doc.Get())),931);require(compositions()==1,932);
        if(scenario==L"handoff-timeout") { require(SUCCEEDED(thread->SetFocus(nullptr)),933);Sleep(180);pump();require(compositions()==0,934); }
        else if(scenario==L"handoff-generation") {keys->OnSetFocus(FALSE);focus->OnSetFocus(doc.Get(),nullptr);require(compositions()==0,935);}
        else if(scenario==L"handoff-context") {
            ComPtr<Store> next;next.Attach(new Store);next->window=store->window;ComPtr<ITfContext> top;TfEditCookie c;
            require(SUCCEEDED(doc->CreateContext(client,0,next.Get(),&top,&c))&&SUCCEEDED(doc->Push(top.Get())),936);
            focus->OnSetFocus(doc.Get(),nullptr);require(compositions()==0,937);doc->Pop(0);
        } else if(scenario==L"handoff-different") {
            ComPtr<ITfDocumentMgr> other;thread->CreateDocumentMgr(&other);focus->OnSetFocus(other.Get(),nullptr);require(compositions()==0,938);
        } else {
            Sleep(10);require(SUCCEEDED(focus->OnSetFocus(doc.Get(),nullptr)),939);require(compositions()==1,940);
            down('2');down('3');require(store->text==L"123"&&compositions()==1,941);
            down(VK_RETURN);require(store->text==L"123"&&compositions()==0,942);
        }
    } else if(scenario==L"cmd-commit") {
        keyboard(0);for(int key:{'A','D','B'})down(key);require(store->text==L"adb",950);down(VK_RETURN);require(store->text==L"adb"&&compositions()==0,951);
    } else {
        store->text=L"回家";store->selection.acpStart=store->selection.acpEnd=0;
        if(scenario==L"word-readonly")store->readOnly=true;
        HWND savedWindow=store->window,password=nullptr;
        if(scenario==L"word-password") {password=CreateWindowW(L"EDIT",L"",WS_POPUP|ES_PASSWORD,-12000,-12000,200,30,nullptr,nullptr,wc.hInstance,nullptr);require(password!=nullptr,960);store->window=password;}
        for(int tap=0;tap<2;tap++) {
            keyboard(VK_LCONTROL);down(VK_LCONTROL);
            if(scenario==L"word-natural") {
                for(int i=0;i<2;i++){PostMessageW(store->window,WM_KEYDOWN,VK_PROCESSKEY,flags(VK_LCONTROL,false)|(LPARAM(1)<<30));pump();}
                Sleep(tap?125:60);
            }
            keyboard(0);up(VK_LCONTROL);
            if(tap==0&&scenario==L"word-natural")Sleep(361);
        }
        // The selection becomes available only after the last TSF key callback.
        store->selection.acpStart=0;store->selection.acpEnd=scenario==L"word-empty"?0:2;
        if(scenario==L"word-invalid")store->text=L"ab";
        if(scenario==L"word-stale")keys->OnSetFocus(FALSE);
        auto end=std::chrono::steady_clock::now()+std::chrono::milliseconds(1700);
        bool expected=scenario==L"word-deferred"||scenario==L"word-natural";
        while(std::chrono::steady_clock::now()<end){pump();if(expected&&panel()&&IsWindowVisible(panel()))break;Sleep(10);}
        require((panel()&&IsWindowVisible(panel()))==expected,961);
        if(expected){require(GetWindow(panel(),GW_OWNER)==savedWindow,962);store->externalCaret(2);pump();require(!IsWindowVisible(panel()),963);}
        require(store->text==(scenario==L"word-invalid"?L"ab":L"回家"),964);
        if(password){store->window=savedWindow;DestroyWindow(password);}
    }
    keyboard(0);require(SUCCEEDED(tip->Deactivate()),970);thread->SetFocus(nullptr);doc->Pop(TF_POPF_ALL);thread->Deactivate();DestroyWindow(store->window);
    std::printf("PASS %ls\n",scenario.c_str());return 0;
}
