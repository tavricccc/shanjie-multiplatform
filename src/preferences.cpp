#include "preferences.h"
#include <windows.h>
#include <filesystem>
#include <string>
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
    out.initialChinese = GetPrivateProfileIntW(L"input",L"chinese",1,path.c_str()) != 0;
    out.initialFullWidth = GetPrivateProfileIntW(L"input",L"full_width",0,path.c_str()) != 0;
    out.shiftSide = GetPrivateProfileIntW(L"shortcuts",L"shift_side",0,path.c_str());
    out.capsLockEnglish = GetPrivateProfileIntW(L"input",L"caps_lock_english",1,path.c_str()) != 0;
    out.escapeClears = GetPrivateProfileIntW(L"input",L"escape_clears",1,path.c_str()) != 0;
    out.spacePages = GetPrivateProfileIntW(L"candidates",L"space_pages",1,path.c_str()) != 0;
    out.backquoteSymbols = GetPrivateProfileIntW(L"shortcuts",L"backquote_symbols",1,path.c_str()) != 0;
    out.codeInput = GetPrivateProfileIntW(L"shortcuts",L"code_input",1,path.c_str()) != 0;
    out.learning = GetPrivateProfileIntW(L"input",L"learning",0,path.c_str()) != 0;
    out.theme = GetPrivateProfileIntW(L"appearance",L"theme",0,path.c_str());
    out.fontSize = GetPrivateProfileIntW(L"appearance",L"font_size",18,path.c_str());
    return out;
}
