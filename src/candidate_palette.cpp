#include "candidate_palette.h"
#include <winrt/Windows.UI.ViewManagement.h>
#include <winrt/Windows.UI.h>
CandidatePalette candidatePalette(unsigned theme) {
    HIGHCONTRASTW contrast{sizeof(contrast)}; SystemParametersInfoW(SPI_GETHIGHCONTRAST,sizeof(contrast),&contrast,0);
    auto system = [](int role) { auto c = GetSysColor(role); return D2D1::ColorF(GetRValue(c)/255.0f,GetGValue(c)/255.0f,GetBValue(c)/255.0f); };
    if (contrast.dwFlags & HCF_HIGHCONTRASTON) return {system(COLOR_WINDOW),system(COLOR_WINDOWTEXT),system(COLOR_WINDOWTEXT),system(COLOR_HIGHLIGHT),system(COLOR_HIGHLIGHTTEXT)};
    winrt::Windows::UI::ViewManagement::UISettings settings;
    using Role = winrt::Windows::UI::ViewManagement::UIColorType;
    auto foreground = settings.GetColorValue(Role::Foreground);
    bool dark = theme == 2 || (theme == 0 && foreground.R > 127);
    auto color = settings.GetColorValue(dark ? Role::AccentLight2 : Role::AccentDark1);
    return {D2D1::ColorF(dark ? 0x24272B : 0xF5F6F7),D2D1::ColorF(dark ? 0x4A4F55 : 0xD8DADD),D2D1::ColorF(dark ? 0xF5F6F7 : 0x24272B),D2D1::ColorF(color.R/255.0f,color.G/255.0f,color.B/255.0f),D2D1::ColorF(dark ? 0x000000 : 0xFFFFFF)};
}
