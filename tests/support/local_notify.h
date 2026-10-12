#pragma once
// Notify only this test process. Never refresh the user's live input services.
inline void localNotify(const wchar_t* name) {
    UINT message = RegisterWindowMessageW(name);
    EnumWindows([](HWND window, LPARAM value) -> BOOL {
        DWORD pid = 0; GetWindowThreadProcessId(window, &pid);
        wchar_t cls[96]{}; GetClassNameW(window, cls, 96);
        if (pid == GetCurrentProcessId() && std::wstring(cls) == L"ShanjieRustProfileControlChannel")
            SendMessageW(window, static_cast<UINT>(value), 0, 0);
        return TRUE;
    }, message);
}
