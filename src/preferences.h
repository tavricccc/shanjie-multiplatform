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
};
Preferences loadPreferences();
bool savePreferences(const Preferences&);
