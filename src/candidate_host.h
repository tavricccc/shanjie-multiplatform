#pragma once
#include <windows.h>
#include "engine.h"
#include "preferences.h"
// Shared with ui/Interop/CandidateChannel.cs; fixed-width layout works from x86 and x64.
struct CandidatePacket {
    LONG sequence;
    DWORD version, visible, layout;
    LONG selected;
    DWORD count, first, total;
    LONG left, bottom, top;
    DWORD dpi, theme, fontSize, columns, shutdown;
    ULONGLONG callback;
    DWORD parent, reserved;
    wchar_t words[82][128]; // Slot 81 is the optional reading action.
};
static_assert(offsetof(CandidatePacket,words) == 80);
class CandidateHost {
    HANDLE mapping_ = nullptr, event_ = nullptr, process_ = nullptr;
    CandidatePacket* packet_ = nullptr;
    HWND hostWindow_ = nullptr;
public:
    ~CandidateHost();
    bool start(HWND callback);
    void publish(const shanjie::Snapshot&, const Preferences&, const RECT&, UINT dpi, bool visible);
    void hide();
    void ready(HWND window);
    LONG sequence() const { return packet_ ? packet_->sequence : 0; }
    bool shown() const { return packet_ && packet_->visible && process_ && WaitForSingleObject(process_,0) == WAIT_TIMEOUT && hostWindow_ && IsWindowVisible(hostWindow_); }
};
