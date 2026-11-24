#include <cwctype>
#include <deque>
#include <fstream>
#include <string>
std::string utf8(const std::wstring& value);
// Exercises the real Windows TSF context and edit locks without registration or
// opening a window by default. --visual also exercises deferred layout and
// leaves an interactive candidate window open for inspection.
#include <windows.h>
#include <msctf.h>
#include <textstor.h>
#include <psapi.h>
#include <wrl/client.h>
#include <wrl/implements.h>
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <string>
#include <thread>
#include "identity.h"
using Microsoft::WRL::ComPtr;
#include "thread_proxy.h"
#include "local_notify.h"
class EditProbe final : public Microsoft::WRL::RuntimeClass<
    Microsoft::WRL::RuntimeClassFlags<Microsoft::WRL::ClassicCom>, ITfEditSession> {
public:
    unsigned calls=0;
    HRESULT STDMETHODCALLTYPE DoEditSession(TfEditCookie) override { ++calls; return S_OK; }
};
class Store final : public ITextStoreACP, public ITfContextOwnerCompositionSink {
    long refs_ = 1;
    ComPtr<ITextStoreACPSink> sink_;
public:
    bool composing=false; unsigned earlyWrites=0,starts=0,ends=0; std::wstring endedText;
    HRESULT STDMETHODCALLTYPE OnStartComposition(ITfCompositionView* view,BOOL* accepted) override { composing=true; ++starts; *accepted=TRUE; GUID owner{}; view->GetOwnerClsid(&owner); std::printf("EVENT composition_start owner=%08lx\n",owner.Data1); return S_OK; }
    HRESULT STDMETHODCALLTYPE OnUpdateComposition(ITfCompositionView*,ITfRange*) override { std::puts("EVENT composition_update"); return S_OK; }
    HRESULT STDMETHODCALLTYPE OnEndComposition(ITfCompositionView* view) override { composing=false; ++ends; ComPtr<ITfRange> range; ComPtr<ITfRangeACP> acp; LONG at=0,length=0; GUID owner{}; view->GetOwnerClsid(&owner); if(SUCCEEDED(view->GetRange(&range)) && SUCCEEDED(range.As(&acp)) && SUCCEEDED(acp->GetExtent(&at,&length)) && at>=0 && length>=0 && at+length<=(LONG)text.size()) { auto part=text.substr(at,length); endedText+=part; std::printf("EVENT composition_end owner=%08lx at=%ld length=%ld text=%s\n",owner.Data1,at,length,utf8(part).c_str()); } else std::puts("EVENT composition_end range_unavailable"); return S_OK; }
    std::wstring text;
    HWND window = nullptr;
    RECT screenBounds{};
    bool layoutReady = false;
    bool deferAsync=false, traceLocks=false;
    bool failNextSetText=false, rejectNextAsync=false, rejectSync=false, rejectContextRead=false, windowless=false;
    unsigned refusedSync=0, deferredWrites=0, refusedContext=0, refusedWindow=0, hostTabs=0;
    std::deque<DWORD> delayedLocks;
    void grantDelayed() { deferAsync=false; while(!delayedLocks.empty()) {DWORD flags=delayedLocks.front();delayedLocks.pop_front();requireGrant(flags);} }
    void requireGrant(DWORD flags) {if(!sink_ || FAILED(sink_->OnLockGranted(flags))) {std::puts("DELAYED_LOCK_FAILED");ExitProcess(120);} }
    void layoutChanged() { if (sink_) sink_->OnLayoutChange(TS_LC_CHANGE,1); }
    void externalCaret(LONG at) { selection.acpStart = selection.acpEnd = at; if (sink_) sink_->OnSelectionChange(); }
    TS_SELECTION_ACP selection{0,0,{TS_AE_NONE,FALSE}};
    HRESULT STDMETHODCALLTYPE QueryInterface(REFIID iid, void** out) override { *out=nullptr; if(iid==IID_IUnknown||iid==IID_ITextStoreACP) *out=static_cast<ITextStoreACP*>(this); else if(iid==IID_ITfContextOwnerCompositionSink) *out=static_cast<ITfContextOwnerCompositionSink*>(this); else return E_NOINTERFACE; AddRef(); return S_OK; }
    ULONG STDMETHODCALLTYPE AddRef() override { return InterlockedIncrement(&refs_); }
    ULONG STDMETHODCALLTYPE Release() override { auto n = InterlockedDecrement(&refs_); if (!n) delete this; return n; }
    HRESULT STDMETHODCALLTYPE AdviseSink(REFIID iid, IUnknown* sink, DWORD) override { if (iid != IID_ITextStoreACPSink) return E_NOINTERFACE; return sink->QueryInterface(IID_PPV_ARGS(&sink_)); }
    HRESULT STDMETHODCALLTYPE UnadviseSink(IUnknown*) override { sink_.Reset(); return S_OK; }
    HRESULT STDMETHODCALLTYPE RequestLock(DWORD flags, HRESULT* result) override {
        if (!sink_) return E_FAIL;
        if (rejectSync && (flags&TS_LF_SYNC) && (flags&TS_LF_READWRITE)==TS_LF_READWRITE) { ++refusedSync; *result=TS_E_SYNCHRONOUS; }
        else if (rejectNextAsync && !(flags&TS_LF_SYNC)) { rejectNextAsync=false; *result=E_FAIL; }
        else if (deferAsync && !(flags&TS_LF_SYNC)) { deferredWrites+=(flags&TS_LF_READWRITE)==TS_LF_READWRITE; delayedLocks.push_back(flags); *result=TS_S_ASYNC; }
        else { *result=sink_->OnLockGranted(flags); }
        if (traceLocks) std::printf("STORE lock_flags=%08lx outer=00000000 session=%08lx\n",flags,(unsigned long)*result);
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE GetStatus(TS_STATUS* out) override { *out = {0,0}; return S_OK; }
    HRESULT STDMETHODCALLTYPE QueryInsert(LONG start, LONG end, ULONG, LONG* a, LONG* b) override { *a = start; *b = end; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetSelection(ULONG, ULONG count, TS_SELECTION_ACP* out, ULONG* fetched) override {
        if (rejectContextRead && count) {
            rejectContextRead = false;
            ++refusedContext;
            *fetched = 0;
            return E_FAIL;
        }
        *fetched = count ? 1 : 0;
        if (count) *out = selection;
        return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetSelection(ULONG count, const TS_SELECTION_ACP* value) override { if (count != 1) return E_INVALIDARG; selection = *value; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetText(LONG start, LONG end, WCHAR* plain, ULONG capacity, ULONG* count, TS_RUNINFO* runs, ULONG runCapacity, ULONG* runCount, LONG* next) override {
        if (end == -1) end = (LONG)text.size(); if (start < 0 || end < start || end > (LONG)text.size()) return TS_E_INVALIDPOS;
        *count = std::min<ULONG>(capacity,end-start); if (*count) std::copy_n(text.data()+start,*count,plain);
        *runCount = runCapacity ? 1 : 0; if (runCapacity) runs[0] = {*count,TS_RT_PLAIN}; *next = start + *count; return S_OK;
    }
    HRESULT STDMETHODCALLTYPE SetText(DWORD, LONG start, LONG end, const WCHAR* value, ULONG count, TS_TEXTCHANGE* change) override {
        if(failNextSetText) {failNextSetText=false;std::puts("FAULT retirement SetText E_FAIL");return E_FAIL;}
        if (start < 0 || end < start || end > (LONG)text.size()) return TS_E_INVALIDPOS;
        if(count && !composing) ++earlyWrites; std::printf("EVENT set_text composing=%d start=%ld end=%ld length=%lu\n",composing,start,end,count);
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
    HRESULT STDMETHODCALLTYPE GetScreenExt(TsViewCookie,RECT* out) override { *out=screenBounds; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetWnd(TsViewCookie,HWND* out) override {
        *out=window;
        if (windowless) { ++refusedWindow; return E_FAIL; }
        return S_OK;
    }
};
class StyledRanges final : public Microsoft::WRL::RuntimeClass<Microsoft::WRL::RuntimeClassFlags<Microsoft::WRL::ClassicCom>,ITfEditSession> {
    ITfContext* context_;
    TfGuidAtom atom_;
public:
    std::vector<std::pair<LONG,LONG>> ranges;
    StyledRanges(ITfContext* context,TfGuidAtom atom):context_(context),atom_(atom) {}
    HRESULT STDMETHODCALLTYPE DoEditSession(TfEditCookie cookie) override {
        ComPtr<ITfProperty> property;
        auto hr=context_->GetProperty(GUID_PROP_ATTRIBUTE,&property);
        if(FAILED(hr))return hr;
        ComPtr<IEnumTfRanges> items;
        hr=property->EnumRanges(cookie,&items,nullptr);
        if(FAILED(hr))return hr;
        ComPtr<ITfRange> item;ULONG fetched=0;
        while(items->Next(1,&item,&fetched)==S_OK && fetched==1) {
            VARIANT attribute{};hr=property->GetValue(cookie,item.Get(),&attribute);
            bool ours=SUCCEEDED(hr)&&attribute.vt==VT_I4&&static_cast<TfGuidAtom>(attribute.lVal)==atom_;
            VariantClear(&attribute);
            if(ours) {
                LONG start=0,length=0;ComPtr<ITfRangeACP> acp;
                if(FAILED(hr=item.As(&acp))||FAILED(hr=acp->GetExtent(&start,&length)))return hr;
                ranges.emplace_back(start,length);
            }
            item.Reset();
        }
        return S_OK;
    }
};
#include <cstdlib>
#include "shanjie.h"
void require(bool condition,int check) { if(!condition) { std::printf("PROBE_SETUP_FAIL=%d\n",check); std::fflush(stdout); ExitProcess(check); } }
std::string utf8(const std::wstring& value) { if(value.empty()) return {}; int count=WideCharToMultiByte(CP_UTF8,0,value.data(),(int)value.size(),nullptr,0,nullptr,nullptr); std::string out(count,'\0'); WideCharToMultiByte(CP_UTF8,0,value.data(),(int)value.size(),out.data(),count,nullptr,nullptr); return out; }
struct KeyboardRestore { BYTE previous[256]{}; KeyboardRestore(){require(GetKeyboardState(previous),101);} ~KeyboardRestore(){SetKeyboardState(previous);} };
void keyboardState(bool caps,bool shift) {
 // Initialize the input queue's individual-key cache before setting the fixture.
 for(int key:{VK_LWIN,VK_RWIN,VK_CONTROL,VK_MENU,VK_SHIFT,VK_CAPITAL})GetKeyState(key);
 BYTE state[256]{};state[VK_CAPITAL]=caps?1:0;state[VK_SHIFT]=shift?0x80:0;state[VK_LSHIFT]=shift?0x80:0;
 require(SetKeyboardState(state),102);require(((GetKeyState(VK_CAPITAL)&1)!=0)==caps,103);
}
int wmain(int argc,wchar_t** argv) {
 std::setvbuf(stdout,nullptr,_IONBF,0);
 std::setvbuf(stderr,nullptr,_IONBF,0);
 const auto harnessStart=std::chrono::steady_clock::now();
 require(argc==5||(argc>5&&argc%2==1&&std::wstring_view(argv[2])==L"input-digits"),1);
 std::wstring mode=argv[2],sequence=argv[3],expected=argv[4];
 const bool digitBatch=mode==L"input-digits";
 auto desktopName=L"Shanjie.Submission."+std::to_wstring(GetCurrentProcessId());
 auto desktop=CreateDesktopW(desktopName.c_str(),nullptr,nullptr,0,GENERIC_ALL,nullptr);
 require(desktop!=nullptr&&SetThreadDesktop(desktop),980);
 std::puts("PRIVATE DESKTOP ready (never switched)");std::fflush(stdout);
 KeyboardRestore restore; keyboardState(false,false);
 require(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)),2);
 HMODULE dll=LoadLibraryW(argv[1]); require(dll!=nullptr,3);
 if(mode.starts_with(L"input-")) {
   auto load=(char* (__cdecl*)())GetProcAddress(dll,"sj_profile_load_text");
   auto release=(void (__cdecl*)(char*))GetProcAddress(dll,"sj_string_free");
   require(load&&release,73);char* profile=load();require(profile!=nullptr,74);
   std::printf("FIXTURE symbols_off=%d mixed=%d\n",std::string(profile).find("space_inputs_zhuyin=0")!=std::string::npos,std::string(profile).find("mixed_input=1")!=std::string::npos);
   release(profile);
 }
 auto factoryFn=(HRESULT (WINAPI*)(REFCLSID,REFIID,void**))GetProcAddress(dll,"DllGetClassObject");require(factoryFn!=nullptr,4);
 auto resident=(unsigned long long (__cdecl*)())GetProcAddress(dll,"sj_resident_status");require(resident!=nullptr,5);
 ComPtr<IClassFactory> factory;require(SUCCEEDED(factoryFn(ServiceId,IID_PPV_ARGS(&factory))),6);
 ComPtr<ITfTextInputProcessorEx> service;require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&service))),7);
 ComPtr<ITfThreadMgr> thread;require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&thread))),8);
 ComPtr<ITfThreadMgrEx> extended;require(SUCCEEDED(thread.As(&extended)),9);
 TfClientId client;require(SUCCEEDED(extended->ActivateEx(&client,TF_TMAE_NOACTIVATETIP|TF_TMAE_NOACTIVATEKEYBOARDLAYOUT)),9);
 ComPtr<ITfDocumentMgr> document;
 ComPtr<Store> first;
 ComPtr<ITfContext> firstContext;
 auto createDocument=[&]{
   require(SUCCEEDED(thread->CreateDocumentMgr(document.ReleaseAndGetAddressOf())),10);
   first.Attach(new Store);
   TfEditCookie cookie;
   require(SUCCEEDED(document->CreateContext(client,0,static_cast<ITextStoreACP*>(first.Get()),firstContext.ReleaseAndGetAddressOf(),&cookie)),11);
   require(SUCCEEDED(document->Push(firstContext.Get()))&&SUCCEEDED(thread->SetFocus(document.Get())),12);
 };
 createDocument();
 ComPtr<ITfClientId> identifiers;TfClientId serviceClient=TF_CLIENTID_NULL;
 require(SUCCEEDED(thread.As(&identifiers))&&SUCCEEDED(identifiers->GetClientId(ServiceId,&serviceClient)),13);
 ComPtr<ThreadProxy> proxy;proxy.Attach(new ThreadProxy(thread.Get(),true));
 auto logStage=[&](const char* stage, const char* detail="") {
   auto status=resident();
   auto elapsed=std::chrono::duration_cast<std::chrono::milliseconds>(std::chrono::steady_clock::now()-harnessStart).count();
   std::printf("STAGE name=%s elapsed_ms=%lld started=%u completed=%u composing=%d ends=%u%s%s\n",
     stage,(long long)elapsed,static_cast<unsigned>(status),static_cast<unsigned>(status>>32),
     first->composing?1:0,first->ends,detail[0]?" ":"",detail);
   std::fflush(stdout);
 };
 if(mode.starts_with(L"rebuild-")) logStage("initial-core-start");
 require(SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),14);
 auto start=std::chrono::steady_clock::now();while((resident()>>32)!=(resident()&0xffffffffULL)){require(std::chrono::steady_clock::now()-start<std::chrono::seconds(35),15);Sleep(5);}
 if(mode.starts_with(L"rebuild-")) logStage("initial-core-ready");
 ComPtr<ITfKeyEventSink> keys;require(SUCCEEDED(service.As(&keys)),16);
 auto context=firstContext;auto store=first;
 auto pump=[] {auto start=std::chrono::steady_clock::now();while(std::chrono::steady_clock::now()-start<std::chrono::milliseconds(350)){MSG message{};while(PeekMessageW(&message,nullptr,0,0,PM_REMOVE)){TranslateMessage(&message);DispatchMessageW(&message);}Sleep(5);}};
 auto press=[&](WPARAM requested, bool shifted=false){WPARAM vk=requested;
   if(mode.starts_with(L"input-")) {
     if(requested==L'\\')vk=VK_OEM_5;
     else if(requested==L'.')vk=VK_OEM_PERIOD;
     else if(requested==L'/')vk=VK_OEM_2;
     else if(requested==L'⌫')vk=VK_BACK;
     else if(requested==L'⎋')vk=VK_ESCAPE;
     else if(requested==L'('){vk='9';shifted=true;}
     else if(requested==L')'){vk='0';shifted=true;}
     else if(requested==L'↓')vk=VK_DOWN;
     else if(requested==L'⌂')vk=VK_HOME;
   }
   keyboardState(false,shifted);BOOL tested=FALSE,eaten=FALSE;LPARAM flags=(LPARAM(MapVirtualKeyW((UINT)vk,MAPVK_VK_TO_VSC))<<16)|1;
   auto testedHr=keys->OnTestKeyDown(context.Get(),vk,flags,&tested);
   require(SUCCEEDED(testedHr),17);
   if(!tested) {
     ComPtr<ITfCompartmentMgr> manager;VARIANT value{};ComPtr<ITfCompartment> compartment;
     proxy.As(&manager);manager->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&compartment);compartment->GetValue(&value);
     std::printf("TEST_FALSE chinese=%ld win=%04x/%04x ctrl=%04x alt=%04x shift=%04x\n",value.vt==VT_I4?value.lVal:-1,GetKeyState(VK_LWIN)&65535,GetKeyState(VK_RWIN)&65535,GetKeyState(VK_CONTROL)&65535,GetKeyState(VK_MENU)&65535,GetKeyState(VK_SHIFT)&65535);VariantClear(&value);
     context.As(&manager);for(auto id:{GUID_COMPARTMENT_KEYBOARD_DISABLED,GUID_COMPARTMENT_EMPTYCONTEXT}){compartment.Reset();manager->GetCompartment(id,&compartment);compartment->GetValue(&value);std::printf("CONTEXT_DISABLED %ld\n",value.vt==VT_I4?value.lVal:-1);VariantClear(&value);}
   }
   HRESULT downHr=S_FALSE;
   if(tested){downHr=keys->OnKeyDown(context.Get(),vk,flags,&eaten);require(SUCCEEDED(downHr),18);}
   if(mode==L"async-keys") {
     if(vk==VK_TAB&&!eaten) {++store->hostTabs;std::printf("HOST original_tab=%u shift=%d\n",store->hostTabs,shifted);}
     std::printf("TSF key=%llu test_hr=%08lx down_called=%d down_hr=%08lx eaten=%d\n",
       (unsigned long long)vk,(unsigned long)testedHr,tested,(unsigned long)downHr,eaten);
   }
   if(!eaten && !mode.starts_with(L"input-") && vk>=32 && vk<=126){std::wstring literal(1,(wchar_t)std::towlower((wchar_t)vk));store->text.replace(store->selection.acpStart,store->selection.acpEnd-store->selection.acpStart,literal);store->selection.acpStart=store->selection.acpEnd=store->selection.acpStart+1;}
   std::printf("KEY vk=%llu tested=%d eaten=%d text=%s\n",(unsigned long long)vk,tested,eaten,utf8(store->text).c_str());
   return eaten!=FALSE;
 };
 bool deactivated=false;
 if(mode!=L"async-keys" && mode!=L"segment" && !mode.starts_with(L"rebuild-") && !mode.starts_with(L"input-")) for(wchar_t vk:sequence)press(vk);
 if(mode.starts_with(L"rebuild-")) {
   const std::wstring pack=sequence==L"SU3" ? L"QI SU6" : L"PO NE2";
   const auto type=[&](const std::wstring& characters){for(wchar_t key:characters)press(key);};
   wchar_t local[32768]{};require(GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768)>0,216);
   const auto path=std::wstring(local)+L"\\Shanjie\\profile.ini";
   auto profile=[&](bool enable, bool rerank, const char* dictionary) {
     std::ifstream in(path,std::ios::binary);std::string content((std::istreambuf_iterator<char>(in)),{});in.close();
     const auto setting=[&](std::string_view name, std::string value){auto at=content.find(name);require(at!=std::string::npos,217);at+=name.size();content.replace(at,content.find_first_of("\r\n",at)-at,value);};
     setting("acg_pack=",enable?"1":"0");setting("rerank=",rerank?"1":"0");setting("dictionary=",dictionary);
     std::ofstream out(path,std::ios::binary|std::ios::trunc);out<<content;out.close();localNotify(L"Shanjie.ProfileChanged.v2");
   };
   auto settle=[&](unsigned builds){
     auto deadline=std::chrono::steady_clock::now()+std::chrono::seconds(25);
     do { pump(); require(std::chrono::steady_clock::now()<deadline,218); } while((resident()>>32)<builds);
     pump();
   };
   auto candidatesContain=[&](const std::wstring& wanted){
     ComPtr<ITfUIElementMgr> manager;require(SUCCEEDED(thread.As(&manager)),219);
     ComPtr<IEnumTfUIElements> elements;require(SUCCEEDED(manager->EnumUIElements(&elements)),220);
     ComPtr<ITfUIElement> element;ULONG count=0;
     while(SUCCEEDED(elements->Next(1,&element,&count)) && count==1) {
       ComPtr<ITfCandidateListUIElement> list;
       if(SUCCEEDED(element.As(&list))) {
         UINT length=0;require(SUCCEEDED(list->GetCount(&length)),221);
         for(UINT i=0;i<length;++i) {BSTR value=nullptr;require(SUCCEEDED(list->GetString(i,&value)),222);bool match=value&&wanted==value;SysFreeString(value);if(match)return true;}
       }
       element.Reset();
     }
     return false;
   };
   // Confirm that the first keys made a round trip before testing profile changes.
   // A slow cold pipe can leave them unconfirmed until Escape.
   for(int attempt=0;;++attempt) {
     require(attempt<5,232);
     type(sequence);
     for(auto deadline=std::chrono::steady_clock::now()+std::chrono::seconds(3);first->text.empty()&&std::chrono::steady_clock::now()<deadline;) pump();
     const bool confirmed=!first->text.empty();
     for(int i=0;i<4&&(i==0||first->composing||!first->text.empty());++i){press(VK_ESCAPE);pump();}
     require(!first->composing&&first->text.empty(),233);
     std::printf("STAGE warm attempt=%d confirmed=%d\n",attempt,confirmed);
     if(confirmed) break;
   }
   first->ends=0;first->starts=0;first->endedText.clear();first->earlyWrites=0;
   auto builds=static_cast<unsigned>(resident());
   type(sequence);
   require(first->composing && !first->text.empty(),215);
   logStage("first-profile-update");
   profile(true,true,"missing.tsv");
   // A refresh waits for queued keys (a busy pipe) before it starts the rebuild; it still must start while composing.
   for(auto deadline=std::chrono::steady_clock::now()+std::chrono::seconds(10);static_cast<unsigned>(resident())==builds;pump()) require(std::chrono::steady_clock::now()<deadline,223);
   require(static_cast<unsigned>(resident())==builds+1&&first->composing,223);
   logStage("first-background-build-started");
   press(VK_DOWN); press('1');
   const auto firstWord=first->text;
   settle(++builds);
   logStage("first-background-build-settled");
   require(first->composing && first->ends==0 && first->text==firstWord,224);
   press(VK_RETURN); require(first->endedText==firstWord&&!first->composing,225);
   logStage("first-commit-done");

   type(sequence);
   logStage("second-consecutive-profile-update");
   profile(true,true,"personal.tsv");
   profile(false,false,"personal.tsv");
   if(static_cast<unsigned>(resident())!=builds) {
     require(static_cast<unsigned>(resident())==builds+1,226); // Two requests, only the first worker started.
     logStage("second-background-build-started","detail=immediate");
   }
   else {
     // Deferred behind queued keys: both requests collapse into the latest profile.
     for(auto deadline=std::chrono::steady_clock::now()+std::chrono::seconds(10);static_cast<unsigned>(resident())==builds;pump()) require(std::chrono::steady_clock::now()<deadline,226);
     require(static_cast<unsigned>(resident())==builds+1,226); --builds;
     logStage("second-background-build-started","detail=deferred-collapsed");
   }
   settle(builds+=2);
   logStage("second-background-build-settled");
   require(first->ends==1 && first->composing,227);
   press(VK_ESCAPE);pump();
   require(first->text==firstWord && first->endedText==firstWord,228);
   type(pack);press(VK_HOME);press(VK_DOWN);
   require(!candidatesContain(expected) && !first->text.ends_with(expected),229);
   press(VK_ESCAPE);press(VK_ESCAPE);
   logStage("second-pack-check-done");

   type(pack);
   const auto oldText=first->text;
   logStage("third-profile-update");
   profile(true,false,"personal.tsv");
   settle(++builds);
   logStage("third-background-build-settled");
   require(first->composing && first->text==oldText && first->endedText==firstWord,230);
   press(VK_RETURN);pump();
   type(pack);press(VK_HOME);press(VK_DOWN);
   require(candidatesContain(expected) || first->text.ends_with(expected),231);
   press(VK_ESCAPE);press(VK_ESCAPE);
   type(sequence);press(VK_DOWN);
   require(candidatesContain(L"龘") || first->text.ends_with(L"龘"),232);
   press(VK_ESCAPE);press(VK_ESCAPE);
   logStage("third-pack-and-dict-check-done");
   logStage("deactivate-and-retire-start");
   profile(false,false,"personal.tsv");
   require(SUCCEEDED(service->Deactivate()),233);deactivated=true;
   const auto afterDeactivate=first->text;
   settle(++builds);
   logStage("deactivate-and-retire-settled");
   require(!first->composing && first->text==afterDeactivate,234);
 } else if(mode==L"segment") {
   ComPtr<ITfDisplayAttributeProvider> styling;
   require(SUCCEEDED(service.As(&styling)),235);
   ComPtr<IEnumTfDisplayAttributeInfo> styles;require(SUCCEEDED(styling->EnumDisplayAttributeInfo(&styles)),236);
   ComPtr<ITfDisplayAttributeInfo> style;ULONG fetched=0;
   require(styles->Next(1,&style,&fetched)==S_OK && fetched==1,237);
   ComPtr<ITfDisplayAttributeInfo> done;require(styles->Next(1,&done,&fetched)==S_FALSE && fetched==0,238);
   GUID guid{};TF_DISPLAYATTRIBUTE appearance{};
   require(SUCCEEDED(style->GetGUID(&guid)) && SUCCEEDED(style->GetAttributeInfo(&appearance)) && appearance.lsStyle==TF_LS_SOLID && appearance.fBoldLine,239);
   ComPtr<ITfCategoryMgr> category;TfGuidAtom atom=0;
   require(SUCCEEDED(CoCreateInstance(CLSID_TF_CategoryMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&category)))&&SUCCEEDED(category->RegisterGUID(guid,&atom)),240);
   auto ranges=[&] {
     auto edit=Microsoft::WRL::Make<StyledRanges>(context.Get(),atom);
     HRESULT result=E_FAIL;
     require(SUCCEEDED(context->RequestEditSession(serviceClient,edit.Get(),TF_ES_READ|TF_ES_SYNC,&result))&&SUCCEEDED(result),241);
     return edit->ranges;
   };
   const std::wstring prefix=L"seed 🧸 ";const auto offset=static_cast<LONG>(prefix.size());
   first->text=prefix;first->externalCaret(offset);
   auto type=[&]{for(auto vk:sequence)press(vk);};
   for(auto i=0;i<8;++i)type();
   require(first->text==prefix+expected && first->composing && ranges().empty(),242);
   press(VK_HOME);
   require(ranges()==std::vector{std::pair{offset,1L}},243);
   for(auto i=0;i<3;++i)press(VK_RIGHT);
   const bool right=sequence==L"SU3";
   require(ranges()==std::vector{std::pair{offset+(right?3L:2L),1L}},244);
   press(VK_DELETE);require(first->text==prefix+expected.substr(1),245);
   type();require(first->text==prefix+expected,246);
   press(VK_DOWN);press('1');press(VK_DOWN);press('1');
   require(first->composing && first->text.size()==prefix.size()+expected.size(),247);
   press(VK_END);require(ranges().empty(),248);
   press(VK_LEFT);press(VK_LEFT);
   const auto before=first->text;
   const auto boundary=first->selection.acpStart;
   require(!ranges().empty(),249);
   press(VK_RETURN);
   require(first->text==before && first->endedText==before.substr(offset,boundary-offset)&&first->composing,250);
   press(VK_ESCAPE);
   require(first->text==before.substr(0,boundary)&&!first->composing&&ranges().empty(),251);
   for(auto i=0;i<2;++i) {
     type();for(wchar_t vk:std::wstring(right?L"CL3":L"HZ3"))press(vk);
   }
   require(first->text.ends_with(L"你好你好"),252);
   press(VK_HOME);press(VK_RIGHT);
   require(ranges()==std::vector{std::pair{boundary,2L}},253);
   press(VK_RIGHT);press(VK_RIGHT);
   require(ranges()==std::vector{std::pair{boundary+2L,2L}},254);
   press(VK_ESCAPE);
 } else if(mode==L"async-keys") {
   first->text=L"seed "; first->externalCaret(5);
   first->windowless=true; first->rejectSync=true; first->deferAsync=true; first->traceLocks=true;
   first->rejectContextRead=true;
   for(wchar_t vk:sequence) press(vk);
   press(VK_DOWN); press('1'); press(VK_RETURN);
   for(wchar_t vk:std::wstring(L"ADB")) press(vk);
   press(VK_ESCAPE);
   pump();
   require(first->text==L"seed " && first->starts==0 && first->ends==0,211);
   require(first->refusedSync==1 && first->deferredWrites==1,212);
   first->rejectSync=false;
   first->grantDelayed();
   // The optional context read can fail, but the first granted edit must still compose.
   require(first->starts==1 && first->text.size()>5,295);
   pump();
   require(first->refusedContext>0,213);
   require(first->text==L"seed "+expected && first->endedText==expected && !first->composing,214);
   require(first->starts==2 && first->ends==2 && first->earlyWrites==0,215);

   // Probe both HRESULTs from real MSCTF, independently of the TIP's decision.
   auto probe=Microsoft::WRL::Make<EditProbe>();
   require(probe!=nullptr,255);
   auto request=[&](DWORD flags, HRESULT expectedSession, unsigned expectedCalls){
     HRESULT session=E_FAIL;
     auto outer=context->RequestEditSession(serviceClient,probe.Get(),flags|TF_ES_READWRITE,&session);
     std::printf("EDIT flags=%08lx outer=%08lx phrSession=%08lx DoEditSession.calls=%u\n",
       flags,(unsigned long)outer,(unsigned long)session,probe->calls);
     require(outer==S_OK && session==expectedSession && probe->calls==expectedCalls,256);
   };
   request(TF_ES_SYNC,S_OK,1);
   first->rejectSync=true;first->deferAsync=true;
   request(TF_ES_SYNC,TF_E_SYNCHRONOUS,1);
   request(TF_ES_ASYNCDONTCARE,TF_S_ASYNC,1);
   first->rejectSync=false;first->grantDelayed();pump();
   require(probe->calls==2,257);
   std::printf("EDIT deferred callback completed: DoEditSession.calls=%u\n",probe->calls);

   wchar_t local[32768]{};require(GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768)>0,258);
   const auto profile=std::wstring(local)+L"\\Shanjie\\profile.ini";
   wchar_t interaction[16]{};
   GetPrivateProfileStringW(L"profile",L"interaction",L"windows",interaction,16,profile.c_str());
   const bool windows=std::wstring_view(interaction)==L"windows";
   auto type=[&]{ for(wchar_t vk:sequence) require(press(vk),259); };
   auto delay=[&]{ first->rejectSync=true; first->deferAsync=true; };
   auto grant=[&]{ first->rejectSync=false; first->grantDelayed(); pump(); };
   auto hostTab=[&](bool shifted=false){
     const auto count=first->hostTabs;
     require(!press(VK_TAB,shifted) && first->hostTabs==count+1,260);
   };
   auto candidateSelection=[&]{
     ComPtr<ITfUIElementMgr> manager;
     ComPtr<IEnumTfUIElements> elements;
     require(SUCCEEDED(thread.As(&manager)) && SUCCEEDED(manager->EnumUIElements(&elements)),261);
     ComPtr<ITfUIElement> element;
     ULONG count=0;require(elements->Next(1,&element,&count)==S_OK && count==1,262);
     ComPtr<ITfCandidateListUIElement> list;
     UINT selection=UINT_MAX;
     require(SUCCEEDED(element.As(&list)) && SUCCEEDED(list->GetSelection(&selection)),263);
     return selection;
   };

   // An idle host Tab also bypasses an earlier IME write waiting for its lock.
   const auto before=first->text;
   hostTab(); hostTab(true);
   delay();
   require(press(sequence.front()),264);
   pump();
   auto writes=first->deferredWrites;
   hostTab();
   require(first->deferredWrites==writes && first->text==before,265);
   for(wchar_t vk:sequence.substr(1)) require(press(vk),266);
   require(press(VK_RETURN),267);
   grant();
   require(first->text==before+expected && first->endedText==expected+expected && !first->composing,268);

   // With a valid screen extent, position candidates far enough to reach
   // GetWnd even though this text store has no window and no text layout.
   const auto refusedWindow=first->refusedWindow;
   first->screenBounds={100,120,800,600};
   type();require(press(VK_DOWN),269);
   require(first->refusedWindow>refusedWindow,294);
   std::printf("WINDOWLESS GetWnd failures=%u; refused first context read=%u\n",first->refusedWindow,first->refusedContext);
   const auto composing=first->text;
   const auto ends=first->ends;
   const auto selected=candidateSelection();
   writes=first->deferredWrites;
   delay();
   if(windows) {
     // Two navigation Tabs target a confirmed Windows candidate list.
     require(press(VK_TAB)&&press(VK_TAB),270);
     pump();
     require(first->deferredWrites==writes+1 && candidateSelection()==selected,271);
     grant();
     require(candidateSelection()==selected+2 && first->text==composing && first->ends==ends,272);
     delay();
     require(press(VK_ESCAPE),273);
     pump();
     writes=first->deferredWrites;
     // This Tab follows a candidate-closing key whose edit has not run yet.
     hostTab();
     require(first->deferredWrites==writes,274);
     grant();
     require(first->text==composing && first->ends==ends && press(VK_RETURN),275);

     require(WritePrivateProfileStringW(L"engine",L"prediction",L"1",profile.c_str()),296);
     localNotify(L"Shanjie.ProfileChanged.v2");pump();
     type();require(press(VK_DOWN),297);
     ComPtr<ITfUIElementMgr> manager;ComPtr<IEnumTfUIElements> elements;ComPtr<ITfUIElement> element;ComPtr<ITfCandidateListUIElement> list;ULONG count=0;BSTR top=nullptr;
     require(SUCCEEDED(thread.As(&manager))&&SUCCEEDED(manager->EnumUIElements(&elements))&&elements->Next(1,&element,&count)==S_OK&&count==1&&SUCCEEDED(element.As(&list))&&SUCCEEDED(list->GetString(0,&top)),298);
     const std::wstring topWord=top?top:L"";SysFreeString(top);
     require(topWord.size()>expected.size()&&press(VK_ESCAPE)&&press(VK_ESCAPE)&&!first->composing,299);
     require(WritePrivateProfileStringW(L"engine",L"prediction",L"0",profile.c_str()),300);
     localNotify(L"Shanjie.ProfileChanged.v2");pump();
   } else {
     const auto refused=first->refusedSync;
     // The Shanjie interaction may finish the composition and give Tab to
     // the host. If sync is refused there is no delayed edit for that key.
     hostTab();hostTab(true);
     require(first->refusedSync==refused+2 && first->deferredWrites==writes,276);
     grant();
     require(first->text==composing && first->ends==ends && candidateSelection()==selected,277);
     hostTab();
     require(first->text==composing && first->ends==ends+1 && !first->composing,278);

     require(press(sequence.front()),291);
     const auto unfinished=first->text;
     delay();require(press(VK_TAB),292);grant();
     require(first->composing && first->text==unfinished && press(VK_ESCAPE),293);

     require(WritePrivateProfileStringW(L"engine",L"prediction",L"1",profile.c_str()),279);
     localNotify(L"Shanjie.ProfileChanged.v2");pump();
     require(press(sequence.front()),280);
     delay();
     require(press(VK_TAB),281);
     grant();
     require(candidateSelection()==0,282);
     delay();
     require(press(VK_TAB,true),283);grant();
     require(first->composing && press(VK_ESCAPE) && !first->composing,284);
   }

   // Ending a context with an edit in flight must invalidate that callback,
   // rather than letting its key land in the next composition.
   for(bool deactivate : {false,true}) {
     const auto stable=first->text;
     const auto starts=first->starts;
     delay();require(press(sequence.front()),285);
     if(deactivate) {
       require(SUCCEEDED(service->Deactivate())
         && SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),286);
     } else {
       ComPtr<ITfThreadFocusSink> focus;
       require(SUCCEEDED(service.As(&focus)) && SUCCEEDED(focus->OnKillThreadFocus())
         && SUCCEEDED(focus->OnSetThreadFocus()),287);
     }
     grant();
     require(first->text==stable && first->starts==starts && !first->composing,288);
     hostTab();
     type();require(press(VK_RETURN),289);
     require(first->text==stable+expected && !first->composing,290);
   }
 } else if(mode.starts_with(L"input-")) {
   const auto initialLoadCounts=digitBatch ? resident() : 0;
   for(int argument=3;argument<argc;argument+=2){
     const int index=(argument-3)/2;
     const auto caseStart=std::chrono::steady_clock::now();
     if(digitBatch)std::printf("STAGE name=input-digits index=%d\n",index);
     if(index){
       // Retire the preceding context and candidate UI while keeping the loaded engine.
       require(SUCCEEDED(keys->OnSetFocus(FALSE))&&SUCCEEDED(thread->SetFocus(nullptr))&&SUCCEEDED(document->Pop(TF_POPF_ALL)),270);
       ComPtr<ITfUIElementMgr> manager;ComPtr<IEnumTfUIElements> elements;
       ComPtr<ITfUIElement> element;ULONG count=0;
       require(SUCCEEDED(thread.As(&manager))&&SUCCEEDED(manager->EnumUIElements(&elements))&&elements->Next(1,&element,&count)==S_FALSE&&count==0,271);
       createDocument();
       context=firstContext;store=first;
       require(store->text.empty()&&store->endedText.empty()&&!store->composing&&store->ends==0,272);
     }
     const std::wstring_view want=argv[argument+1];
     for(wchar_t vk:std::wstring_view(argv[argument]))press(vk);
     if(mode==L"input-space-commit") {
       require(store->text==want && store->endedText==want && !store->composing,70);
     } else {
       std::printf("INPUT composing=%d text=%s committed=%s\n",store->composing,utf8(store->text).c_str(),utf8(store->endedText).c_str());
       require(store->text==want && store->endedText.empty() && store->composing,71);
       press(VK_RETURN);require(store->text==want&&store->endedText==want&&!store->composing,72);
     }
     if(digitBatch){
       require(resident()==initialLoadCounts,273);
       const auto elapsed=std::chrono::duration<double,std::milli>(std::chrono::steady_clock::now()-caseStart).count();
       std::printf("INPUT PASS index=%d elapsed_ms=%.3f\n",index,elapsed);
     }
   }
 } else if(mode.starts_with(L"retire-")) {
   require(store->text==expected&&store->composing,200);
   ComPtr<ITfThreadFocusSink> focus;require(SUCCEEDED(service.As(&focus)),201);
   first->deferAsync=true;
   first->rejectNextAsync=(mode==L"retire-request-fail");
   require(SUCCEEDED(focus->OnKillThreadFocus()),202);
   if(mode==L"retire-write-fail" || mode==L"retire-escape") {
     require(!first->delayedLocks.empty(),203);
     first->failNextSetText=true;first->grantDelayed();
   }
   if(mode==L"retire-stale") Sleep(800);
   if(mode==L"retire-deactivate") {
     require(SUCCEEDED(service->Deactivate())&&SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),204);
   }
   if(mode==L"retire-escape") press(VK_ESCAPE);
   // Failed edits retry on re-entry; stale queued callbacks must not touch the new composition.
   press('X');
   require(first->text==expected+L"x"&&first->endedText==expected&&first->composing,205);
   first->grantDelayed();pump();
   require(first->text==expected+L"x"&&first->endedText==expected&&first->composing,206);
   press(VK_RETURN);
   require(first->text==expected+L"x"&&first->endedText==expected+L"x"&&!first->composing,207);
 } else if(mode.starts_with(L"busy")) {
   if(mode==L"busy-pick"||mode==L"busy-mouse")press(VK_DOWN);
   wchar_t local[32768]{};require(GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768)>0,50);
   std::wstring folder=std::wstring(local)+L"\\Shanjie",name=folder;
   std::transform(name.begin(),name.end(),name.begin(),[](wchar_t ch){return (wchar_t)std::towupper(ch);});std::replace(name.begin(),name.end(),L'\\',L'_');name=L"Local\\ShanjieLearning-"+name;
   HANDLE ready=CreateEventW(nullptr,TRUE,FALSE,nullptr);require(ready!=nullptr,51);
   std::thread holder([&]{HANDLE mutex=CreateMutexW(nullptr,FALSE,name.c_str());require(mutex!=nullptr&&WaitForSingleObject(mutex,1000)==WAIT_OBJECT_0,52);SetEvent(ready);Sleep(mode==L"busy-expire"?6000:900);
     if(mode==L"busy-epoch"){std::ofstream epoch(folder+L"\\learning.epoch",std::ios::binary);epoch<<"new isolated epoch";}
     ReleaseMutex(mutex);CloseHandle(mutex);});
   require(WaitForSingleObject(ready,2000)==WAIT_OBJECT_0,53);CloseHandle(ready);
   auto before=store->text;
   if(mode==L"busy-mouse"||mode==L"busy-pick") {
     ComPtr<ITfUIElementMgr> manager;require(SUCCEEDED(thread.As(&manager)),54);ComPtr<IEnumTfUIElements> elements;require(SUCCEEDED(manager->EnumUIElements(&elements)),55);
     ComPtr<ITfUIElement> element;ULONG count=0;require(SUCCEEDED(elements->Next(1,&element,&count))&&count==1,56);
     ComPtr<ITfCandidateListUIElementBehavior> candidates;require(SUCCEEDED(element.As(&candidates)),57);
     UINT length=0,pick=UINT_MAX;require(SUCCEEDED(candidates->GetCount(&length)),208);
     for(UINT i=0;i<length;++i) {BSTR word=nullptr;require(SUCCEEDED(candidates->GetString(i,&word)),209);if(word&&expected==word)pick=i;SysFreeString(word);}
     require(pick<9,210); // Select the word, independently of reading/literal rows and model ranking.
     if(mode==L"busy-mouse")require(SUCCEEDED(candidates->SetSelection(pick))&&SUCCEEDED(candidates->Finalize()),58);
     else press('1'+pick);
   } else press('D');
   require(store->text==before&&store->endedText.empty(),59);
   if(mode==L"busy-focus") {require(SUCCEEDED(keys->OnSetFocus(FALSE)),64);holder.join();pump();require(store->text==L"a"&&store->endedText==L"a"&&!store->composing,65);}
   else if(mode==L"busy-cancel") {press(VK_ESCAPE);holder.join();pump();require(store->text.empty()&&store->endedText.empty(),60);}
   else {
     if(mode==L"busy-expire") {for(int at=0;at<16;++at)pump();require(store->text==before,61);}
     holder.join();
     if(mode!=L"busy-pick"&&mode!=L"busy-mouse")press('B');
     pump();require(store->text==expected&&store->endedText.empty(),62);press(VK_RETURN);require(store->endedText==expected,63);
   }
 }
 else if(mode==L"cancel") {press(VK_ESCAPE);require(store->endedText.empty(),20);require(store->text.empty(),21);}
 else if(mode==L"cancel-generation"||mode==L"deactivate") {
   first->deferAsync=true;
   press(VK_RSHIFT);BOOL eaten=FALSE;require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_RSHIFT,0,&eaten))&&eaten,66);
   if(mode==L"deactivate") {require(SUCCEEDED(service->Deactivate())&&SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),67);}
   else press(VK_ESCAPE);
   for(wchar_t vk:std::wstring(L"XYZ"))press(vk);
   first->grantDelayed();pump();press(VK_RETURN);
   auto prefix=mode==L"deactivate"?expected:L"";
   require(first->text==prefix+L"xyz"&&first->endedText==prefix+L"xyz",68);
 }
 else if(mode==L"focus"||mode==L"toggle"||mode==L"refresh"||mode==L"refresh-twice"||mode==L"return") {
   first->deferAsync=true;
   if(mode==L"toggle"||mode==L"return") {press(VK_RSHIFT);BOOL eaten=FALSE;require(SUCCEEDED(keys->OnKeyUp(context.Get(),VK_RSHIFT,0,&eaten))&&eaten,22);}
   if(mode==L"refresh"||mode==L"refresh-twice") {
     wchar_t local[32768]{};require(GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768)>0,23);
     auto path=std::wstring(local)+L"\\Shanjie\\profile.ini";
     std::ifstream in(path,std::ios::binary);std::string content((std::istreambuf_iterator<char>(in)),{});in.close();
     auto at=content.find("prediction=1");require(at!=std::string::npos,24);content.replace(at,12,"prediction=0");
     std::ofstream out(path,std::ios::binary|std::ios::trunc);out<<content;out.close();localNotify(L"Shanjie.ProfileChanged.v2");
     if(mode==L"refresh-twice") {content.replace(at,12,"prediction=1");std::ofstream again(path,std::ios::binary|std::ios::trunc);again<<content;again.close();localNotify(L"Shanjie.ProfileChanged.v2");}
   }
   ComPtr<ITfDocumentMgr> second;require(SUCCEEDED(thread->CreateDocumentMgr(&second)),25);
   ComPtr<Store> other;other.Attach(new Store);ComPtr<ITfContext> otherContext;TfEditCookie otherCookie;
   require(SUCCEEDED(second->CreateContext(client,0,static_cast<ITextStoreACP*>(other.Get()),&otherContext,&otherCookie)),26);
   require(SUCCEEDED(second->Push(otherContext.Get()))&&SUCCEEDED(thread->SetFocus(second.Get())),27);
   context=otherContext;store=other;for(wchar_t vk:std::wstring(L"XYZ"))press(vk);
   require(first->endedText.empty(),28);require(first->delayedLocks.size()>0,29);
   if(mode==L"return") {
     require(SUCCEEDED(thread->SetFocus(document.Get())),33);context=firstContext;store=first;
     for(wchar_t vk:std::wstring(L"XYZ"))press(vk);
     require(first->text==expected,41);first->grantDelayed();pump();
     require(first->text==expected+L"xyz",42);press(VK_RETURN);require(first->endedText==expected+L"xyz",34);require(other->endedText==L"xyz",32);
   } else {
     first->grantDelayed();require(first->endedText==expected,30);require(other->text==L"xyz"&&other->endedText.empty()&&other->composing,31);
     press(VK_RETURN);require(other->endedText==L"xyz",32);
   }
   require(first->delayedLocks.empty(),35);
 } else {
   require(store->ends==0&&store->composing,36);
   press(VK_RETURN);require(store->text==expected,37);require(store->endedText==expected,38);require(!store->composing,39);
 }
 std::printf("PASS mode=%s text=%s submitted=%s\n",utf8(mode).c_str(),utf8(first->text).c_str(),utf8(first->endedText).c_str());
 if(!deactivated) require(SUCCEEDED(service->Deactivate()),40);thread->SetFocus(nullptr);document->Pop(TF_POPF_ALL);thread->Deactivate();std::fflush(stdout);return 0;
}
