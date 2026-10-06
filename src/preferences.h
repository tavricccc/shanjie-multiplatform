#pragma once
struct Preferences {
    bool windowsKeys = true;
    bool vertical = true;
    bool shiftToggle = true;
    bool widthToggle = true;
    bool ctrlPunctuation = true;
    unsigned ranking = 0;
};
Preferences loadPreferences();
