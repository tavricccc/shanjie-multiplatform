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
#include <cstdlib>
#include "shanjie.h"
void require(bool condition,int check) { if(!condition) { std::printf("PROBE_SETUP_FAIL=%d\n",check); std::fflush(stdout); ExitProcess(check); } }
std::string utf8(const std::wstring& value) { if(value.empty()) return {}; int count=WideCharToMultiByte(CP_UTF8,0,value.data(),(int)value.size(),nullptr,0,nullptr,nullptr); std::string out(count,'\0'); WideCharToMultiByte(CP_UTF8,0,value.data(),(int)value.size(),out.data(),count,nullptr,nullptr); return out; }
struct KeyboardRestore { BYTE previous[256]{}; KeyboardRestore(){require(GetKeyboardState(previous),101);} ~KeyboardRestore(){SetKeyboardState(previous);} };
void keyboardState(bool caps,bool shift) { BYTE state[256]{}; state[VK_CAPITAL]=caps?1:0; state[VK_SHIFT]=shift?0x80:0; state[VK_LSHIFT]=shift?0x80:0; require(SetKeyboardState(state),102); require(((GetKeyState(VK_CAPITAL)&1)!=0)==caps,103); }
int wmain(int argc,wchar_t** argv) {
 require(argc==7 || argc==8,1); bool caps=std::wcstol(argv[2],nullptr,10)!=0, shift=std::wcstol(argv[3],nullptr,10)!=0, capsEnglish=std::wcstol(argv[4],nullptr,10)!=0, flip=std::wcstol(argv[5],nullptr,10)!=0, prefix=std::wcstol(argv[6],nullptr,10)!=0;
 KeyboardRestore restore; require(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)),2);
 HMODULE dll=LoadLibraryW(argv[1]); require(dll!=nullptr,3);
 auto factoryFn=(HRESULT (WINAPI*)(REFCLSID,REFIID,void**))GetProcAddress(dll,"DllGetClassObject"); require(factoryFn!=nullptr,4);
 auto translator=(bool (__cdecl*)(size_t,intptr_t,ShanjieKey*))GetProcAddress(dll,"sj_translate"); require(translator!=nullptr,5);
 auto resident=(unsigned long long (__cdecl*)())GetProcAddress(dll,"sj_resident_status"); require(resident!=nullptr,6);
 ComPtr<IClassFactory> factory; require(SUCCEEDED(factoryFn(ServiceId,IID_PPV_ARGS(&factory))),7);
 ComPtr<ITfTextInputProcessorEx> service; require(SUCCEEDED(factory->CreateInstance(nullptr,IID_PPV_ARGS(&service))),8);
 ComPtr<ITfThreadMgr> thread; require(SUCCEEDED(CoCreateInstance(CLSID_TF_ThreadMgr,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&thread))),9);
 TfClientId client; require(SUCCEEDED(thread->Activate(&client)),10);
 ComPtr<ITfDocumentMgr> document; require(SUCCEEDED(thread->CreateDocumentMgr(&document)),11);
 ComPtr<Store> store; store.Attach(new Store); ComPtr<ITfContext> context; TfEditCookie cookie;
 require(SUCCEEDED(document->CreateContext(client,0,store.Get(),&context,&cookie)),12);
 require(SUCCEEDED(document->Push(context.Get())),13); require(SUCCEEDED(thread->SetFocus(document.Get())),14);
 ComPtr<ITfClientId> identifiers; TfClientId serviceClient=TF_CLIENTID_NULL;
 require(SUCCEEDED(thread.As(&identifiers))&&SUCCEEDED(identifiers->GetClientId(ServiceId,&serviceClient)),15);
 ComPtr<ThreadProxy> proxy; proxy.Attach(new ThreadProxy(thread.Get()));
 require(SUCCEEDED(service->ActivateEx(proxy.Get(),serviceClient,TF_TMAE_UIELEMENTENABLEDONLY)),16);
 auto start=std::chrono::steady_clock::now(); while(argc==7 && (resident()>>32)!=(resident()&0xffffffffULL)) { require(std::chrono::steady_clock::now()-start<std::chrono::seconds(15),17); Sleep(5); }
 ComPtr<ITfKeyEventSink> keys; require(SUCCEEDED(service.As(&keys)),18);
 ComPtr<ITfCompartmentMgr> compartments; ComPtr<ITfCompartment> chinese; VARIANT mode; VariantInit(&mode);
 require(SUCCEEDED(thread.As(&compartments)),19); require(SUCCEEDED(compartments->GetCompartment(GUID_COMPARTMENT_KEYBOARD_OPENCLOSE,&chinese)),20); require(SUCCEEDED(chinese->GetValue(&mode))&&mode.vt==VT_I4&&mode.lVal!=0,21); VariantClear(&mode);
 const wchar_t* prefixKeys=std::getenv("SHANJIE_TEST_ETEN")?L"NE3":L"SU3";
 double firstKeyMs=-1;
 auto press=[&](WPARAM vk) { auto begin=std::chrono::steady_clock::now(); BOOL tested=FALSE,eaten=FALSE; LPARAM flags=(LPARAM(MapVirtualKeyW((UINT)vk,MAPVK_VK_TO_VSC))<<16)|1; require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),vk,flags,&tested))&&tested,22); require(SUCCEEDED(keys->OnKeyDown(context.Get(),vk,flags,&eaten))&&eaten,23); if(firstKeyMs<0) firstKeyMs=std::chrono::duration<double,std::milli>(std::chrono::steady_clock::now()-begin).count(); };
 if(argc==8 && std::wstring(argv[7])==L"fields") {
   keyboardState(true,false); for(wchar_t vk:std::wstring(L"ENGLISH")) press(vk);
   require(SUCCEEDED(keys->OnSetFocus(FALSE)),50); require(store->text==L"ENGLISH",51);
   ComPtr<ITfDocumentMgr> secondDoc; ComPtr<Store> secondStore; secondStore.Attach(new Store);
   ComPtr<ITfContext> secondContext; TfEditCookie secondCookie;
   require(SUCCEEDED(thread->CreateDocumentMgr(&secondDoc)),52);
   require(SUCCEEDED(secondDoc->CreateContext(client,0,secondStore.Get(),&secondContext,&secondCookie)),53);
   require(SUCCEEDED(secondDoc->Push(secondContext.Get()))&&SUCCEEDED(thread->SetFocus(secondDoc.Get())),54);
   auto firstContext=context; context=secondContext; keyboardState(false,false);
   for(wchar_t vk:std::wstring(L"ENGLISH")) press(vk);
   require(SUCCEEDED(keys->OnSetFocus(FALSE)),55);
   require(secondStore->text==L"english"&&store->text==L"ENGLISH",56);
   require(SUCCEEDED(thread->SetFocus(document.Get())),57); context=firstContext;
   for(wchar_t vk:std::wstring(prefixKeys)) press(vk); press(VK_RETURN);
   require(store->text==L"ENGLISH你"&&secondStore->text==L"english",58);
   std::puts("RESULT {\"field_switch\":true,\"matches\":true}");
   require(SUCCEEDED(service->Deactivate()),59); return 0;
 }
 if(argc==8 && std::wstring(argv[7])==L"punctuation") {
   keyboardState(false,false); if(prefix) { for(wchar_t vk:std::wstring(prefixKeys)) press(vk); }
   BYTE state[256]{}; state[VK_CONTROL]=0x80; state[VK_LCONTROL]=0x80; require(SetKeyboardState(state),40);
   press(VK_OEM_7); require(store->text==(prefix?L"你、":L"、"),41);
   ComPtr<ITfContextComposition> compositions; require(SUCCEEDED(context.As(&compositions)),42);
   ComPtr<IEnumITfCompositionView> views; require(SUCCEEDED(compositions->EnumCompositions(&views)),43);
   ComPtr<ITfCompositionView> view; ULONG count=0; require(SUCCEEDED(views->Next(1,&view,&count))&&count==1,44);
   keyboardState(false,false); press(VK_RETURN); require(store->text==(prefix?L"你、":L"、"),45);
   std::puts("RESULT {\"punctuation\":true,\"composition_preserved\":true,\"matches\":true}");
   require(SUCCEEDED(service->Deactivate()),46); return 0;
 }
 if(argc==8 && std::wstring(argv[7])==L"chinese") {
   keyboardState(true,false); for(wchar_t vk:std::wstring(prefixKeys)) press(vk);
   require(store->text==L"你",47); keyboardState(false,false); press(VK_RETURN); require(store->text==L"你",48);
   std::puts("RESULT {\"caps_chinese\":true,\"matches\":true}");
   require(SUCCEEDED(service->Deactivate()),49); return 0;
 }
 if(prefix) { keyboardState(false,false); for(wchar_t vk:std::wstring(prefixKeys)) press(vk); if(store->text!=L"你") std::printf("PREFIX=%s\n",utf8(store->text).c_str()); require(store->text==L"你",24); }
 std::wstring expected=prefix?L"你":L""; std::wstring word=L"ENGLISH"; unsigned firstModifiers=0, firstBase=0; wchar_t nativeFirst=0;
 for(size_t index=0;index<word.size();++index) {
   bool currentCaps=(flip&&index>=3)?!caps:caps; keyboardState(currentCaps,shift);
   BYTE state[256]{}; WCHAR text[8]{}; require(GetKeyboardState(state),25);
   UINT scan=MapVirtualKeyW((UINT)word[index],MAPVK_VK_TO_VSC); int count=ToUnicodeEx((UINT)word[index],scan,state,text,8,4,GetKeyboardLayout(0)); require(count==1,26); expected.push_back(text[0]);
   ShanjieKey translated{}; require(translator(word[index],(intptr_t(scan)<<16)|1,&translated),27);
   if(index==0) { firstModifiers=translated.modifiers; firstBase=translated.ch; nativeFirst=text[0]; }
   press(word[index]);
 }
 std::wstring actual=store->text; keyboardState(false,false); BOOL wantsEnter=FALSE; require(SUCCEEDED(keys->OnTestKeyDown(context.Get(),VK_RETURN,0,&wantsEnter)),28);
 if(wantsEnter) { BOOL eaten=FALSE; require(SUCCEEDED(keys->OnKeyDown(context.Get(),VK_RETURN,0,&eaten))&&eaten,29); require(store->text==actual,30); }
 std::printf("RESULT {\"caps_lock_english\":%d,\"caps\":%d,\"shift\":%d,\"flip_after_3\":%d,\"chinese_prefix\":%d,\"native_first\":\"%c\",\"bridge_first\":\"%c\",\"bridge_modifiers\":%u,\"actual\":\"%s\",\"expected\":\"%s\",\"matches\":%s,\"all_keys_handled\":true,\"first_key_ms\":%.3f}\n",capsEnglish,caps,shift,flip,prefix,(char)nativeFirst,(char)firstBase,firstModifiers,utf8(actual).c_str(),utf8(expected).c_str(),actual==expected?"true":"false",firstKeyMs); std::fflush(stdout);
 require(SUCCEEDED(service->Deactivate()),31); keys.Reset(); service.Reset(); chinese.Reset(); compartments.Reset(); thread->SetFocus(nullptr); document->Pop(TF_POPF_ALL); context.Reset(); document.Reset(); store.Reset(); proxy.Reset(); identifiers.Reset(); thread->Deactivate(); thread.Reset(); factory.Reset(); CoUninitialize(); return 0;
}
