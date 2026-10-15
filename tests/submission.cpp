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
#include <algorithm>
#include <chrono>
#include <cstdio>
#include <string>
#include <thread>
#include "identity.h"
using Microsoft::WRL::ComPtr;
#include "thread_proxy.h"
#include "local_notify.h"
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
    bool layoutReady = false;
    bool deferAsync=false;
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
    HRESULT STDMETHODCALLTYPE RequestLock(DWORD flags, HRESULT* result) override { if (!sink_) return E_FAIL; if(deferAsync && !(flags&TS_LF_SYNC)) {delayedLocks.push_back(flags);*result=TS_S_ASYNC;return S_OK;} *result = sink_->OnLockGranted(flags); return S_OK; }
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
    HRESULT STDMETHODCALLTYPE GetScreenExt(TsViewCookie,RECT* out) override { *out = {}; return S_OK; }
    HRESULT STDMETHODCALLTYPE GetWnd(TsViewCookie,HWND* out) override { *out = window; return S_OK; }
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
 require(argc==5,1); std::wstring mode=argv[2],sequence=argv[3],expected=argv[4];
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
 ComPtr<ITfDocumentMgr> document;require(SUCCEEDED(thread->CreateDocumentMgr(&document)),10);
 ComPtr<Store> first;first.Attach(new Store);ComPtr<ITfContext> firstContext;TfEditCookie cookie;
 require(SUCCEEDED(document->CreateContext(client,0,static_cast<ITextStoreACP*>(first.Get()),&firstContext,&cookie)),11);
 require(SUCCEEDED(document->Push(firstContext.Get()))&&SUCCEEDED(thread->SetFocus(document.Get())),12);
 ComPtr<ITfClientId> identifiers;TfClientId serviceClient=TF_CLIENTID_NULL;
 require(SUCCEEDED(thread.As(&identifiers))&&SUCCEEDED(identifiers->GetClientId(ServiceId,&serviceClient)),13);
 ComPtr<ThreadProxy> proxy;proxy.Attach(new ThreadProxy(thread.Get(),true));
 require(SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),14);
 auto start=std::chrono::steady_clock::now();while((resident()>>32)!=(resident()&0xffffffffULL)){require(std::chrono::steady_clock::now()-start<std::chrono::seconds(35),15);Sleep(5);}
 ComPtr<ITfKeyEventSink> keys;require(SUCCEEDED(service.As(&keys)),16);
 auto context=firstContext;auto store=first;
 auto pump=[] {auto start=std::chrono::steady_clock::now();while(std::chrono::steady_clock::now()-start<std::chrono::milliseconds(350)){MSG message{};while(PeekMessageW(&message,nullptr,0,0,PM_REMOVE)){TranslateMessage(&message);DispatchMessageW(&message);}Sleep(5);}};
 auto press=[&](WPARAM requested){WPARAM vk=requested;bool shifted=false;
   if(mode.starts_with(L"input-")) {
     if(requested==L'\\')vk=VK_OEM_5;
     else if(requested==L'('){vk='9';shifted=true;}
     else if(requested==L')'){vk='0';shifted=true;}
     else if(requested==L'↓')vk=VK_DOWN;
     else if(requested==L'⌂')vk=VK_HOME;
   }
   keyboardState(false,shifted);BOOL tested=FALSE,eaten=FALSE;LPARAM flags=(LPARAM(MapVirtualKeyW((UINT)vk,MAPVK_VK_TO_VSC))<<16)|1;
   require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),vk,flags,&tested)),17);
   if(!tested) {
     ComPtr<ITfCompartmentMgr> manager;VARIANT value{};ComPtr<ITfCompartment> compartment;
     proxy.As(&manager);manager->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&compartment);compartment->GetValue(&value);
     std::printf("TEST_FALSE chinese=%ld win=%04x/%04x ctrl=%04x alt=%04x shift=%04x\n",value.vt==VT_I4?value.lVal:-1,GetKeyState(VK_LWIN)&65535,GetKeyState(VK_RWIN)&65535,GetKeyState(VK_CONTROL)&65535,GetKeyState(VK_MENU)&65535,GetKeyState(VK_SHIFT)&65535);VariantClear(&value);
     context.As(&manager);for(auto id:{GUID_COMPARTMENT_KEYBOARD_DISABLED,GUID_COMPARTMENT_EMPTYCONTEXT}){compartment.Reset();manager->GetCompartment(id,&compartment);compartment->GetValue(&value);std::printf("CONTEXT_DISABLED %ld\n",value.vt==VT_I4?value.lVal:-1);VariantClear(&value);}
   }
   if(tested)require(SUCCEEDED(keys->OnKeyDown(context.Get(),vk,flags,&eaten)),18);
   if(!eaten && !mode.starts_with(L"input-") && vk>=32 && vk<=126){std::wstring literal(1,(wchar_t)std::towlower((wchar_t)vk));store->text.replace(store->selection.acpStart,store->selection.acpEnd-store->selection.acpStart,literal);store->selection.acpStart=store->selection.acpEnd=store->selection.acpStart+1;}
   std::printf("KEY vk=%llu tested=%d eaten=%d text=%s\n",(unsigned long long)vk,tested,eaten,utf8(store->text).c_str());
 };
 for(wchar_t vk:sequence)press(vk);
 if(mode.starts_with(L"input-")) {
   if(mode==L"input-space-commit") {
     require(store->text==expected && store->endedText==expected && !store->composing,70);
   } else {
     std::printf("INPUT composing=%d text=%s committed=%s\n",store->composing,utf8(store->text).c_str(),utf8(store->endedText).c_str());
     require(store->text==expected && store->endedText.empty() && store->composing,71);
     press(VK_RETURN);require(store->text==expected&&store->endedText==expected&&!store->composing,72);
   }
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
   if(mode==L"busy-mouse") {
     ComPtr<ITfUIElementMgr> manager;require(SUCCEEDED(thread.As(&manager)),54);ComPtr<IEnumTfUIElements> elements;require(SUCCEEDED(manager->EnumUIElements(&elements)),55);
     ComPtr<ITfUIElement> element;ULONG count=0;require(SUCCEEDED(elements->Next(1,&element,&count))&&count==1,56);
     ComPtr<ITfCandidateListUIElementBehavior> candidates;require(SUCCEEDED(element.As(&candidates)),57);
     require(SUCCEEDED(candidates->SetSelection(1))&&SUCCEEDED(candidates->Finalize()),58);
   } else press(mode==L"busy-pick"?'2':'D');
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
 require(SUCCEEDED(service->Deactivate()),40);thread->SetFocus(nullptr);document->Pop(TF_POPF_ALL);thread->Deactivate();std::fflush(stdout);return 0;
}
