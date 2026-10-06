// Uses the real registered TIP and session profile APIs. No keyboard injection.
#include <windows.h>
#include <msctf.h>
#include <wrl/client.h>
#include <cstdio>
#include <cwchar>
#include <string>
#include "identity.h"
using Microsoft::WRL::ComPtr;
int wmain(int argc, wchar_t** argv) {
    CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED);
    ComPtr<ITfInputProcessorProfiles> profiles;
    HRESULT hr = CoCreateInstance(CLSID_TF_InputProcessorProfiles,nullptr,CLSCTX_INPROC_SERVER,IID_PPV_ARGS(&profiles));
    if (FAILED(hr)) return 1;
    ComPtr<ITfInputProcessorProfileMgr> manager; profiles.As(&manager);
    TF_INPUTPROCESSORPROFILE active{};
    hr = manager->GetActiveProfile(GUID_TFCAT_TIP_KEYBOARD,&active);
    wchar_t id[40], profile[40]; StringFromGUID2(active.clsid,id,40); StringFromGUID2(active.guidProfile,profile,40);
    std::wprintf(L"Active: %04x %s %s (HRESULT %08lx)\n",active.langid,id,profile,(unsigned long)hr);
    if (argc == 2) {
        GUID target = std::wstring(argv[1]) == L"eten" ? EtenProfile : StandardProfile;
        GUID service = ServiceId;
        if (std::wstring(argv[1]) == L"microsoft") {
            CLSIDFromString(L"{D9012138-88BF-46F4-B599-E69D8BA2FED6}",&service);
            CLSIDFromString(L"{2939307A-285F-47F9-B909-8089DF8A7FDD}",&target);
        }
        hr = profiles->ChangeCurrentLanguage(Language);
        if (SUCCEEDED(hr)) hr = manager->ActivateProfile(TF_PROFILETYPE_INPUTPROCESSOR,Language,service,target,nullptr,TF_IPPMF_FORSESSION);
        std::printf("Activate registered Shanjie: %08lx\n",(unsigned long)hr);
    }
    manager.Reset(); profiles.Reset(); CoUninitialize(); return FAILED(hr) ? 1 : 0;
}
