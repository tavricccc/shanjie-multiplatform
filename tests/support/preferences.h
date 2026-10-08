#pragma once
#include <string>
enum class CandidateTrigger { Down, Space, Both };
enum class EscapeAction { Staged, Clear, Ignore };
struct Preferences {
    bool windowsKeys = true;
    CandidateTrigger trigger = CandidateTrigger::Down;
    bool rightAnchor = true;
    bool shiftToggle = true;
    bool ctrlSpaceToggle = false;
    bool widthToggle = true;
    bool ctrlPunctuation = true;
    unsigned ranking = 0;
    unsigned model = 4;
    bool prediction = false;
    bool demote = true;
    bool useContext = true;
    bool initialChinese = true;
    bool initialFullWidth = false;
    unsigned shiftSide = 0; // 0: both, 1: left, 2: right.
    bool capsLockEnglish = false;
    EscapeAction escape = EscapeAction::Staged;
    bool spaceAfterCommit = false;
    bool pinyinSpaceConverts = false;
    bool spacePages = true;
    bool backquoteSymbols = false;
    bool codeInput = true;
    bool learning = true;
    bool associations = false;
    unsigned keyboard = 0; // Profile / Standard / Eten / Hanyu / Taiwan.
    bool alphanumeric = true;
    bool big5 = false;
    bool enterCommits = true;
    bool upCycles = true;
    bool backspaceDeletes = false;
    bool wrapPages = true;
    bool symbolKeyboard = true;
    bool shiftPunctuation = false;
    bool plainPunctuation = false;
    bool standaloneTones = false;
    bool ctrlBackslash = false;
    bool forgetCandidate = false;
    bool modeIndicator = true;
    std::wstring dictionary = L"dictionary.tsv";
    unsigned theme = 0; // System / Light / Dark.
    unsigned fontSize = 16;
};
Preferences loadPreferences();
unsigned long long preferencesStamp();
