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
    ComPtr<Store> store;store.Attach(new Store);store->layoutReady=true;
    if(scenario.starts_with(L"privacy-")) {
        store->text=L"隔離測試前文";store->selection.acpStart=store->selection.acpEnd=LONG(store->text.size());store->privacyBoundary=LONG(store->text.size());
        if(scenario==L"privacy-private")store->inputScope=IS_PRIVATE;
        if(scenario==L"privacy-password")store->inputScope=IS_PASSWORD;
        if(scenario==L"privacy-pin")store->inputScope=IS_NUMERIC_PIN;
    }
    store->readOnly=scenario==L"word-readonly"||scenario.starts_with(L"status-readonly");
    if(scenario.ends_with(L"-disjoint")) {
        store->staticFlags=TS_SS_DISJOINTSEL|TS_SS_REGIONS|TS_SS_TKBAUTOCORRECTENABLE|TS_SS_TKBPREDICTIONENABLE;
    } else if(scenario==L"status-writable-regions") {
        store->staticFlags=TS_SS_REGIONS|TS_SS_TKBAUTOCORRECTENABLE;
    }
    WNDCLASSW wc{};wc.lpfnWndProc=DefWindowProcW;wc.hInstance=GetModuleHandleW(nullptr);wc.lpszClassName=L"Shanjie.Compat0122.Test";RegisterClassW(&wc);
    store->window=CreateWindowW(wc.lpszClassName,L"Private fixture",WS_OVERLAPPEDWINDOW,-12000,-12000,800,400,nullptr,nullptr,wc.hInstance,nullptr);
    require(store->window!=nullptr,910);ShowWindow(store->window,SW_SHOWNOACTIVATE);
    HWND passwordOwner=nullptr;
    if(scenario==L"privacy-native-password"){
        passwordOwner=store->window;
        store->window=CreateWindowW(L"EDIT",L"",WS_CHILD|WS_VISIBLE|ES_PASSWORD,0,0,100,24,passwordOwner,nullptr,wc.hInstance,nullptr);
        require(store->window!=nullptr,1130);SetFocus(store->window);require(GetFocus()==store->window,1131);
    }
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
    if(scenario.starts_with(L"shift-retain-") || scenario==L"shift-finish-eten") {
        auto tapShift=[&]{
            keyboard(VK_RSHIFT);down(VK_RSHIFT);
            keyboard(0);up(VK_RSHIFT);pump();
        };
        keyboard(0);
        if(scenario==L"shift-finish-eten") {
            for(int key:{'N','E','3','N'})down(key);
            require(store->text.starts_with(L"你")&&store->text!=L"你"&&compositions()==1,1160);
            tapShift();
            require(!chinese()&&store->text==L"你"&&compositions()==0,1161);
            tapShift();
            down('N');require(compositions()==1,1162);
            tapShift();
            require(!chinese()&&store->text==L"你"&&compositions()==0,1163);
        } else {
            for(int key:{'S','U','3','2','8'})down(key);
            require(store->text.starts_with(L"你")&&store->text!=L"你"&&compositions()==1,1164);
            tapShift();
            require(!chinese()&&store->text==L"你"&&compositions()==1,1165);
            keyboard(VK_SHIFT);down('G');keyboard(0);
            for(int key:{'I','T','H','U','B','2'})down(key);
            keyboard(VK_SHIFT);down('1');keyboard(0);
            require(store->text==L"你Github2!"&&compositions()==1,1166);
            keyboard(VK_LCONTROL);
            BOOL offered=TRUE;
            require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),'A',flags('A',false),&offered))&&!offered,1167);
            keyboard(0);tapShift();
            require(chinese()&&store->text==L"你Github2!"&&compositions()==1,1168);
            for(int key:{'C','L','3'})down(key);
            require(store->text==L"你Github2!好"&&compositions()==1,1169);
            down(VK_RETURN);
            auto expected=scenario==L"shift-retain-shanjie"?L"你 Github2!好":L"你Github2!好";
            require(store->text==expected&&compositions()==0,1170);

            store->externalText(L"");
            down('2');down('8');require(compositions()==1,1171);
            tapShift();
            require(!chinese()&&store->text.empty()&&compositions()==0,1172);
            tapShift();
            for(int key:{'S','U','3'})down(key);
            tapShift();down('B');down(VK_RETURN);
            expected=scenario==L"shift-retain-shanjie"?L"你 b":L"你b";
            require(!chinese()&&store->text==expected&&compositions()==0,1173);
            BOOL eaten=FALSE;
            require(SUCCEEDED(keys->OnKeyDown(context.Get(),'A',flags('A',false),&eaten))
                &&!eaten&&store->text==expected&&compositions()==0,1174);
        }
    } else if (scenario==L"punctuation-mcbopomofo") {
        keyboard(VK_SHIFT);
        for (int key='2';key<='8';++key) down(key);
        down(VK_OEM_7);
        down('1');
        keyboard(0);
        down(VK_OEM_7);
        down(VK_OEM_5);
        constexpr auto expected=L"＠＃＄％︿＆＊；！、＼";
        require(store->text==expected&&compositions()==1,1180);
        down(VK_RETURN);
        require(store->text==expected&&compositions()==0,1181);
    } else if (scenario.starts_with(L"spacing-english")) {
        v.lVal = 0;
        require(SUCCEEDED(mode->SetValue(serviceClient, &v)) && !chinese(), 1140);
        store->externalText(L"漢");
        store->privacyBoundary = 1;
        keyboard(0);
        BOOL offered = FALSE, eaten = FALSE;
        require(SUCCEEDED(keys->OnTestKeyDown(context.Get(), 'A', flags('A', false), &offered)), 1141);
        require(bool(offered) == (scenario == L"spacing-english"), 1142);
        require(SUCCEEDED(keys->OnKeyDown(context.Get(), 'A', flags('A', false), &eaten)), 1143);
        if (scenario == L"spacing-english") {
            require(eaten && store->text == L"漢 a" && store->prefixReadsBeforeWrite > 0, 1144);
            require(compositions() == 0 && store->selection.acpStart == 3, 1145);
            auto reads = store->prefixReads;
            require(SUCCEEDED(keys->OnTestKeyDown(context.Get(), 'B', flags('B', false), &offered)) && !offered, 1146);
            require(SUCCEEDED(keys->OnKeyDown(context.Get(), 'B', flags('B', false), &eaten)) && !eaten, 1147);
            require(store->prefixReads == reads && store->text == L"漢 a", 1148);

            // Simulate host insertion followed by movement back behind the Han character.
            store->externalText(L"漢 ab");
            require(SUCCEEDED(keys->OnTestKeyDown(context.Get(), VK_LEFT, flags(VK_LEFT, false), &offered)) && !offered, 1149);
            store->externalCaret(1);
            keyboard(VK_SHIFT);
            require(SUCCEEDED(keys->OnKeyDown(context.Get(), 'C', flags('C', false), &eaten)) && eaten, 1150);
            require(store->text == L"漢 C ab" && compositions() == 0, 1151);

            // Modified keys still reach the host, including Ctrl+Alt and the next key in a private field.
            keyboard(VK_CONTROL, VK_MENU);
            require(SUCCEEDED(keys->OnTestKeyDown(context.Get(), 'D', flags('D', false), &offered)) && !offered, 1152);
            require(SUCCEEDED(keys->OnKeyDown(context.Get(), 'D', flags('D', false), &eaten)) && !eaten, 1153);
            store->scopeChanged(IS_PASSWORD);
            store->externalCaret(1);
            pump();
            reads = store->prefixReads;
            keyboard(0);
            require(SUCCEEDED(keys->OnKeyDown(context.Get(), 'E', flags('E', false), &eaten)) && !eaten, 1154);
            require(store->text == L"漢 C ab" && store->prefixReads == reads, 1155);
        } else {
            require(!eaten && store->text == L"漢" && store->prefixReads == 0, 1156);
        }
    } else if(scenario==L"privacy-disabled") {
        ComPtr<ITfCompartmentMgr> cm;ComPtr<ITfCompartment> disabled;require(SUCCEEDED(context.As(&cm))&&SUCCEEDED(cm->GetCompartment(GUID_COMPARTMENT_KEYBOARD_DISABLED,&disabled)),1132);
        require(SUCCEEDED(disabled->SetValue(client,&v)),1133);
        keyboard(0);for(int key:{int('S'),VK_RETURN,VK_TAB,VK_DOWN}){
            BOOL accepted=TRUE;require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),key,flags(key,false),&accepted))&&!accepted,1134);
        }
        require(store->prefixReads==0&&store->text==L"隔離測試前文",1135);
        std::puts("privacy prefix_reads=0 (keyboard-disabled compartment)");
    } else if(scenario.starts_with(L"modifiers-")) {
        keyboard(0);
        if(scenario==L"modifiers-english") {
            keyboard(VK_SHIFT);for(int key:{'H','T','T','P'})down(key);keyboard(0);
            require(store->text==L"HTTP" && compositions()==1,1100);
        } else if(scenario!=L"modifiers-empty") {
            for(int key:{'S','U','3'})down(key);
            if(scenario==L"modifiers-candidates")down(VK_DOWN);
            require(!store->text.empty() && compositions()==1,1101);
        }
        auto before=store->text;auto caret=store->selection;
        for(int modifiers:{1,2,3,4,5,6,7,8,9,10,11})for(int key:{int('A'),VK_RETURN,VK_TAB,VK_UP,VK_DOWN,VK_LEFT,VK_NEXT}) {
            BYTE state[256]{};
            if(modifiers&1)state[VK_MENU]=state[VK_RMENU]=0x80;
            if(modifiers&2)state[VK_CONTROL]=state[VK_LCONTROL]=0x80;
            if(modifiers&4)state[VK_LWIN]=0x80;
            if(modifiers&8)state[VK_RWIN]=0x80;
            require(SetKeyboardState(state),1102);
            BOOL test=TRUE,eaten=TRUE;auto f=flags(key,false);
            require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),key,f,&test))&&!test,1103);
            require(SUCCEEDED(keys->OnKeyDown(context.Get(),key,f,&eaten))&&!eaten,1104);
            require(store->text==before && store->selection.acpStart==caret.acpStart && store->selection.acpEnd==caret.acpEnd,1105);
        }
        keyboard(VK_CONTROL,VK_MENU);BOOL accepted=FALSE;
        require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),VK_OEM_COMMA,flags(VK_OEM_COMMA,false),&accepted))&&accepted,1106);
        BOOL eaten=FALSE;require(SUCCEEDED(keys->OnKeyDown(context.Get(),VK_OEM_COMMA,flags(VK_OEM_COMMA,false),&eaten))&&eaten,1107);
        keyboard(0);down(VK_ESCAPE);
        std::puts("EVIDENCE synthetic SetKeyboardState + TSF callbacks; no hardware or foreground App acceptance");
    } else if(scenario.starts_with(L"privacy-")) {
        keyboard(0);for(int key:{'S','U','3'})down(key);
        auto original=store->text.substr(store->privacyBoundary); down(VK_DOWN);
        ComPtr<ITfUIElementMgr> ui;ComPtr<IEnumTfUIElements> elements;
        require(SUCCEEDED(thread.As(&ui))&&SUCCEEDED(ui->EnumUIElements(&elements)),1114);
        ComPtr<ITfUIElement> element;ULONG count=0;
        require(elements->Next(1,&element,&count)==S_OK&&count==1,1115);
        ComPtr<ITfCandidateListUIElementBehavior> list;UINT length=0;
        require(SUCCEEDED(element.As(&list))&&SUCCEEDED(list->GetCount(&length)),1116);
        UINT choice=length;
        for(UINT i=0;i<length;++i){BSTR word=nullptr;require(SUCCEEDED(list->GetString(i,&word)),1117);if(word&&std::wstring(word)!=original&&SysStringLen(word)==1)choice=i;SysFreeString(word);if(choice<length)break;}
        require(choice<length,1118);
        require(SUCCEEDED(list->SetSelection(choice))&&SUCCEEDED(list->Finalize()),1119);pump();
        if(scenario==L"privacy-transition"){
            store->scopeChanged(IS_PRIVATE);store->externalCaret(store->selection.acpEnd);pump();
            require(SUCCEEDED(focus->OnSetFocus(nullptr,doc.Get()))&&SUCCEEDED(thread->SetFocus(nullptr)),1136);Sleep(180);pump();
        }else{down(VK_RETURN);}
        require(store->text.starts_with(L"隔離測試前文") && compositions()==0,1110);
        bool paused=store->inputScope!=IS_DEFAULT||scenario==L"privacy-native-password";
        wchar_t profile[MAX_PATH]{};require(GetEnvironmentVariableW(L"LOCALAPPDATA",profile,MAX_PATH)>0,1112);
        auto file=std::wstring(profile)+L"\\Shanjie\\learning.tsv";
        std::printf("privacy chosen=%u candidates=%u original_units=%zu final_units=%zu prefix_reads=%u file=%d\n",choice,length,original.size(),store->text.size(),store->prefixReads,GetFileAttributesW(file.c_str())!=INVALID_FILE_ATTRIBUTES);
        if(scenario!=L"privacy-control")require((GetFileAttributesW(file.c_str())!=INVALID_FILE_ATTRIBUTES)!=paused,1113);
        // The context lookup precedes the first composition write. TSF itself
        // reads the text store while applying edits, and those reads can vary.
        std::printf("privacy prefix_reads=%u prewrite_prefix_reads=%u (dummy text, isolated profile)\n",
            store->prefixReads, store->prefixReadsBeforeWrite);
        if (scenario == L"privacy-normal") require(store->prefixReadsBeforeWrite > 0, 1123);
        else if (scenario != L"privacy-transition") require(store->prefixReadsBeforeWrite == 0, 1124);
    } else if(scenario==L"range-refresh") {
        keyboard(0);for(int key:{'S','U','3'})down(key);auto text=store->text;
        auto before=store->unchangedWrites;down(VK_DOWN);down(VK_ESCAPE);
        require(store->text==text && compositions()==1,1120);
        require(store->unchangedWrites>before,1121); // reproduce writes, without inferring a user-visible fault
        down(VK_RETURN);require(store->text==text && compositions()==0,1122);
        std::printf("range writes=%u unchanged=%u; one composition, one final text\n",store->textWrites,store->unchangedWrites);
    } else if(scenario.starts_with(L"status-")) {
        TF_STATUS status{};require(SUCCEEDED(context->GetStatus(&status)),1000);
        require(status.dwStaticFlags==store->staticFlags,1001);
        require(((status.dwDynamicFlags&TF_SD_READONLY)!=0)==store->readOnly,1002);
        keyboard(0);BOOL accepted=FALSE;
        require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),'S',flags('S',false),&accepted)),1003);
        require(bool(accepted)!=store->readOnly,1004);
        if(store->readOnly) {
            keyboard(VK_RSHIFT);accepted=TRUE;
            require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),VK_RSHIFT,flags(VK_RSHIFT,false),&accepted))&&!accepted,1005);
            keyboard(0);require(store->text.empty()&&chinese(),1006);
        } else {
            for(int key:{'S','U','3','\r'})down(key);
            require(store->text==L"你"&&compositions()==0,1007);
            keyboard(VK_RSHIFT);down(VK_RSHIFT);keyboard(0);up(VK_RSHIFT);require(!chinese(),1008);
            keyboard(VK_RSHIFT);down(VK_RSHIFT);keyboard(0);up(VK_RSHIFT);require(chinese(),1009);
            if (scenario == L"status-writable-disjoint") {
                store->setReadOnly(true);
                require(SUCCEEDED(context->GetStatus(&status)) && (status.dwDynamicFlags & TF_SD_READONLY), 1137);
                // A host may deliver OnKeyDown without first probing the key.
                BOOL eaten = TRUE;
                require(SUCCEEDED(keys->OnKeyDown(context.Get(), 'S', flags('S', false), &eaten)) && !eaten, 1138);
                require(store->text == L"你" && compositions() == 0, 1139);
            }
        }
    } else if(scenario.starts_with(L"shift-")) {
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
        bool expected=scenario==L"word-deferred"||scenario==L"word-natural"||scenario==L"word-disjoint";
        while(std::chrono::steady_clock::now()<end){pump();if(expected&&panel()&&IsWindowVisible(panel()))break;Sleep(10);}
        require((panel()&&IsWindowVisible(panel()))==expected,961);
        if(expected){require(GetWindow(panel(),GW_OWNER)==savedWindow,962);store->externalCaret(2);pump();require(!IsWindowVisible(panel()),963);}
        require(store->text==(scenario==L"word-invalid"?L"ab":L"回家"),964);
        if(password){store->window=savedWindow;DestroyWindow(password);}
    }
    keyboard(0);require(SUCCEEDED(tip->Deactivate()),970);thread->SetFocus(nullptr);doc->Pop(TF_POPF_ALL);thread->Deactivate();DestroyWindow(store->window);if(passwordOwner)DestroyWindow(passwordOwner);
    std::printf("PASS %ls\n",scenario.c_str());return 0;
}
