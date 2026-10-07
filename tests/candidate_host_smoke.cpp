#include "candidate_host.h"
#include "identity.h"
#include <dwmapi.h>
#include <cassert>
#include <cstdio>
#include <psapi.h>
#include <algorithm>
#include <vector>
HINSTANCE module = nullptr;
long objects = 0;
CandidateHost* host = nullptr;
HWND surface = nullptr;
unsigned acknowledgements = 0;
double clockMs() { LARGE_INTEGER value{}, frequency{}; QueryPerformanceCounter(&value); QueryPerformanceFrequency(&frequency); return value.QuadPart * 1000.0 / frequency.QuadPart; }
LRESULT CALLBACK receive(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    if (message == WM_APP + 43) { surface = (HWND)wp; host->ready(surface); ++acknowledgements; return 0; }
    return DefWindowProcW(window,message,wp,lp);
}
template<class Predicate> bool wait(Predicate predicate) {
    auto deadline = GetTickCount64() + 10000;
    while (GetTickCount64() < deadline) {
        MSG message; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) { TranslateMessage(&message); DispatchMessageW(&message); }
        if (predicate()) return true;
        MsgWaitForMultipleObjects(0,nullptr,FALSE,1,QS_ALLINPUT);
    }
    return false;
}
int wmain(int argc,wchar_t** argv) {
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    assert(argc == 2 || argc == 3); bool benchmark = argc == 3; module = LoadLibraryW(argv[1]); assert(module);
    SetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",L"1");
    WNDCLASSW wc{}; wc.lpfnWndProc = receive; wc.hInstance = GetModuleHandleW(nullptr); wc.lpszClassName = L"ShanjieChannelSmoke"; RegisterClassW(&wc);
    auto callback = CreateWindowW(wc.lpszClassName,L"",0,0,0,0,0,HWND_MESSAGE,nullptr,wc.hInstance,nullptr);
    HANDLE child = nullptr;
    {
        CandidateHost presenter; host = &presenter; double coldStart = clockMs(); assert(presenter.start(callback));
        shanjie::Snapshot frame; frame.candidates = {L"你",L"妳",L"擬"}; frame.selected = 1; frame.total = 3;
        Preferences preferences; RECT caret{-12000,-12040,-11999,-12000};
        presenter.hide();
        assert(wait([&] { return surface != nullptr; }));
        assert(!IsWindowVisible(surface));
        double preloadMs = clockMs() - coldStart;
        coldStart = clockMs();
        presenter.publish(frame,preferences,caret,96,true);
        assert(wait([&] { return surface && presenter.shown(); }));
        assert(wait([&] { return IsWindowVisible(surface); }));
        double coldMs = clockMs() - coldStart;
        assert(GetWindowLongPtrW(surface,GWL_EXSTYLE) & WS_EX_NOACTIVATE);
        assert(SendMessageW(surface,WM_MOUSEACTIVATE,0,0) == MA_NOACTIVATE);
        assert(GetForegroundWindow() != surface);
        bool offscreen = wait([&] { RECT bounds{}; GetWindowRect(surface,&bounds); return bounds.right < -10000 && bounds.bottom < -10000; });
        if (!offscreen) { RECT bounds{}; GetWindowRect(surface,&bounds); std::fprintf(stderr,"UI bounds: %ld,%ld,%ld,%ld\n",bounds.left,bounds.top,bounds.right,bounds.bottom); }
        assert(offscreen);
        BOOL nonClientRendering = TRUE;
        assert(SUCCEEDED(DwmGetWindowAttribute(surface,DWMWA_NCRENDERING_ENABLED,&nonClientRendering,sizeof(nonClientRendering))) && !nonClientRendering);
        RECT client{}, bounds{}; GetClientRect(surface,&client); GetWindowRect(surface,&bounds);
        if (client.right != bounds.right - bounds.left || client.bottom != bounds.bottom - bounds.top)
            std::fprintf(stderr,"Client %ldx%ld, window %ldx%ld, style %lx\n",client.right,client.bottom,bounds.right-bounds.left,bounds.bottom-bounds.top,GetWindowLongW(surface,GWL_STYLE));
        assert(client.right == bounds.right - bounds.left && client.bottom == bounds.bottom - bounds.top);
        DWORD process = 0; GetWindowThreadProcessId(surface,&process); child = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_INFORMATION | PROCESS_VM_READ,FALSE,process); assert(child);
        preferences.layout = CandidateLayout::Horizontal; presenter.publish(frame,preferences,caret,144,true);
        presenter.hide(); assert(wait([&] { return !IsWindowVisible(surface); }));
        presenter.publish(frame,preferences,caret,96,true); assert(wait([&] { return IsWindowVisible(surface); }));
        if (benchmark) {
            std::vector<double> samples;
            for (int i = 0; i < 30; ++i) {
                presenter.hide(); assert(wait([&] { return !IsWindowVisible(surface); }));
                auto previous = acknowledgements; double start = clockMs();
                presenter.publish(frame,preferences,caret,96,true);
                assert(wait([&] { return acknowledgements > previous && IsWindowVisible(surface); }));
                samples.push_back(clockMs() - start);
            }
            std::sort(samples.begin(),samples.end());
            PROCESS_MEMORY_COUNTERS_EX memory{}; memory.cb = sizeof(memory);
            assert(GetProcessMemoryInfo(child,reinterpret_cast<PROCESS_MEMORY_COUNTERS*>(&memory),sizeof(memory)));
            std::printf("PERF preload_ms=%.3f first_visible_ms=%.3f warm_median_ms=%.3f warm_p95_ms=%.3f host_private_mib=%.2f host_working_set_mib=%.2f samples=30\n",preloadMs,coldMs,samples[15],samples[28],memory.PrivateUsage/1048576.0,memory.WorkingSetSize/1048576.0);
        }
    }
    assert(WaitForSingleObject(child,5000) == WAIT_OBJECT_0); CloseHandle(child);
    DestroyWindow(callback); FreeLibrary(module);
    std::puts("PASS: real WinUI host, no native frame, inherited handle, no activation, hide/reopen and shutdown");
}
