// Synthetic keyboard state on an owned TSF thread; never sends desktop input.
#define wmain old_smoke_main
#include "tsf_smoke.cpp"
#undef wmain
int wmain(int argc,wchar_t** argv) {
    require(argc==3,901);std::wstring scenario=argv[2];
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
        if(required)require(test&&eaten,917);
    };
    if(scenario==L"standalone-tones"){
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
