#include <windows.h>
#include <vector>
#include "preferences.h"
#pragma comment(linker, "/manifestdependency:\"type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'\"")
namespace {
enum { Preset = 100, Interaction, Trigger, Anchor, Layout, Ranking, ShiftToggle, CtrlSpaceToggle, WidthToggle, Punctuation, Save, Cancel };
struct Control { HWND window; int x,y,width,height; };
std::vector<Control> controls;
HFONT font = nullptr;
HWND create(HWND parent, const wchar_t* type, const wchar_t* text, DWORD style, int id, int x,int y,int width,int height) {
    auto window = CreateWindowW(type,text,WS_CHILD | WS_VISIBLE | style,x,y,width,height,parent,(HMENU)(INT_PTR)id,GetModuleHandleW(nullptr),nullptr);
    controls.push_back({window,x,y,width,height}); return window;
}
void combo(HWND parent, const wchar_t* label, int id, int y, std::initializer_list<const wchar_t*> values) {
    create(parent,L"STATIC",label,0,0,24,y+5,170,26);
    auto window = create(parent,L"COMBOBOX",L"",CBS_DROPDOWNLIST | WS_TABSTOP | WS_VSCROLL,id,208,y,344,220);
    for (auto value : values) SendMessageW(window,CB_ADDSTRING,0,(LPARAM)value);
}
int selected(HWND parent,int id) { return (int)SendDlgItemMessageW(parent,id,CB_GETCURSEL,0,0); }
bool checked(HWND parent,int id) { return SendDlgItemMessageW(parent,id,BM_GETCHECK,0,0) == BST_CHECKED; }
void fill(HWND window, const Preferences& p) {
    SendDlgItemMessageW(window,Interaction,CB_SETCURSEL,p.windowsKeys ? 0 : 1,0);
    SendDlgItemMessageW(window,Trigger,CB_SETCURSEL,(int)p.trigger,0);
    SendDlgItemMessageW(window,Anchor,CB_SETCURSEL,p.rightAnchor ? 0 : 1,0);
    SendDlgItemMessageW(window,Layout,CB_SETCURSEL,(int)p.layout,0);
    SendDlgItemMessageW(window,Ranking,CB_SETCURSEL,p.ranking,0);
    SendDlgItemMessageW(window,ShiftToggle,BM_SETCHECK,p.shiftToggle,0);
    SendDlgItemMessageW(window,CtrlSpaceToggle,BM_SETCHECK,p.ctrlSpaceToggle,0);
    SendDlgItemMessageW(window,WidthToggle,BM_SETCHECK,p.widthToggle,0);
    SendDlgItemMessageW(window,Punctuation,BM_SETCHECK,p.ctrlPunctuation,0);
}
void scale(HWND window) {
    int dpi = (int)GetDpiForWindow(window);
    HFONT next = CreateFontW(-MulDiv(14,dpi,96),0,0,0,FW_NORMAL,FALSE,FALSE,FALSE,DEFAULT_CHARSET,0,0,CLEARTYPE_QUALITY,0,L"Microsoft JhengHei UI");
    for (auto& control : controls) {
        SetWindowPos(control.window,nullptr,MulDiv(control.x,dpi,96),MulDiv(control.y,dpi,96),MulDiv(control.width,dpi,96),MulDiv(control.height,dpi,96),SWP_NOZORDER | SWP_NOACTIVATE);
        SendMessageW(control.window,WM_SETFONT,(WPARAM)next,TRUE);
    }
    if (font) DeleteObject(font); font = next;
}
LRESULT CALLBACK procedure(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    if (message == WM_CREATE) {
        create(window,L"STATIC",L"調整操作方式，或載入一組預設再修改。",0,0,24,20,528,28);
        combo(window,L"載入預設",Preset,56,{L"目前設定",L"Windows 微軟注音",L"善解上游"});
        combo(window,L"方向鍵操作",Interaction,100,{L"Windows 微軟注音",L"善解上游"});
        combo(window,L"開啟選字",Trigger,140,{L"↓ 下鍵",L"空白鍵",L"↓ 或空白鍵"});
        combo(window,L"選字位置",Anchor,180,{L"游標右側的字／詞",L"游標左側的字／詞"});
        combo(window,L"候選排列",Layout,220,{L"直列（上下排列）",L"橫列（左右排列）",L"格狀（每列 9 個）"});
        combo(window,L"詞句偏好",Ranking,260,{L"聊天（Chat）",L"正式（Formal）"});
        create(window,L"BUTTON",L"單按 Shift 切換中英",BS_AUTOCHECKBOX | WS_TABSTOP,ShiftToggle,24,310,528,26);
        create(window,L"BUTTON",L"Ctrl＋空白切換中英",BS_AUTOCHECKBOX | WS_TABSTOP,CtrlSpaceToggle,24,344,528,26);
        create(window,L"BUTTON",L"Shift＋空白切換全半形",BS_AUTOCHECKBOX | WS_TABSTOP,WidthToggle,24,378,528,26);
        create(window,L"BUTTON",L"Ctrl＋標點輸入全形標點",BS_AUTOCHECKBOX | WS_TABSTOP,Punctuation,24,412,528,26);
        create(window,L"STATIC",L"儲存後請切換到其他輸入法再切回善解。\n仍使用舊設定的程式，請重新開啟。",0,0,24,458,528,46);
        create(window,L"BUTTON",L"儲存",BS_DEFPUSHBUTTON | WS_TABSTOP,Save,356,520,92,32);
        create(window,L"BUTTON",L"取消",BS_PUSHBUTTON | WS_TABSTOP,Cancel,460,520,92,32);
        SendDlgItemMessageW(window,Preset,CB_SETCURSEL,0,0); fill(window,loadPreferences()); scale(window); return 0;
    }
    if (message == WM_COMMAND) {
        if (LOWORD(wp) == Preset && HIWORD(wp) == CBN_SELCHANGE) {
            int preset = selected(window,Preset);
            if (!preset) { fill(window,loadPreferences()); return 0; }
            Preferences p;
            if (preset == 2) { p.windowsKeys = p.rightAnchor = p.shiftToggle = p.ctrlSpaceToggle = p.widthToggle = p.ctrlPunctuation = false; p.layout = CandidateLayout::Horizontal; p.trigger = CandidateTrigger::Both; }
            fill(window,p); return 0;
        }
        if (LOWORD(wp) == Save) {
            Preferences p; p.windowsKeys = selected(window,Interaction) == 0;
            p.trigger = (CandidateTrigger)selected(window,Trigger); p.rightAnchor = selected(window,Anchor) == 0;
            p.layout = (CandidateLayout)selected(window,Layout); p.ranking = selected(window,Ranking);
            p.shiftToggle = checked(window,ShiftToggle); p.ctrlSpaceToggle = checked(window,CtrlSpaceToggle);
            p.widthToggle = checked(window,WidthToggle); p.ctrlPunctuation = checked(window,Punctuation);
            if (!savePreferences(p)) MessageBoxW(window,L"無法儲存個人設定。",L"善解設定",MB_OK | MB_ICONERROR);
            else DestroyWindow(window);
            return 0;
        }
        if (LOWORD(wp) == Cancel) { DestroyWindow(window); return 0; }
    }
    if (message == WM_DPICHANGED) { auto rect = (RECT*)lp; SetWindowPos(window,nullptr,rect->left,rect->top,rect->right-rect->left,rect->bottom-rect->top,SWP_NOZORDER | SWP_NOACTIVATE); scale(window); return 0; }
    if (message == WM_DESTROY) { if (font) DeleteObject(font); PostQuitMessage(0); return 0; }
    return DefWindowProcW(window,message,wp,lp);
}
}
int WINAPI wWinMain(HINSTANCE instance,HINSTANCE,PWSTR,int show) {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    WNDCLASSW wc{}; wc.lpfnWndProc = procedure; wc.hInstance = instance; wc.lpszClassName = L"ShanjieSettings"; wc.hbrBackground = GetSysColorBrush(COLOR_BTNFACE); wc.hCursor = LoadCursor(nullptr,IDC_ARROW); RegisterClassW(&wc);
    int dpi = (int)GetDpiForSystem(); RECT rect{0,0,MulDiv(576,dpi,96),MulDiv(576,dpi,96)};
    DWORD style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX; AdjustWindowRectExForDpi(&rect,style,FALSE,0,dpi);
    HWND window = CreateWindowW(wc.lpszClassName,L"善解設定",style,CW_USEDEFAULT,CW_USEDEFAULT,rect.right-rect.left,rect.bottom-rect.top,nullptr,nullptr,instance,nullptr);
    if (!window) return 1; ShowWindow(window,show);
    MSG message{}; while (GetMessageW(&message,nullptr,0,0)>0) if (!IsDialogMessageW(window,&message)) { TranslateMessage(&message); DispatchMessageW(&message); }
    return 0;
}
