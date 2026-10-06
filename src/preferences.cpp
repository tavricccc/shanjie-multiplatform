#include "preferences.h"
#include <windows.h>
#include <filesystem>
#include <string>
Preferences loadPreferences() {
    Preferences out;
    DWORD length = GetEnvironmentVariableW(L"LOCALAPPDATA",nullptr,0);
    if (!length) return out;
    std::wstring folder(length,L'\0'); GetEnvironmentVariableW(L"LOCALAPPDATA",folder.data(),length); folder.resize(length-1);
    auto path = std::filesystem::path(folder) / L"Shanjie" / L"profile.ini";
    wchar_t value[32]{};
    GetPrivateProfileStringW(L"profile",L"interaction",L"windows",value,32,path.c_str());
    out.windowsKeys = std::wstring(value) == L"windows";
    GetPrivateProfileStringW(L"candidates",L"layout",L"vertical",value,32,path.c_str());
    out.vertical = std::wstring(value) == L"vertical";
    out.shiftToggle = GetPrivateProfileIntW(L"shortcuts",L"shift_toggle",1,path.c_str()) != 0;
    out.widthToggle = GetPrivateProfileIntW(L"shortcuts",L"shift_space_toggle",1,path.c_str()) != 0;
    out.ctrlPunctuation = GetPrivateProfileIntW(L"shortcuts",L"ctrl_punctuation",1,path.c_str()) != 0;
    GetPrivateProfileStringW(L"engine",L"ranking",L"chat",value,32,path.c_str());
    out.ranking = std::wstring(value) == L"formal" ? 1 : 0;
    return out;
}
