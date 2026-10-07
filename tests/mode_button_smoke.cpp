#include "mode_button.h"
#include "identity.h"
#include <cassert>
#include <cstdio>
HMODULE module = nullptr;
long objects = 0;
int main() {
    bool chinese = true;
    auto button = new ModeButton([&] { return chinese; },[] { return S_OK; },[] {});
    TF_LANGBARITEMINFO item{}; assert(SUCCEEDED(button->GetInfo(&item)));
    assert(item.dwStyle & TF_LBI_STYLE_TEXTCOLORICON);
    for (bool mode : {true,false}) {
        chinese = mode; HICON icon = nullptr; assert(SUCCEEDED(button->GetIcon(&icon)));
        ICONINFO info{}; assert(GetIconInfo(icon,&info));
        assert(!info.hbmColor); BITMAP bitmap{}; assert(GetObjectW(info.hbmMask,sizeof(bitmap),&bitmap));
        assert(bitmap.bmBitsPixel == 1 && bitmap.bmHeight == GetSystemMetrics(SM_CYSMICON)*2);
        DeleteObject(info.hbmMask); DestroyIcon(icon);
    }
    button->Release(); assert(objects == 0);
    std::puts("PASS: Chinese/English TSF theme-recolorable monochrome icons");
}
