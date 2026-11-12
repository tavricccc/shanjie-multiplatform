// Synthetic keyboard state on a private TSF desktop; never sends desktop input.
#define wmain old_smoke_main
#include "tsf_smoke.cpp"
#undef wmain
int wmain(int argc,wchar_t** argv) {
    require(argc==3,901);std::wstring scenario=argv[2];
    auto desktopName=L"Shanjie.English."+std::to_wstring(GetCurrentProcessId());
    auto desktop=CreateDesktopW(desktopName.c_str(),nullptr,nullptr,0,GENERIC_ALL,nullptr);
    require(desktop!=nullptr&&SetThreadDesktop(desktop),980);
    std::puts("PRIVATE DESKTOP ready (never switched)");std::fflush(stdout);
    BYTE original[256]{};require(GetKeyboardState(original),902);
    struct Restore {BYTE* state;~Restore(){SetKeyboardState(state);}} restore{original};
    require(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)),903);
    auto dll=LoadLibraryW(argv[1]);require(dll!=nullptr,904);
    auto get=(HRESULT(WINAPI*)(REFCLSID,REFIID,void**))GetProcAddress(dll,"DllGetClassObject");require(get!=nullptr,905);
    ComPtr<IClassFactory> factory;require(SUCCEEDED(get(ServiceId,IID_PPV_ARGS(&factory))),906);
    ComPtr<ITfTextInputProcessorEx> tip;require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&tip))),907);
    ComPtr<ITfThreadMgr> thread;require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&thread))),908);
    ComPtr<ITfThreadMgrEx> extended;thread.As(&extended);TfClientId client=0;
    require(SUCCEEDED(extended->ActivateEx(&client,TF_TMAE_NOACTIVATETIP|TF_TMAE_NOACTIVATEKEYBOARDLAYOUT)),909);
    ComPtr<ITfDocumentMgr> doc;require(SUCCEEDED(thread->CreateDocumentMgr(&doc)),910);
    ComPtr<Store> store;store.Attach(new Store);ComPtr<ITfContext> context;TfEditCookie cookie;
    require(SUCCEEDED(doc->CreateContext(client,0,store.Get(),&context,&cookie))&&SUCCEEDED(doc->Push(context.Get()))&&SUCCEEDED(thread->SetFocus(doc.Get())),911);
    ComPtr<ITfClientId> ids;thread.As(&ids);TfClientId serviceClient=0;require(SUCCEEDED(ids->GetClientId(ServiceId,&serviceClient)),912);
    ComPtr<ThreadProxy> proxy;proxy.Attach(new ThreadProxy(thread.Get(),true));
    require(SUCCEEDED(tip->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),913);
    ComPtr<ITfKeyEventSink> keys;tip.As(&keys);
    auto press=[&](int vk,bool shift=false,bool caps=false,bool required=true){
        for(int k:{VK_SHIFT,VK_LSHIFT,VK_RSHIFT,VK_CONTROL,VK_MENU,VK_CAPITAL})GetKeyState(k);
        BYTE keyboard[256]{};keyboard[VK_NUMLOCK]=1;if(shift){keyboard[VK_SHIFT]=0x80;keyboard[VK_LSHIFT]=0x80;}if(caps)keyboard[VK_CAPITAL]=1;
        require(SetKeyboardState(keyboard),914);BOOL test=FALSE,eaten=FALSE;
        LPARAM flags=LPARAM(1)|(LPARAM(MapVirtualKeyW(vk,MAPVK_VK_TO_VSC))<<16);
        require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),vk,flags,&test)),915);
        if(test)require(SUCCEEDED(keys->OnKeyDown(context.Get(),vk,flags,&eaten)),916);
        if(required && !(test&&eaten)) {
            struct Translated {unsigned kind,ch,modifiers;} translated{};
            auto translate=(bool(__cdecl*)(size_t,intptr_t,Translated*))GetProcAddress(dll,"sj_translate");
            bool available=translate&&translate(vk,flags,&translated);
            std::printf("Key rejection: vk=%d shift=%d caps=%d test=%d eaten=%d layout=%p translated=%d kind=%u ch=%u modifiers=%u\n",vk,shift,caps,test,eaten,GetKeyboardLayout(0),available,translated.kind,translated.ch,translated.modifiers);
            std::fflush(stdout);
        }
        if(required)require(test&&eaten,917);
    };
    if(scenario==L"digit-navigation"){
        std::wstring committed;
        for(auto digit:std::wstring(L"125890")){
            for(int navigation:{VK_LEFT,VK_HOME,VK_RIGHT,VK_END}){
                press(digit);require(store->text==committed+digit,973);
                press(navigation,false,false,navigation==VK_LEFT||navigation==VK_HOME);
                LONG position=(LONG)committed.size()+((navigation==VK_LEFT||navigation==VK_HOME)?0:1);
                require(store->text==committed+digit && store->selection.acpStart==position && store->selection.acpEnd==position,974);
                press('3');
                auto edited=committed+((navigation==VK_LEFT||navigation==VK_HOME)?std::wstring(L"3")+digit:std::wstring(1,digit)+L"3");
                require(store->text==edited,975);
                press(VK_BACK);require(store->text==committed+digit,976);
                press(VK_RETURN,false,false,false);committed+=digit;require(store->text==committed,977);
            }
        }
    }else if(scenario==L"english-symbol-order"){
        std::wstring expected;
        for(auto mark:{std::pair{VK_ADD,L"+"},std::pair{VK_SPACE,L" "}}){
            for(auto ch:std::wstring(L"ABC"))press(ch,true);
            require(store->text==expected+L"ABC",960);
            press(mark.first);expected+=std::wstring(L"ABC")+mark.second;
            require(store->text==expected,961);
            press(VK_RETURN,false,false,false);require(store->text==expected,962);
        }
        for(auto ch:std::wstring(L"99.9"))press(ch==L'.'?VK_OEM_PERIOD:ch);
        require(store->text==expected+L"99.9",963);
        press('5',true);expected+=L"99.9%";
        require(store->text==expected,964);
        press(VK_RETURN,false,false,false);require(store->text==expected,965);
        auto committed=expected;
        for(auto ch:std::wstring(L"99.9"))press(ch==L'.'?VK_OEM_PERIOD:ch);
        press(VK_LEFT);press('5',true);
        require(store->text==committed+L"99.%9" && store->selection.acpStart==(LONG)committed.size()+4,966);
        press(VK_BACK);require(store->text==committed+L"99.9",967);
        press(VK_ESCAPE);require(store->text==committed,968);
        press('1');press('5',true);require(store->text==committed+L"1%",969);
        press(VK_SPACE,true);require(store->text==committed+L"１％",970);
        press(VK_SPACE,true);require(store->text==committed+L"1%",971);
        press(VK_RETURN,false,false,false);expected=committed+L"1%";require(store->text==expected,972);
    }else if(scenario==L"standalone-tones"){
        for(int key:{'3','4','6','7'}){press(key);press(VK_RETURN,false,false,false);}
        require(store->text==L"ˇˋˊ˙",948);
    }else if(scenario==L"standalone-tones-off"||scenario==L"standalone-tones-shanjie"){
        press('3');press(VK_RETURN,false,false,false);require(store->text==L"3",950);
    }else if(scenario==L"backquote-disabled"){
        press(VK_OEM_3);press(VK_OEM_COMMA);require(store->text==L"`",954);press(VK_ESCAPE);
    }else if(scenario==L"backquote"){
        for(auto entry:{std::pair{VK_OEM_COMMA,L'，'},std::pair{VK_OEM_PERIOD,L'。'},std::pair{VK_OEM_2,L'／'},std::pair{VK_OEM_1,L'；'},std::pair{VK_OEM_MINUS,L'－'}}){
            auto before=store->text;press(VK_OEM_3);press(entry.first);require(store->text==before+entry.second,941);
        }
        press(VK_OEM_3);press('2',true);require(store->text==L"，。／；－＠",942);
        auto before=store->text;
        if(std::getenv("SHANJIE_TEST_PINYIN")){press('N');press('I');before+=L"ni";}
        press(VK_OEM_3);press('U');for(auto ch:std::wstring(L"FF0C"))press(ch);
        auto code=store->text;store->externalCaret((LONG)before.size());
        MSG message{};while(PeekMessageW(&message,nullptr,0,0,PM_REMOVE)){TranslateMessage(&message);DispatchMessageW(&message);}
        require(store->text==code,943);press(VK_SPACE);require(store->text==before+L"，",944);
        auto committed=store->text;
        press(VK_OEM_3);press(VK_OEM_4);press(VK_DOWN);press('2');
        require(store->text==committed+L"『",955);
        press(VK_RETURN);committed=store->text;
        press(VK_OEM_3);press(VK_OEM_4);press(VK_ESCAPE);
        require(store->text==committed,956); // Mapped marks remain editable; codes above commit directly.
        press(VK_OEM_3);press(VK_OEM_5,true);press(VK_DOWN);
        ComPtr<ITfUIElementMgr> ui;thread.As(&ui);ComPtr<IEnumTfUIElements> items;ui->EnumUIElements(&items);
        ComPtr<ITfUIElement> item;ULONG found=0;require(items->Next(1,&item,&found)==S_OK&&found,957);
        ComPtr<ITfCandidateListUIElement> list;require(SUCCEEDED(item.As(&list)),958);
        UINT count=0,target=UINT_MAX,current=0;list->GetCount(&count);list->GetSelection(&current);
        for(UINT i=0;i<count;i++){BSTR text=nullptr;require(SUCCEEDED(list->GetString(i,&text)),959);if(std::wstring(text)==L"↑")target=i;SysFreeString(text);}
        require(target!=UINT_MAX,960);
        while(current<target){press(VK_RIGHT);UINT next=0;list->GetSelection(&next);require(next>current,961);current=next;}
        press(VK_RETURN);require(store->text==committed+L"↑",962);
    }else if(scenario==L"numpad"){
        for(int vk=VK_NUMPAD0;vk<=VK_NUMPAD9;vk++)press(vk);
        for(int vk:{VK_ADD,VK_SUBTRACT,VK_MULTIPLY,VK_DIVIDE,VK_DECIMAL})press(vk);
        require(store->text==L"0123456789+-*/.",934);
        press(VK_RETURN,false,false,false);
        const wchar_t* typing=std::getenv("SHANJIE_TEST_ETEN")?L"NE3":L"SU3";
        for(auto ch:std::wstring(typing))press(ch);
        press(VK_NUMPAD1);press(VK_DIVIDE);
        require(store->text==L"0123456789+-*/.你1/",935);
        press(VK_ESCAPE);require(store->text==L"0123456789+-*/.",936);
        auto base=store->text;press(typing[0]);press(VK_NUMPAD1);press(VK_RETURN,false,false,false);
        require(store->text.size()==base.size()+2&&store->text.back()==L'1',937);
        base=store->text;press(VK_OEM_3);press('U');press(VK_NUMPAD4);press('E');press(VK_NUMPAD0);press(VK_NUMPAD0);press(VK_SPACE);
        require(store->text==base+L"一",938);
    }else if(scenario==L"fullwidth"){
        press('A',true);require(store->text==L"A",937);
        press(VK_SPACE,true);require(store->text==L"Ａ",938);
        press(VK_SPACE,true);require(store->text==L"A",939);
        press(VK_SPACE,true);press('B',true);press(VK_SPACE);require(store->text==L"ＡＢ　",940);
        press(VK_RETURN);require(store->text==L"ＡＢ　",941);
        press(VK_SPACE,true);press('C',true);require(store->text==L"ＡＢ　C",942);
        press(VK_SPACE,true);require(store->text==L"ＡＢ　Ｃ",943);
        require(SUCCEEDED(keys->OnSetFocus(FALSE))&&store->text==L"ＡＢ　Ｃ",944);
    }else if(scenario==L"fullwidth-english"){
        press(VK_SPACE,true);press('A');press('2');press(VK_OEM_PERIOD);press(VK_SPACE);
        require(store->text==L"ａ２．　",945);
        press(VK_SPACE,true);BOOL test=TRUE;
        require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),'B',0,&test))&&!test,946);
    }else if(scenario==L"fullwidth-disabled"){
        BOOL test=TRUE;BYTE keyboard[256]{};keyboard[VK_SHIFT]=keyboard[VK_LSHIFT]=0x80;SetKeyboardState(keyboard);
        require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),VK_SPACE,0,&test))&&!test,947);
    }else if(scenario==L"caps"){
        // HELLO is in the public bundled English list; RUST is intentionally absent.
        for(auto ch:std::wstring(L"HELLO"))press(ch,false,true);
        press(VK_RETURN);require(store->text==L"HELLO",918);
    }else if(scenario==L"caps-zh"){
        press('S',false,true);press('U',false,true);press('3',false,true);press(VK_RETURN);
        require(store->text==L"你",919);
    }else if(scenario==L"shift"){
        press('R',true);press('U');press('S');press('T');press(VK_RETURN);require(store->text==L"Rust",920);
    }else if(scenario==L"shift-caps-zh"){
        for(auto ch:std::wstring(L"GEMINI"))press(ch,true,true);
        for(auto ch:std::wstring(L"CL3M/4A87"))press(ch==L'/'?VK_OEM_2:ch,false,true);
        require(store->text==L"gemini好用嗎",963);
        press(VK_RETURN);require(store->text==L"gemini好用嗎",964);
    }else if(scenario==L"shift-caps"){
        press('R',true,true);press('U',false,true);press(VK_RETURN);require(store->text==L"rU",921);
    }else if(scenario==L"eten"){
        press('N');press('E');press('3');press(VK_RETURN);require(store->text==L"你",922);
    }else{
        for(int i=0;i<5;i++){press('S');press('U');press('3');}
        require(store->text==L"你你你你你",923);press(VK_LEFT);press(VK_LEFT);
        if(scenario==L"candidate"){
            press(VK_DOWN);
            ComPtr<ITfUIElementMgr> ui;thread.As(&ui);ComPtr<IEnumTfUIElements> items;ui->EnumUIElements(&items);
            ComPtr<ITfUIElement> item;ULONG found=0;require(items->Next(1,&item,&found)==S_OK&&found,928);
            ComPtr<ITfCandidateListUIElement> list;require(SUCCEEDED(item.As(&list)),929);
            UINT count=0,target=UINT_MAX,current=0;list->GetCount(&count);list->GetSelection(&current);
            for(UINT i=0;i<count;i++){BSTR text=nullptr;require(SUCCEEDED(list->GetString(i,&text)),930);if(std::wstring(text)==L"你")target=i;std::printf("candidate %u: %u UTF-16 units\n",i,SysStringLen(text));SysFreeString(text);}
            require(target!=UINT_MAX,931);
            while(current<target){press(VK_RIGHT);UINT next=0;list->GetSelection(&next);require(next>current,932);current=next;}
            require(current==target,933);
        }
        press(VK_RETURN);auto before=store->text;
        press(VK_ESCAPE,false,false,false);
        if(scenario==L"whole")require(store->text==L"你你你你你",924);
        else if(scenario==L"candidate"){std::printf("Candidate prefix: before Esc %zu UTF-16, after %zu\n",before.size(),store->text.size());require(!store->text.empty()&&store->text.size()<before.size(),925);}
        else require(store->text==L"你你你",926);
    }
    require(SUCCEEDED(tip->Deactivate()),927);thread->SetFocus(nullptr);doc->Pop(TF_POPF_ALL);thread->Deactivate();
    std::puts("PASS real TSF English/Caps/Shift/Eten and Enter partial composition");return 0;
}
