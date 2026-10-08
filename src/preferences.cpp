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
unsigned long long preferencesStamp() {
    WIN32_FILE_ATTRIBUTE_DATA data{};
    auto path = profilePath();
    if (!GetFileAttributesExW(path.c_str(),GetFileExInfoStandard,&data)) return 0;
    return ((unsigned long long)data.ftLastWriteTime.dwHighDateTime << 32) | data.ftLastWriteTime.dwLowDateTime;
}
Preferences loadPreferences() {
    Preferences out;
    auto path = profilePath(); if (path.empty()) return out;
    bool legacy = std::filesystem::exists(path) && GetPrivateProfileIntW(L"profile",L"schema",1,path.c_str()) < 3;
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
    out.ctrlSpaceToggle = GetPrivateProfileIntW(L"shortcuts",L"ctrl_space_toggle",legacy ? 1 : 0,path.c_str()) != 0;
    out.widthToggle = GetPrivateProfileIntW(L"shortcuts",L"shift_space_toggle",1,path.c_str()) != 0;
    out.ctrlPunctuation = GetPrivateProfileIntW(L"shortcuts",L"ctrl_punctuation",1,path.c_str()) != 0;
    GetPrivateProfileStringW(L"engine",L"ranking",L"chat",value,32,path.c_str());
    out.ranking = std::wstring(value) == L"formal" ? 1 : 0;
    out.model = 4;
    out.prediction = GetPrivateProfileIntW(L"engine",L"prediction",!legacy && !out.windowsKeys,path.c_str()) != 0;
    out.demote = GetPrivateProfileIntW(L"engine",L"demote",1,path.c_str()) != 0;
    out.useContext = GetPrivateProfileIntW(L"input",L"context",1,path.c_str()) != 0;
    out.initialChinese = GetPrivateProfileIntW(L"input",L"chinese",1,path.c_str()) != 0;
    out.initialFullWidth = GetPrivateProfileIntW(L"input",L"full_width",0,path.c_str()) != 0;
    out.shiftSide = GetPrivateProfileIntW(L"shortcuts",L"shift_side",0,path.c_str());
    out.capsLockEnglish = GetPrivateProfileIntW(L"input",L"caps_lock_english",legacy && out.windowsKeys,path.c_str()) != 0;
    auto oldEscape = GetPrivateProfileIntW(L"input",L"escape_clears",1,path.c_str()) ? L"staged" : L"ignore";
    GetPrivateProfileStringW(L"input",L"escape",oldEscape,value,32,path.c_str());
    out.escape = std::wstring(value) == L"clear" ? EscapeAction::Clear : std::wstring(value) == L"ignore" ? EscapeAction::Ignore : EscapeAction::Staged;
    out.spaceAfterCommit = GetPrivateProfileIntW(L"input",L"space_after_commit",legacy || !out.windowsKeys,path.c_str()) != 0;
    out.pinyinSpaceConverts = GetPrivateProfileIntW(L"input",L"pinyin_space_converts",0,path.c_str()) != 0;
    out.spacePages = GetPrivateProfileIntW(L"candidates",L"space_pages",1,path.c_str()) != 0;
    out.backquoteSymbols = GetPrivateProfileIntW(L"shortcuts",L"backquote_symbols",legacy && out.windowsKeys,path.c_str()) != 0;
    out.codeInput = GetPrivateProfileIntW(L"shortcuts",L"code_input",out.windowsKeys ? 1 : 0,path.c_str()) != 0;
    out.learning = GetPrivateProfileIntW(L"input",L"learning",!legacy && out.windowsKeys,path.c_str()) != 0;
    out.associations = GetPrivateProfileIntW(L"input",L"associations",legacy && out.windowsKeys,path.c_str()) != 0;
    out.keyboard = GetPrivateProfileIntW(L"input",L"keyboard",0,path.c_str());
    out.alphanumeric = GetPrivateProfileIntW(L"input",L"alphanumeric",out.windowsKeys ? 1 : 0,path.c_str()) != 0;
    out.big5 = GetPrivateProfileIntW(L"candidates",L"big5",0,path.c_str()) != 0;
    out.enterCommits = GetPrivateProfileIntW(L"candidates",L"enter_commits",out.windowsKeys,path.c_str()) != 0;
    out.upCycles = GetPrivateProfileIntW(L"candidates",L"up_cycles",out.windowsKeys,path.c_str()) != 0;
    out.tabExpands = GetPrivateProfileIntW(L"candidates",L"tab_expands",out.windowsKeys,path.c_str()) != 0;
    out.rightExpands = GetPrivateProfileIntW(L"candidates",L"right_expands",out.windowsKeys,path.c_str()) != 0;
    out.gridUpCollapses = GetPrivateProfileIntW(L"candidates",L"grid_up_collapses",!out.windowsKeys,path.c_str()) != 0;
    out.backspaceDeletes = GetPrivateProfileIntW(L"candidates",L"backspace_deletes",0,path.c_str()) != 0;
    out.wrapPages = GetPrivateProfileIntW(L"candidates",L"wrap_pages",1,path.c_str()) != 0;
    out.symbolKeyboard = GetPrivateProfileIntW(L"shortcuts",L"symbol_keyboard",legacy || out.windowsKeys,path.c_str()) != 0;
    out.shiftPunctuation = GetPrivateProfileIntW(L"input",L"shift_punctuation",legacy || !out.windowsKeys,path.c_str()) != 0;
    out.plainPunctuation = GetPrivateProfileIntW(L"input",L"plain_punctuation",legacy || !out.windowsKeys,path.c_str()) != 0;
    out.standaloneTones = GetPrivateProfileIntW(L"input",L"standalone_tones",legacy || !out.windowsKeys,path.c_str()) != 0;
    out.ctrlBackslash = GetPrivateProfileIntW(L"shortcuts",L"ctrl_backslash",!out.windowsKeys,path.c_str()) != 0;
    out.forgetCandidate = GetPrivateProfileIntW(L"shortcuts",L"forget_candidate",0,path.c_str()) != 0;
    out.modeIndicator = GetPrivateProfileIntW(L"appearance",L"mode_indicator",1,path.c_str()) != 0;
    wchar_t dictionary[128]{};
    GetPrivateProfileStringW(L"input",L"dictionary",L"dictionary.tsv",dictionary,128,path.c_str());
    out.dictionary = dictionary;
    out.theme = GetPrivateProfileIntW(L"appearance",L"theme",0,path.c_str());
    out.fontSize = GetPrivateProfileIntW(L"appearance",L"font_size",18,path.c_str());
    return out;
}
