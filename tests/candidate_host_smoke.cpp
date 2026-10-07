#include "candidate_host.h"
#include "identity.h"
#include "native_capture.h"
#include <psapi.h>
#include <algorithm>
#include <cassert>
#include <cstdio>
HMODULE module = nullptr;
long objects = 0;
unsigned picks = 0;
LONG pickedSequence = 0;
LRESULT CALLBACK receive(HWND window,UINT message,WPARAM,LPARAM lp) {
    if (message == WM_APP+42) { ++picks; pickedSequence = (LONG)lp; return 0; }
    return DefWindowProcW(window,message,0,lp);
}
double clockMs() { LARGE_INTEGER value{},frequency{}; QueryPerformanceCounter(&value); QueryPerformanceFrequency(&frequency); return value.QuadPart*1000.0/frequency.QuadPart; }
PROCESS_MEMORY_COUNTERS_EX memory() { PROCESS_MEMORY_COUNTERS_EX m{}; m.cb = sizeof(m); assert(GetProcessMemoryInfo(GetCurrentProcess(),(PROCESS_MEMORY_COUNTERS*)&m,sizeof(m))); return m; }
int wmain(int argc,wchar_t** argv) {
    assert(argc >= 2); assert(SUCCEEDED(CoInitializeEx(nullptr,COINIT_APARTMENTTHREADED)));
    SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    module = LoadLibraryW(argv[1]); assert(module); SetEnvironmentVariableW(L"SHANJIE_VERIFY_UI",L"1");
    WNDCLASSW wc{}; wc.lpfnWndProc = receive; wc.hInstance = GetModuleHandleW(nullptr); wc.lpszClassName = L"ShanjieSurfaceSmoke"; RegisterClassW(&wc);
    auto callback = CreateWindowW(wc.lpszClassName,L"",0,0,0,0,0,HWND_MESSAGE,nullptr,wc.hInstance,nullptr);
    auto foreground = GetForegroundWindow(); auto before = memory(); double start = clockMs();
    {
        CandidateHost presenter; assert(presenter.start(callback)); double initialization = clockMs()-start;
        assert(!presenter.shown()); DWORD process = 0; GetWindowThreadProcessId(presenter.window(),&process); assert(process == GetCurrentProcessId());
        shanjie::Snapshot frame; frame.candidates = {L"你",L"妳",L"擬",L"禰",L"妮",L"尼",L"倪",L"泥",L"逆"}; frame.selected = 1; frame.total = 9;
        Preferences preferences; preferences.theme = 1; RECT caret{100,100,101,140}; start = clockMs(); presenter.publish(frame,preferences,caret,96,true); double first = clockMs()-start;
        assert(presenter.shown()); assert(GetForegroundWindow() == foreground); assert(SendMessageW(presenter.window(),WM_MOUSEACTIVATE,0,0) == MA_NOACTIVATE);
        RECT bounds{},client{}; GetWindowRect(presenter.window(),&bounds); GetClientRect(presenter.window(),&client);
        assert(bounds.right < -10000 && bounds.bottom < -10000); assert(client.right == bounds.right-bounds.left && client.bottom == bounds.bottom-bounds.top);
        SendMessageW(presenter.window(),WM_LBUTTONUP,0,MAKELPARAM(15,15)); MSG message; while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) DispatchMessageW(&message);
        assert(picks == 1 && pickedSequence == presenter.sequence()); presenter.hide(); assert(!presenter.shown()); assert(pickedSequence != presenter.sequence());
        std::vector<double> samples;
        for (int i=0;i<30;++i) { presenter.hide(); start = clockMs(); presenter.publish(frame,preferences,caret,96,true); samples.push_back(clockMs()-start); }
        std::sort(samples.begin(),samples.end()); auto after = memory();
        std::printf("PERF native_init_ms=%.3f first_draw_ms=%.3f warm_median_ms=%.3f warm_p95_ms=%.3f private_delta_mib=%.2f working_set_delta_mib=%.2f process_private_mib=%.2f samples=30\n",initialization,first,samples[15],samples[28],(after.PrivateUsage-before.PrivateUsage)/1048576.0,(after.WorkingSetSize-before.WorkingSetSize)/1048576.0,after.PrivateUsage/1048576.0);
        if (argc >= 3) {
            std::filesystem::path output(argv[2]); std::filesystem::create_directories(output);
            for (auto theme : {1u,2u}) {
                frame.candidates = {L"你",L"妳",L"擬",L"禰",L"妮",L"尼",L"倪",L"泥",L"逆"};
                preferences.theme = theme;
                for (auto layout : {CandidateLayout::Vertical,CandidateLayout::Horizontal,CandidateLayout::Grid}) {
                    preferences.layout = layout; frame.columns = layout == CandidateLayout::Grid ? 9 : 0;
                    presenter.publish(frame,preferences,caret,96,true);
                    captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-"+std::to_wstring((int)layout)+L".bmp"));
                }
                frame.candidates = {L"輸入法",L"候選視窗",L"善解注音",L"微軟注音",L"個人詞庫",L"全形標點",L"倚天鍵盤",L"使用者",L"設定"};
                for (auto size : {14u,32u}) {
                    preferences.fontSize = size; presenter.publish(frame,preferences,caret,96,true);
                    captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-grid-"+std::to_wstring(size)+L".bmp"));
                }
                SetEnvironmentVariableW(L"SHANJIE_VERIFY_WORK_WIDTH",L"720");
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-narrow-grid.bmp"));
                SetEnvironmentVariableW(L"SHANJIE_VERIFY_WORK_WIDTH",nullptr);
                preferences.fontSize = 18; preferences.layout = CandidateLayout::Vertical; frame.columns = 0; frame.associated = true;
                presenter.publish(frame,preferences,caret,96,true); captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-associated.bmp")); frame.associated = false;
                frame.candidates = {L"😀",L"👍🏽",L"👩‍💻",L"🇹🇼",L"❤️",L"文字😀"};
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-emoji.bmp"));
            }
        }
        presenter.hide(); assert(!presenter.shown());
    }
    DestroyWindow(callback); FreeLibrary(module); CoUninitialize();
    std::puts("PASS: in-process native candidates, mouse selection, stale click sequence, no activation, no frame, hide/reopen");
}
