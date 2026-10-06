#include "preferences.h"
#include <windows.h>
#include <filesystem>
#include <string>
#include <fstream>
namespace {
std::filesystem::path profilePath() {
    DWORD length = GetEnvironmentVariableW(L"LOCALAPPDATA",nullptr,0);
    if (!length) return {};
    std::wstring folder(length,L'\0'); GetEnvironmentVariableW(L"LOCALAPPDATA",folder.data(),length); folder.resize(length-1);
    return std::filesystem::path(folder) / L"Shanjie" / L"profile.ini";
}
}
Preferences loadPreferences() {
    Preferences out;
    auto path = profilePath(); if (path.empty()) return out;
    wchar_t value[32]{};
    GetPrivateProfileStringW(L"profile",L"interaction",L"windows",value,32,path.c_str());
    out.windowsKeys = std::wstring(value) == L"windows";
    GetPrivateProfileStringW(L"candidates",L"layout",L"vertical",value,32,path.c_str());
    out.layout = std::wstring(value) == L"grid" ? CandidateLayout::Grid : std::wstring(value) == L"horizontal" ? CandidateLayout::Horizontal : CandidateLayout::Vertical;
    GetPrivateProfileStringW(L"candidates",L"open",L"down",value,32,path.c_str());
    out.trigger = std::wstring(value) == L"space" ? CandidateTrigger::Space : std::wstring(value) == L"both" ? CandidateTrigger::Both : CandidateTrigger::Down;
    GetPrivateProfileStringW(L"candidates",L"anchor",L"right",value,32,path.c_str());
    out.rightAnchor = std::wstring(value) == L"right";
    out.shiftToggle = GetPrivateProfileIntW(L"shortcuts",L"shift_toggle",1,path.c_str()) != 0;
    out.ctrlSpaceToggle = GetPrivateProfileIntW(L"shortcuts",L"ctrl_space_toggle",1,path.c_str()) != 0;
    out.widthToggle = GetPrivateProfileIntW(L"shortcuts",L"shift_space_toggle",1,path.c_str()) != 0;
    out.ctrlPunctuation = GetPrivateProfileIntW(L"shortcuts",L"ctrl_punctuation",1,path.c_str()) != 0;
    GetPrivateProfileStringW(L"engine",L"ranking",L"chat",value,32,path.c_str());
    out.ranking = std::wstring(value) == L"formal" ? 1 : 0;
    return out;
}
bool savePreferences(const Preferences& value) {
    auto path = profilePath(); if (path.empty()) return false;
    std::error_code error; std::filesystem::create_directories(path.parent_path(),error);
    if (error) return false;
    auto temporary = path; temporary += L".tmp";
    std::ofstream file(temporary,std::ios::binary | std::ios::trunc);
    file << "[profile]\r\nschema=1\r\ninteraction=" << (value.windowsKeys ? "windows" : "shanjie")
        << "\r\n\r\n[engine]\r\nranking=" << (value.ranking ? "formal" : "chat")
        << "\r\n\r\n[candidates]\r\nlayout=" << (value.layout == CandidateLayout::Grid ? "grid" : value.layout == CandidateLayout::Horizontal ? "horizontal" : "vertical")
        << "\r\nanchor=" << (value.rightAnchor ? "right" : "left")
        << "\r\nopen=" << (value.trigger == CandidateTrigger::Both ? "both" : value.trigger == CandidateTrigger::Space ? "space" : "down")
        << "\r\n\r\n[shortcuts]\r\nshift_toggle=" << value.shiftToggle
        << "\r\nctrl_space_toggle=" << value.ctrlSpaceToggle << "\r\nshift_space_toggle=" << value.widthToggle << "\r\nctrl_punctuation=" << value.ctrlPunctuation << "\r\n";
    file.close(); if (!file) return false;
    return MoveFileExW(temporary.c_str(),path.c_str(),MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH) != FALSE;
}
