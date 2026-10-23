// Private-desktop popup lifecycle regression. Focus forwarding is an explicit
// reentrancy model, not a captured Explorer stack. No registration or desktop input.
#define wmain previous_smoke_main
#include "tsf_smoke.cpp"
#undef wmain
void pump() { MSG m; while(PeekMessageW(&m,nullptr,0,0,PM_REMOVE)){TranslateMessage(&m);DispatchMessageW(&m);} }
HWND panel() { HWND window=nullptr; while((window=FindWindowExW(nullptr,window,L"Shanjie.AddWord.Popup.v1",nullptr))) {
    DWORD pid=0;GetWindowThreadProcessId(window,&pid);if(pid==GetCurrentProcessId())return window;
} return nullptr; }
COLORREF background(HWND window) {
    auto dc=CreateCompatibleDC(nullptr);auto source=GetDC(window);auto bitmap=CreateCompatibleBitmap(source,16,16);ReleaseDC(window,source);
    auto old=SelectObject(dc,bitmap);SendMessageW(window,WM_PRINT,(WPARAM)dc,PRF_CLIENT|PRF_ERASEBKGND);auto color=GetPixel(dc,8,8);
    SelectObject(dc,old);DeleteObject(bitmap);DeleteDC(dc);return color;
}
// ThreadProxy deliberately substitutes TSF's key-focus sink registration.
// Forward only our private controls' real focus-loss messages to that sink.
// This exercises reentrant TSF callbacks without registering or loading a TIP
// in the user's desktop. It is a focused simulation, not a live Explorer stack.
ITfKeyEventSink* probeKeys=nullptr;
int reentries=0;bool deactivated=false;
WNDPROC editProcedure=nullptr,buttonProcedure=nullptr,popupProcedure=nullptr;
LRESULT CALLBACK probePopup(HWND w,UINT m,WPARAM a,LPARAM b) {
    // A host can report TSF focus loss synchronously while Windows hides a
    // panel whose button has taken focus. Forward that host notification only
    // in this specific private focus state, never on the user's desktop.
    auto focused=GetFocus();
    if(m==WM_SHOWWINDOW&&!a&&probeKeys&&(focused==w||IsChild(w,focused))){
        ++reentries;std::printf("NATIVE hide with panel focus -> reentrant TSF focus FALSE, focused=%p\n",focused);std::fflush(stdout);
        auto hr=probeKeys->OnSetFocus(FALSE);std::printf("HIDE callback returned %08lx\n",hr);std::fflush(stdout);
    }
    return CallWindowProcW(popupProcedure,w,m,a,b);
}
LRESULT CALLBACK probeEdit(HWND w,UINT m,WPARAM a,LPARAM b) {
    if(m==WM_KILLFOCUS&&probeKeys){
        std::printf("EDIT focus loss -> TSF key focus FALSE, next=%p\n",(HWND)a);std::fflush(stdout);
        auto hr=probeKeys->OnSetFocus(FALSE);std::printf("EDIT callback returned %08lx\n",hr);std::fflush(stdout);
    }
    return CallWindowProcW(editProcedure,w,m,a,b);
}
LRESULT CALLBACK probeButton(HWND w,UINT m,WPARAM a,LPARAM b) {
    if(m==WM_KILLFOCUS&&probeKeys){
        std::printf("BUTTON focus loss -> TSF key focus FALSE, next=%p\n",(HWND)a);std::fflush(stdout);
        auto hr=probeKeys->OnSetFocus(FALSE);std::printf("BUTTON callback returned %08lx\n",hr);std::fflush(stdout);
    }
    return CallWindowProcW(buttonProcedure,w,m,a,b);
}
int wmain(int argc,wchar_t**argv) {
    require(argc==3,900);std::wstring scenario=argv[2];
    require(GetEnvironmentVariableW(L"SHANJIE_HEADLESS",nullptr,0)>0,901);

    auto desktopName=L"Shanjie.Popup.Cancel."+std::to_wstring(GetCurrentProcessId());
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

    auto edit=CreateWindowW(L"EDIT",L"回家",WS_CHILD|WS_VISIBLE,-12000,-12000,250,40,store->window,nullptr,wc.hInstance,nullptr);
    require(edit!=nullptr,981);
    ComPtr<ITfDocumentMgr> previous;
    require(SUCCEEDED(thread->AssociateFocus(edit,doc.Get(),&previous)),982);
    SetFocus(edit);pump();
    ComPtr<ITfClientId> ids;thread.As(&ids);TfClientId serviceClient;require(SUCCEEDED(ids->GetClientId(ServiceId,&serviceClient)),912);
    ComPtr<ThreadProxy> proxy;proxy.Attach(new ThreadProxy(thread.Get(),true));
    require(SUCCEEDED(tip->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),913);
    ComPtr<ITfKeyEventSink> keys;tip.As(&keys);
    probeKeys=keys.Get();
    editProcedure=(WNDPROC)SetWindowLongPtrW(edit,GWLP_WNDPROC,(LONG_PTR)probeEdit);ComPtr<ITfThreadMgrEventSink> focus;tip.As(&focus);
    ComPtr<ITfCompartmentMgr> mgr;proxy.As(&mgr);ComPtr<ITfCompartment> mode;mgr->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&mode);
    VARIANT v{};v.vt=VT_I4;v.lVal=1;require(SUCCEEDED(mode->SetValue(serviceClient,&v)),914);
    auto chinese=[&]{VARIANT v{};require(SUCCEEDED(mode->GetValue(&v)),915);bool on=v.vt==VT_I4&&v.lVal;VariantClear(&v);return on;};
    auto keyboard=[&](int key,int extra=0){BYTE state[256]{};if(key)state[key]=0x80;if(key==VK_RSHIFT||key==VK_LSHIFT)state[VK_SHIFT]=0x80;if(key==VK_LCONTROL)state[VK_CONTROL]=0x80;if(extra)state[extra]=0x80;require(SetKeyboardState(state),916);};
    auto flags=[](int key,bool up){return LPARAM(1)|(LPARAM(MapVirtualKeyW(key,MAPVK_VK_TO_VSC))<<16)|(up?(LPARAM(3)<<30):0);};
    auto down=[&](int key){BOOL test=FALSE,eaten=FALSE;auto f=flags(key,false);require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),key,f,&test)),917);if(test)require(SUCCEEDED(keys->OnKeyDown(context.Get(),key,f,&eaten)),918);};
    auto up=[&](int key){BOOL e=FALSE;auto f=flags(key,true);require(SUCCEEDED(keys->OnTestKeyUp(context.Get(),key,f,&e)),919);require(SUCCEEDED(keys->OnKeyUp(context.Get(),key,f,&e)),920);};
    auto compositions=[&]{ComPtr<ITfContextComposition> c;context.As(&c);ComPtr<IEnumITfCompositionView> e;c->EnumCompositions(&e);ComPtr<ITfCompositionView> first;ULONG count=0;e->Next(1,&first,&count);return count;};
    {
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
        bool expected=scenario==L"word-deferred"||scenario==L"word-natural"||scenario.starts_with(L"cancel-");
        while(std::chrono::steady_clock::now()<end){pump();if(expected&&panel()&&IsWindowVisible(panel()))break;Sleep(10);}
        require((panel()&&IsWindowVisible(panel()))==expected,961);

        if(expected){
            auto popup=panel();auto cancel=GetDlgItem(popup,11);auto add=GetDlgItem(popup,10);auto originalFocus=GetFocus();
            require(cancel&&add&&originalFocus==edit,983);
            popupProcedure=(WNDPROC)SetWindowLongPtrW(popup,GWLP_WNDPROC,(LONG_PTR)probePopup);
            buttonProcedure=(WNDPROC)SetWindowLongPtrW(cancel,GWLP_WNDPROC,(LONG_PTR)probeButton);
            auto mouse=[&](HWND target,int x=8,int y=8){SendMessageW(target,WM_LBUTTONDOWN,MK_LBUTTON,MAKELPARAM(8,8));require(GetFocus()==originalFocus,990);SendMessageW(target,WM_LBUTTONUP,0,MAKELPARAM(x,y));};
            auto wait=[&](auto predicate){auto until=std::chrono::steady_clock::now()+std::chrono::seconds(12);while(!predicate()){if(std::chrono::steady_clock::now()>=until){
                    for(int id:{3,4,5}){wchar_t value[1024]{};GetWindowTextW(GetDlgItem(popup,id),value,1024);char utf8[4096]{};WideCharToMultiByte(CP_UTF8,0,value,-1,utf8,4096,nullptr,nullptr);std::printf("CONTROL %d %s\n",id,utf8);}
                    require(false,991);
                }pump();Sleep(10);}};
            auto text=[&](HWND w){std::wstring value(GetWindowTextLengthW(w)+1,0);auto count=GetWindowTextW(w,value.data(),(int)value.size());value.resize(count);return value;};
            if(scenario==L"cancel-theme-dark"||scenario==L"cancel-theme-light") {
                auto expectedColor=scenario==L"cancel-theme-dark"?RGB(32,32,32):RGB(255,255,255);
                require(background(popup)==expectedColor&&background(add)==expectedColor,1002);
            }
            if(scenario==L"cancel-escape")down(VK_ESCAPE);
            else if(scenario==L"cancel-command")SendMessageW(popup,WM_COMMAND,MAKEWPARAM(11,BN_CLICKED),(LPARAM)cancel);
            else if(scenario==L"cancel-reentry") {SetFocus(cancel);require(GetFocus()==cancel,992);}
            else if(scenario==L"cancel-focus-notification") {
                SendMessageW(popup,WM_COMMAND,MAKEWPARAM(11,BN_KILLFOCUS),(LPARAM)cancel);
                SendMessageW(popup,WM_COMMAND,MAKEWPARAM(11,BN_CLICKED),(LPARAM)add);pump();
                require(IsWindowVisible(popup),993);down(VK_ESCAPE);
            } else if(scenario==L"cancel-outside") {mouse(cancel,999,999);pump();require(IsWindowVisible(popup),994);down(VK_ESCAPE);}
            else if(scenario==L"cancel-deactivate") {require(SUCCEEDED(tip->Deactivate()),995);deactivated=true;}
            else if(scenario==L"cancel-owner-destroy")DestroyWindow(store->window);
            else if(scenario==L"cancel-keyboard-reading") {
                keyboard(VK_LCONTROL);down('A');keyboard(0);
                for(int key:std::initializer_list<int>{'C','J','O','6','R','U','8',VK_SPACE})down(key);
                pump();require(text(GetDlgItem(popup,3))==L"ㄏㄨㄟˊ-ㄐㄧㄚ-",996);down(VK_ESCAPE);
            } else if(scenario==L"cancel-save") {
                wait([&]{return text(GetDlgItem(popup,5)).find(L"內建詞庫：")!=std::wstring::npos;});
                require(text(GetDlgItem(popup,3)).find(L"ㄏ")!=std::wstring::npos,997);mouse(add);
                wait([&]{return !IsWindowVisible(popup);});
                wchar_t local[32768];GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768);
                bool found=false;
                WIN32_FIND_DATAW file{};auto pattern=std::wstring(local)+L"\\Shanjie\\dictionary-*.tsv";
                auto search=FindFirstFileW(pattern.c_str(),&file);if(search!=INVALID_HANDLE_VALUE){found=true;FindClose(search);}require(found,998);
            } else if(scenario==L"cancel-stale-notify") {
                SendMessageW(popup,WM_COMMAND,MAKEWPARAM(11,BN_CLICKED),(LPARAM)cancel);
                for(int tap=0;tap<2;tap++){keyboard(VK_LCONTROL);down(VK_LCONTROL);keyboard(0);up(VK_LCONTROL);}
                wait([&]{auto current=panel();return current&&current!=popup&&IsWindowVisible(current);});
                down(VK_ESCAPE);
            } else {mouse(cancel);if(scenario==L"cancel-double"){mouse(cancel);}}
            pump();
            require(!IsWindowVisible(popup),984);
            if(scenario==L"cancel-reentry")require(reentries>0,999);
            // Closing then reopening must leave the command path and native state reusable.
            if(scenario==L"cancel-mouse") {
                for(int tap=0;tap<2;tap++){keyboard(VK_LCONTROL);down(VK_LCONTROL);keyboard(0);up(VK_LCONTROL);}
                wait([&]{return panel()&&IsWindowVisible(panel());});down(VK_ESCAPE);pump();require(!panel(),1003);
            }
            if(scenario==L"cancel-late"){Sleep(700);pump();require(!panel(),1000);}
            if(scenario!=L"cancel-save") {
                wchar_t local[32768];GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768);WIN32_FIND_DATAW file{};
                auto pattern=std::wstring(local)+L"\\Shanjie\\dictionary-*.tsv";auto search=FindFirstFileW(pattern.c_str(),&file);
                require(search==INVALID_HANDLE_VALUE,1001);if(search!=INVALID_HANDLE_VALUE)FindClose(search);
            }
            std::printf("LIFECYCLE completed, reentries=%d\n",reentries);
        }
        require(store->text==(scenario==L"word-invalid"?L"ab":L"回家"),964);
        if(password){store->window=savedWindow;DestroyWindow(password);}
    }
    probeKeys=nullptr;keyboard(0);if(!deactivated)require(SUCCEEDED(tip->Deactivate()),970);thread->SetFocus(nullptr);doc->Pop(TF_POPF_ALL);thread->Deactivate();DestroyWindow(store->window);
    std::printf("PASS %ls\n",scenario.c_str());return 0;
}
