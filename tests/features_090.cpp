// Real TSF/edit-lock regression in offscreen owned windows; no registration or desktop input.
#define wmain previous_smoke_main
#include "tsf_smoke.cpp"
#undef wmain
#include "native_capture.h"
#include <fstream>
#include <filesystem>
void pump() { MSG m; while(PeekMessageW(&m,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&m); DispatchMessageW(&m); } }
std::wstring controlText(HWND hwnd) { int n=GetWindowTextLengthW(hwnd); std::wstring s(n+1,0); GetWindowTextW(hwnd,s.data(),n+1); s.resize(n); return s; }
COLORREF sampleBackground(HWND window) {
    auto screen=GetDC(nullptr),dc=CreateCompatibleDC(screen);auto bitmap=CreateCompatibleBitmap(screen,16,16);auto old=SelectObject(dc,bitmap);
    SendMessageW(window,WM_PRINT,(WPARAM)dc,PRF_CLIENT|PRF_ERASEBKGND);auto color=GetPixel(dc,8,8);
    SelectObject(dc,old);DeleteObject(bitmap);DeleteDC(dc);ReleaseDC(nullptr,screen);return color;
}
int wmain(int argc,wchar_t** argv) {
    require(argc==3,701);
    wchar_t headless[2]{}; require(GetEnvironmentVariableW(L"SHANJIE_HEADLESS",headless,2)==1&&headless[0]==L'1',702);
    require(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)),704);
    auto dll=LoadLibraryW(argv[1]);require(dll!=nullptr,705);
    auto get=(HRESULT(WINAPI*)(REFCLSID,REFIID,void**))GetProcAddress(dll,"DllGetClassObject"); require(get!=nullptr,706);
    ComPtr<IClassFactory> factory;require(SUCCEEDED(get(ServiceId,IID_PPV_ARGS(&factory))),707);
    ComPtr<ITfTextInputProcessorEx> tip;require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&tip))),708);
    ComPtr<ITfThreadMgr> thread;require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&thread))),709);
    ComPtr<ITfThreadMgrEx> extended;thread.As(&extended); TfClientId client;
    require(SUCCEEDED(extended->ActivateEx(&client,TF_TMAE_NOACTIVATETIP|TF_TMAE_NOACTIVATEKEYBOARDLAYOUT)),710);
    ComPtr<ITfDocumentMgr> doc;require(SUCCEEDED(thread->CreateDocumentMgr(&doc)),711);
    ComPtr<Store> store;store.Attach(new Store);
    WNDCLASSW wc{};wc.lpfnWndProc=DefWindowProcW;wc.hInstance=GetModuleHandleW(nullptr);wc.lpszClassName=L"ShanjiePrivateTest";RegisterClassW(&wc);
    store->window=CreateWindowW(wc.lpszClassName,L"Offscreen fixture",WS_OVERLAPPEDWINDOW,-12000,-12000,900,500,nullptr,nullptr,wc.hInstance,nullptr);
    require(store->window!=nullptr,712);store->layoutReady=true;ShowWindow(store->window,SW_SHOWNOACTIVATE);
    ComPtr<ITfContext> context;TfEditCookie cookie;
    require(SUCCEEDED(doc->CreateContext(client,0,store.Get(),&context,&cookie))&&SUCCEEDED(doc->Push(context.Get()))&&SUCCEEDED(thread->SetFocus(doc.Get())),713);
    ComPtr<ITfClientId> ids;thread.As(&ids);TfClientId serviceClient;require(SUCCEEDED(ids->GetClientId(ServiceId,&serviceClient)),714);
    ComPtr<ThreadProxy> proxy;proxy.Attach(new ThreadProxy(thread.Get(),true));
    require(SUCCEEDED(tip->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),715);
    ComPtr<ITfKeyEventSink> keys;tip.As(&keys);
    ComPtr<ITfCompartmentMgr> compartments;proxy.As(&compartments);ComPtr<ITfCompartment> mode;compartments->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&mode);
    auto chinese=[&]{VARIANT v{};require(SUCCEEDED(mode->GetValue(&v)),716);bool on=v.vt==VT_I4&&v.lVal;VariantClear(&v);return on;};
    auto setChinese=[&]{VARIANT v{};v.vt=VT_I4;v.lVal=1;require(SUCCEEDED(mode->SetValue(serviceClient,&v)),717);};
    auto keyboard=[&](int held,int extra=0){ for(int k:{VK_SHIFT,VK_LSHIFT,VK_RSHIFT,VK_CONTROL,VK_LCONTROL,VK_RCONTROL,VK_RETURN,VK_LWIN,VK_RWIN,VK_MENU})GetKeyState(k);BYTE state[256]{};if(held)state[held]=0x80;if(held==VK_RSHIFT)state[VK_SHIFT]=0x80;if(held==VK_LCONTROL)state[VK_CONTROL]=0x80;if(extra)state[extra]=0x80;require(SetKeyboardState(state),718);};
    auto flags=[](int vk,bool up){return LPARAM(1)|(LPARAM(MapVirtualKeyW(vk==VK_SHIFT?VK_RSHIFT:vk,MAPVK_VK_TO_VSC))<<16)|(up?(LPARAM(3)<<30):0);};
    auto down=[&](int vk,bool test=true){BOOL eaten=FALSE,t=TRUE;auto f=flags(vk,false);if(test)require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),vk,f,&t)),719);if(t)require(SUCCEEDED(keys->OnKeyDown(context.Get(),vk,f,&eaten)),720);};
    auto up=[&](int vk){BOOL e=FALSE;auto f=flags(vk,true);require(SUCCEEDED(keys->OnTestKeyUp(context.Get(),vk,f,&e)),721);require(SUCCEEDED(keys->OnKeyUp(context.Get(),vk,f,&e)),722);};
    for(int scenario=0;scenario<7;scenario++) {
        setChinese();keyboard(VK_RSHIFT,scenario==5?VK_RETURN:0);down(VK_SHIFT);
        if(scenario==0){keyboard(VK_RSHIFT,VK_RETURN);down(VK_RETURN);keyboard(VK_RSHIFT);up(VK_RETURN);}
        if(scenario==1){keyboard(VK_RSHIFT,VK_RETURN);down(VK_RETURN,false);keyboard(VK_RSHIFT);up(VK_RETURN);}
        if(scenario==2){keyboard(VK_RSHIFT);up(VK_RETURN);}
        if(scenario==3){PostMessageW(store->window,WM_KEYDOWN,VK_RETURN,flags(VK_RETURN,false));PostMessageW(store->window,WM_KEYUP,VK_RETURN,flags(VK_RETURN,true));pump();}
        if(scenario==4){keys->OnSetFocus(FALSE);}
        keyboard(0);up(VK_SHIFT);
        require(chinese()==(scenario!=6),723+scenario);
        require(store->text.empty(),731);std::printf("PASS Shift scenario %d\n",scenario);
    }
    setChinese();keyboard(0);store->text=L"回家";store->selection.acpStart=0;store->selection.acpEnd=2;
    auto doubleCtrl=[&]{for(int i=0;i<2;i++){keyboard(VK_LCONTROL);down(VK_CONTROL);keyboard(0);up(VK_CONTROL);}};
    auto panel=[&]{return FindWindowExW(nullptr,nullptr,L"Shanjie.AddWord.Popup.v1",nullptr);};
    auto wait=[&](auto predicate,int check){auto until=std::chrono::steady_clock::now()+std::chrono::seconds(12);while(!predicate()){require(std::chrono::steady_clock::now()<until,check);pump();Sleep(10);}};
    HWND focus=GetFocus();doubleCtrl();wait([&]{auto p=panel();return p&&IsWindowVisible(p);},740);
    auto popup=panel();require(GetWindow(popup,GW_OWNER)==store->window&&GetFocus()==focus,741);
    require((GetWindowLongW(popup,GWL_EXSTYLE)&(WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW))==(WS_EX_NOACTIVATE|WS_EX_TOOLWINDOW),742);
    wait([&]{bool ready=controlText(GetDlgItem(popup,3)).find(L"ㄏ")!=std::wstring::npos;if(!IsWindowVisible(popup)){std::puts("POPUP hidden before suggestion");return true;}return ready;},743);
    require(IsWindowVisible(popup),755);
    wait([&]{return controlText(GetDlgItem(popup,5)).find(L"內建詞庫：已有同詞同讀音")!=std::wstring::npos;},756);
    require(controlText(GetDlgItem(popup,5)).find(L"目前模型：有此詞")!=std::wstring::npos,757);
    wchar_t model[128]{};GetEnvironmentVariableW(L"SHANJIE_TEST_POPUP_MODEL",model,128);require(controlText(GetDlgItem(popup,5)).find(model)!=std::wstring::npos,763);
    require(controlText(GetDlgItem(popup,10))==L"仍要加入",758);
    wchar_t themeText[4]{};GetEnvironmentVariableW(L"SHANJIE_TEST_POPUP_THEME",themeText,4);int theme=_wtoi(themeText);DWORD light=1,size=sizeof(light);
    if(theme==0)RegGetValueW(HKEY_CURRENT_USER,L"Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize",L"AppsUseLightTheme",RRF_RT_REG_DWORD,nullptr,&light,&size);
    HIGHCONTRASTW contrast{sizeof(contrast)};SystemParametersInfoW(SPI_GETHIGHCONTRAST,sizeof(contrast),&contrast,0);
    auto background=contrast.dwFlags&HCF_HIGHCONTRASTON?GetSysColor(COLOR_WINDOW):theme==2||(theme==0&&light==0)?RGB(32,32,32):RGB(255,255,255);
    require(sampleBackground(popup)==background,759);require(sampleBackground(GetDlgItem(popup,10))==background,760);
    captureNative(popup,std::filesystem::path(argv[2])/L"add-word.bmp",false,true);
    keyboard(VK_LCONTROL);down('A');keyboard(0);
    int sequence[]={'C','J','O','6','R','U','8',VK_SPACE};for(int vk:sequence)down(vk);
    { auto reading=controlText(GetDlgItem(popup,3)); if(reading!=L"ㄏㄨㄟˊ-ㄐㄧㄚ-"){std::printf("READING codepoints:");for(auto c:reading)std::printf(" %04x",unsigned(c));std::puts("");} require(reading==L"ㄏㄨㄟˊ-ㄐㄧㄚ-",744); }
    // A quick Enter waits for the current reading's check and prompts on an existing word.
    down(VK_RETURN);wait([&]{return controlText(GetDlgItem(popup,5)).find(L"內建詞庫：已有同詞同讀音")!=std::wstring::npos;},761);
    require(IsWindowVisible(popup),762);down(VK_RETURN);wait([&]{return !IsWindowVisible(popup);},745);
    require(store->text==L"回家"&&store->selection.acpStart==0&&store->selection.acpEnd==2,746);
    wchar_t local[32768];GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768);auto folder=std::filesystem::path(local)/L"Shanjie";
    bool saved=false;for(auto& file:std::filesystem::directory_iterator(folder)){if(file.path().filename().wstring().starts_with(L"dictionary-")){std::ifstream in(file.path(),std::ios::binary);std::string s((std::istreambuf_iterator<char>(in)),{});if(s.find("ㄏㄨㄟˊ-ㄐㄧㄚ\t回家")!=std::string::npos)saved=true;}}require(saved,747);
    doubleCtrl();wait([&]{return panel()&&IsWindowVisible(panel());},748);down(VK_ESCAPE);require(!IsWindowVisible(panel()),749);
    store->text=L"龘龘龘龘龘龘龘龘";store->selection.acpStart=0;store->selection.acpEnd=8;
    doubleCtrl();wait([&]{return panel()&&IsWindowVisible(panel())&&controlText(GetDlgItem(panel(),5)).find(L"目前模型：無完整詞條")!=std::wstring::npos;},764);
    require(controlText(GetDlgItem(panel(),5)).find(L"內建詞庫：未收錄此詞")!=std::wstring::npos,765);down(VK_ESCAPE);
    store->text=L"回家";store->selection.acpStart=0;store->selection.acpEnd=2;
    doubleCtrl();wait([&]{return panel()&&IsWindowVisible(panel());},750);store->externalCaret(2);pump();require(!IsWindowVisible(panel()),751);
    store->readOnly=true;store->selection.acpStart=0;store->selection.acpEnd=2;doubleCtrl();pump();require(!IsWindowVisible(panel()),752);
    store->readOnly=false;SetWindowLongW(store->window,GWL_STYLE,GetWindowLongW(store->window,GWL_STYLE)|ES_PASSWORD);doubleCtrl();pump();require(!IsWindowVisible(panel()),753);
    require(SUCCEEDED(tip->Deactivate()),754);thread->SetFocus(nullptr);doc->Pop(TF_POPF_ALL);thread->Deactivate();
    std::puts("PASS real TSF Shift, owned add-word popup, manual readings, save, selection preservation, Esc, stale selection, read-only and password rejection");return 0;
}
