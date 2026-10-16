// Real TSF context and resident core, with an isolated delayed setting request.
#define wmain old_smoke_main
#include "tsf_smoke.cpp"
#undef wmain
int wmain(int argc,wchar_t** argv) {
    require(argc==2||argc==3,801);
    std::wstring scenario=argc==3?argv[2]:L"normal";
    require(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)),802);
    auto dll=LoadLibraryW(argv[1]); require(dll!=nullptr,803);
    auto get=(HRESULT(WINAPI*)(REFCLSID,REFIID,void**))GetProcAddress(dll,"DllGetClassObject"); require(get!=nullptr,804);
    ComPtr<IClassFactory> factory; require(SUCCEEDED(get(ServiceId,IID_PPV_ARGS(&factory))),805);
    ComPtr<ITfTextInputProcessorEx> tip; require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&tip))),806);
    ComPtr<ITfThreadMgr> thread; require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&thread))),807);
    ComPtr<ITfThreadMgrEx> extended; thread.As(&extended); TfClientId client=0;
    require(SUCCEEDED(extended->ActivateEx(&client,TF_TMAE_NOACTIVATETIP|TF_TMAE_NOACTIVATEKEYBOARDLAYOUT)),808);
    ComPtr<ITfDocumentMgr> doc; require(SUCCEEDED(thread->CreateDocumentMgr(&doc)),809);
    ComPtr<Store> store; store.Attach(new Store);
    ComPtr<ITfContext> context; TfEditCookie cookie;
    require(SUCCEEDED(doc->CreateContext(client,0,store.Get(),&context,&cookie))&&SUCCEEDED(doc->Push(context.Get()))&&SUCCEEDED(thread->SetFocus(doc.Get())),810);
    ComPtr<ITfClientId> ids; thread.As(&ids); TfClientId serviceClient=0; require(SUCCEEDED(ids->GetClientId(ServiceId,&serviceClient)),811);
    ComPtr<ThreadProxy> proxy; proxy.Attach(new ThreadProxy(thread.Get(),true));
    require(SUCCEEDED(tip->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),812);
    BYTE keyboard[256]{}; require(SetKeyboardState(keyboard),813);
    ComPtr<ITfKeyEventSink> keys; tip.As(&keys);
    auto press=[&](WPARAM vk) {BOOL test=FALSE,eaten=FALSE;require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),vk,0,&test))&&test,814);require(SUCCEEDED(keys->OnKeyDown(context.Get(),vk,0,&eaten))&&eaten,815);};
    press('S'); auto original=store->text; require(original==L"ㄋ",816);
    ComPtr<ITfUIElementMgr> ui; thread.As(&ui);
    auto listed=[&] {ComPtr<IEnumTfUIElements> items; ui->EnumUIElements(&items); ComPtr<ITfUIElement> item;ULONG count=0;if(items->Next(1,&item,&count)!=S_OK||!count)return false;ComPtr<ITfCandidateListUIElement> list;UINT total=0;return SUCCEEDED(item.As(&list))&&SUCCEEDED(list->GetCount(&total))&&total>1;};
    auto wait=[&](auto predicate,int check) {auto until=std::chrono::steady_clock::now()+std::chrono::seconds(6);while(!predicate()){require(std::chrono::steady_clock::now()<until,check);MSG m;while(PeekMessageW(&m,nullptr,0,0,PM_REMOVE)){TranslateMessage(&m);DispatchMessageW(&m);}Sleep(5);}};
    require(!listed(),817);
    wchar_t local[32768];GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768);auto profile=std::wstring(local)+L"\\Shanjie\\profile.ini";
    require(WritePrivateProfileStringW(L"engine",L"prediction",L"1",profile.c_str()),818);
    auto refreshStart=std::chrono::steady_clock::now();localNotify(L"Shanjie.ProfileChanged.v2");
    wchar_t fault[32]{};GetEnvironmentVariableW(L"SHANJIE_TEST_FAULT",fault,32);
    if(scenario==L"deadline") {
        auto paused=[&]{ComPtr<IEnumTfUIElements> items;ui->EnumUIElements(&items);ComPtr<ITfUIElement> item;ULONG n=0;if(items->Next(1,&item,&n)!=S_OK||!n)return false;ComPtr<ITfCandidateListUIElement> list;if(FAILED(item.As(&list)))return false;BSTR text=nullptr;if(FAILED(list->GetString(0,&text)))return false;std::wstring message(text,SysStringLen(text));SysFreeString(text);return message.find(L"按任一鍵重試")!=std::wstring::npos;};
        wait(paused,832);
        while(std::chrono::steady_clock::now()-refreshStart<std::chrono::seconds(8)){MSG m;while(PeekMessageW(&m,nullptr,0,0,PM_REMOVE)){TranslateMessage(&m);DispatchMessageW(&m);}Sleep(10);}
        require(!listed()&&store->text==original,833);
        press('U');press('3');press(VK_RETURN);wait([&]{return store->text==L"你";},834);
    } else if(scenario==L"escape") {
        press(VK_ESCAPE);require(store->text.empty(),823);
        press('S');press('U');press('3');press(VK_RETURN);
        wait([&]{return store->text==L"你";},824);
    } else if(scenario==L"focus") {
        ComPtr<ITfThreadFocusSink> focus;tip.As(&focus);require(SUCCEEDED(focus->OnKillThreadFocus()),825);
        wait([&]{return store->text==original;},826);
        press('S');press('U');press('3');press(VK_RETURN);
        wait([&]{return store->text==original+L"你";},827);
    } else if(scenario==L"queued"||scenario==L"latest") {
        if(scenario==L"latest") {require(WritePrivateProfileStringW(L"engine",L"prediction",L"0",profile.c_str()),828);localNotify(L"Shanjie.ProfileChanged.v2");}
        press('U');press('3');press(VK_RETURN);
        wait([&]{return store->text==L"你";},829);
        if(scenario==L"latest") {
            MSG m;while(PeekMessageW(&m,nullptr,0,0,PM_REMOVE)){TranslateMessage(&m);DispatchMessageW(&m);}
            press('S');require(!listed(),830);press(VK_ESCAPE);
        }
    } else {
        wait([&]{return listed();},819);
        require(store->text==original,820);
        press('U');press('3');press(VK_RETURN);require(store->text==L"你",821);
    }
    if((scenario==L"normal"||scenario==L"queued"||scenario==L"latest")&&(std::wstring(fault)==L"before-apply"||std::wstring(fault)==L"after-apply"))
        require(std::chrono::steady_clock::now()-refreshStart>=std::chrono::milliseconds(700),831);
    require(SUCCEEDED(tip->Deactivate()),822);thread->SetFocus(nullptr);doc->Pop(TF_POPF_ALL);thread->Deactivate();
    std::puts("PASS prediction refresh, queued input/latest setting, cancellation and single commit after delayed setting reply");return 0;
}
