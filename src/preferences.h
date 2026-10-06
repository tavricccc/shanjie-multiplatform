#pragma once
enum class CandidateLayout { Vertical, Horizontal, Grid };
enum class CandidateTrigger { Down, Space, Both };
struct Preferences {
    bool windowsKeys = true;
    CandidateLayout layout = CandidateLayout::Vertical;
    CandidateTrigger trigger = CandidateTrigger::Down;
    bool rightAnchor = true;
    bool shiftToggle = true;
    bool ctrlSpaceToggle = true;
    bool widthToggle = true;
    bool ctrlPunctuation = true;
    unsigned ranking = 0;
    bool initialChinese = true;
    bool initialFullWidth = false;
    unsigned shiftSide = 0; // 0: both, 1: left, 2: right.
    bool capsLockEnglish = true;
    bool escapeClears = true;
    bool spacePages = true;
    bool backquoteSymbols = true;
    bool codeInput = true;
    bool learning = false;
    unsigned theme = 0; // System / Light / Dark.
    unsigned fontSize = 18;
};
Preferences loadPreferences();
