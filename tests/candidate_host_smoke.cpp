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
WPARAM pickedIndex = 0;
LRESULT CALLBACK receive(HWND window,UINT message,WPARAM wp,LPARAM lp) {
    if (message == WM_APP+42) { ++picks; pickedSequence = (LONG)lp; pickedIndex = wp; return 0; }
    return DefWindowProcW(window,message,wp,lp);
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
        assert(!GetWindow(presenter.window(),GW_OWNER));
        RECT bounds{},client{}; GetWindowRect(presenter.window(),&bounds); GetClientRect(presenter.window(),&client);
        assert(bounds.right < -10000 && bounds.bottom < -10000); assert(client.right == bounds.right-bounds.left && client.bottom == bounds.bottom-bounds.top);
        std::printf("Native click geometry: dpi=%u client=%ldx%ld\n",GetDpiForWindow(presenter.window()),client.right,client.bottom);
        // Nine rows, and a click on row three picks index 2 under the current sequence.
        // The window carries a shadow margin around the panel (candidate.rs TOP and BOTTOM: 34 DIPs).
        LONG margin = MulDiv(34,GetDpiForWindow(presenter.window()),96);
        assert(client.bottom-margin > MulDiv(9*24,GetDpiForWindow(presenter.window()),96));
        SendMessageW(presenter.window(),WM_LBUTTONUP,0,MAKELPARAM(client.right/2,(LONG)(client.bottom*(2.5/9.0)))); MSG message;
        while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) DispatchMessageW(&message);
        assert(picks == 1 && pickedIndex == 2 && pickedSequence == presenter.sequence());
        // Three visual columns retain the core indices, including clicks in later rows.
        frame.columns = 9; frame.total = 27;
        for(int i=9;i<27;i++)frame.candidates.push_back(L"候選"+std::to_wstring(i+1));
        frame.selected = 10; presenter.publish(frame,preferences,caret,96,true);
        RECT grid{}; GetClientRect(presenter.window(),&grid);
        assert(grid.right > client.right);
        SendMessageW(presenter.window(),WM_LBUTTONUP,0,MAKELPARAM(MulDiv(35,GetDpiForWindow(presenter.window()),96),grid.bottom/2));
        while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) DispatchMessageW(&message);
        assert(picks == 2 && pickedIndex == 12);
        frame.candidates.resize(9); frame.columns = 0; frame.total = 9; frame.selected = 1;
        // The reading panel alone fits its text with no minimum width, and glides as the text grows and shrinks.
        {
            auto dpi = GetDpiForWindow(presenter.window());
            auto widthOf = [&](const std::wstring& text,bool withList) {
                shanjie::Snapshot panel; panel.preedit = text; panel.cursor = (unsigned)text.size();
                if (withList) { panel.candidates = {L"你",L"妳",L"擬"}; panel.selected = 0; panel.total = 3; }
                presenter.publish(panel,preferences,caret,96,true);
                RECT r{}; GetClientRect(presenter.window(),&r); return r.right-MulDiv(32,dpi,96);
            };
            preferences.fontSize = 16;
            LONG one = widthOf(L"ㄋ",false), two = widthOf(L"ㄋㄧ",false), four = widthOf(L"你好你好",false), sixteen = widthOf(L"你好你好你好你好你好你好你好你好",false);
            std::printf("Reading panel width (DIP): 1=%ld 2=%ld 4=%ld 16=%ld\n",MulDiv(one,96,dpi),MulDiv(two,96,dpi),MulDiv(four,96,dpi),MulDiv(sixteen,96,dpi));
            assert(MulDiv(one,96,dpi) < 50); assert(one < two && two < four && four < sixteen);
            assert(MulDiv(sixteen-four,96,dpi) >= 12*16-2 && MulDiv(sixteen-four,96,dpi) <= 12*16+2);
            assert(widthOf(L"ㄋ",false) == one);
            // Under a list the panel is the list's width until the sentence needs more, then shrinks back to it.
            LONG listed = widthOf(L"你",true), longer = widthOf(L"你好你好你好你好你好你好你好你好",true), back = widthOf(L"你",true);
            std::printf("Reading panel over a list (DIP): 1=%ld 16=%ld back=%ld\n",MulDiv(listed,96,dpi),MulDiv(longer,96,dpi),MulDiv(back,96,dpi));
            assert(longer > listed && back == listed);
            // Timed, with motion on: a width on screen between the two ends, moving one way, landing exactly.
            presenter.hide(); SetEnvironmentVariableW(L"SHANJIE_VERIFY_MOTION",L"1");
            auto glide = [&](const std::wstring& text,bool withList,LONG from,LONG to,const char* name) {
                std::vector<LONG> seen; shanjie::Snapshot panel; panel.preedit = text; panel.cursor = (unsigned)text.size();
                if (withList) { panel.candidates = {L"你",L"妳",L"擬"}; panel.selected = 0; panel.total = 3; }
                presenter.publish(panel,preferences,caret,96,true);
                double started = clockMs();
                while (clockMs()-started < 400) {
                    MSG pump; while (PeekMessageW(&pump,nullptr,0,0,PM_REMOVE)) DispatchMessageW(&pump);
                    RECT r{}; GetClientRect(presenter.window(),&r); LONG w = r.right-MulDiv(32,dpi,96);
                    if (seen.empty() || seen.back() != w) seen.push_back(w);
                    Sleep(1);
                }
                std::printf("Glide %s: %ld to %ld through %zu widths\n",name,from,to,seen.size());
                std::fflush(stdout);
                assert(seen.back() == to); assert(seen.size() >= 5);
                // One way, give or take the curve's slight overshoot near the end.
                for (size_t i = 1; i < seen.size(); ++i) assert(to > from ? seen[i] >= seen[i-1]-2 : seen[i] <= seen[i-1]+2);
            };
            presenter.publish([&]{ shanjie::Snapshot s; s.preedit = L"ㄋ"; s.cursor = 1; return s; }(),preferences,caret,96,true);
            Sleep(400); { MSG pump; while (PeekMessageW(&pump,nullptr,0,0,PM_REMOVE)) DispatchMessageW(&pump); }
            glide(L"你好你好你好你好你好你好你好你好",false,one,sixteen,"panel grows");
            glide(L"ㄋ",false,sixteen,one,"panel shrinks");
            presenter.hide(); presenter.publish([&]{ shanjie::Snapshot s; s.preedit = L"你"; s.cursor = 1; s.candidates = {L"你",L"妳",L"擬"}; s.selected = 0; s.total = 3; return s; }(),preferences,caret,96,true);
            Sleep(400); { MSG pump; while (PeekMessageW(&pump,nullptr,0,0,PM_REMOVE)) DispatchMessageW(&pump); }
            glide(L"你好你好你好你好你好你好你好你好",true,listed,longer,"with a list grows");
            glide(L"你",true,longer,listed,"with a list shrinks");
            SetEnvironmentVariableW(L"SHANJIE_VERIFY_MOTION",nullptr);
        }
        presenter.hide(); assert(!presenter.shown()); assert(pickedSequence != presenter.sequence());
        for (auto notification : {WM_SETTINGCHANGE,WM_THEMECHANGED,WM_DWMCOLORIZATIONCOLORCHANGED}) {
            SendMessageW(presenter.window(),notification,0,0);
            assert(!presenter.shown());
        }
        std::vector<double> samples;
        for (int i=0;i<30;++i) { presenter.hide(); start = clockMs(); presenter.publish(frame,preferences,caret,96,true); samples.push_back(clockMs()-start); }
        std::sort(samples.begin(),samples.end()); auto after = memory();
        std::printf("PERF native_init_ms=%.3f first_draw_ms=%.3f warm_median_ms=%.3f warm_p95_ms=%.3f private_delta_mib=%.2f working_set_delta_mib=%.2f process_private_mib=%.2f samples=30\n",initialization,first,samples[15],samples[28],(after.PrivateUsage-before.PrivateUsage)/1048576.0,(after.WorkingSetSize-before.WorkingSetSize)/1048576.0,after.PrivateUsage/1048576.0);
        if (argc >= 3) {
            std::filesystem::path output(argv[2]); std::filesystem::create_directories(output);
            for (auto theme : {1u,2u}) {
                frame.candidates = {L"你",L"妳",L"擬",L"禰",L"妮",L"尼",L"倪",L"泥",L"逆"};
                preferences.theme = theme; preferences.fontSize = 16;
                frame.columns = 0; presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-vertical.bmp"));
                for(int i=9;i<36;i++)frame.candidates.push_back(L"候選"+std::to_wstring(i+1));
                frame.columns = 9; frame.total = 36; frame.selected = 10;
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-grid.bmp"));
                SetEnvironmentVariableW(L"SHANJIE_VERIFY_WORK_WIDTH",L"720");
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-grid-narrow.bmp"));
                SetEnvironmentVariableW(L"SHANJIE_VERIFY_WORK_WIDTH",nullptr);
                frame.columns = 0;
                frame.candidates = {L"比方",L"彼方",L"比訪"}; frame.selected = 0;
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"reference-"+std::to_wstring(theme)+L"-conversion.bmp"));
                frame.candidates = {L"輸入法",L"候選視窗",L"善解注音",L"微軟注音",L"個人詞庫",L"全形標點",L"倚天鍵盤",L"使用者",L"設定"};
                for (auto size : {14u,32u}) {
                    preferences.fontSize = size; presenter.publish(frame,preferences,caret,96,true);
                    captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-vertical-"+std::to_wstring(size)+L".bmp"));
                }
                SetEnvironmentVariableW(L"SHANJIE_VERIFY_WORK_WIDTH",L"720");
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-narrow.bmp"));
                SetEnvironmentVariableW(L"SHANJIE_VERIFY_WORK_WIDTH",nullptr);
                preferences.fontSize = 16; frame.columns = 0; frame.associated = true;
                presenter.publish(frame,preferences,caret,96,true); captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-associated.bmp")); frame.associated = false;
                frame.candidates = {L"😀",L"👍🏽",L"👩‍💻",L"🇹🇼",L"❤️",L"文字😀"};
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"candidates-"+std::to_wstring(theme)+L"-emoji.bmp"));
                frame.prediction = true; frame.selected = -1; frame.candidates = {L"比方說",L"說",L"數",L"說明",L"率"};
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"prediction-"+std::to_wstring(theme)+L"-passive.bmp"));
                frame.selected = 0; presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"prediction-"+std::to_wstring(theme)+L"-entered.bmp"));
                frame.prediction = false; frame.selected = 1;
                frame.readingHint = true; frame.prediction = true; frame.selected = -1; frame.candidates = {L"ㄕㄨ"};
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"mixed-hint-"+std::to_wstring(theme)+L".bmp"));
                auto beforeHintClick = picks;
                GetClientRect(presenter.window(),&client);
                SendMessageW(presenter.window(),WM_LBUTTONUP,0,MAKELPARAM(client.right/2,client.bottom/2));
                while (PeekMessageW(&message,nullptr,0,0,PM_REMOVE)) DispatchMessageW(&message);
                assert(picks == beforeHintClick);
                frame.readingHint = false; frame.prediction = false; frame.selected = 1;
                frame.preedit = L"善ㄐㄧㄝˇ"; frame.candidates = {L"解",L"姐",L"借",L"介",L"界"};
                presenter.publish(frame,preferences,caret,96,true);
                captureNative(presenter.window(),output/(L"composing-"+std::to_wstring(theme)+L".bmp"));
                frame.candidates.clear(); frame.selected = -1;
                presenter.publish(frame,preferences,caret,96,true); assert(presenter.shown());
                captureNative(presenter.window(),output/(L"composing-"+std::to_wstring(theme)+L"-reading.bmp"));
                for (auto [name,text] : {std::pair{L"1",L"ㄋ"},std::pair{L"4",L"你好你好"},std::pair{L"16",L"你好你好你好你好你好你好你好你好"}}) {
                    frame.preedit = text; frame.cursor = (uint32_t)wcslen(text); frame.candidates.clear(); frame.selected = -1;
                    presenter.publish(frame,preferences,caret,96,true);
                    captureNative(presenter.window(),output/(L"panel-"+std::to_wstring(theme)+L"-"+name+L".bmp"));
                    frame.candidates = {L"你",L"妳",L"擬"}; frame.selected = 0;
                    presenter.publish(frame,preferences,caret,96,true);
                    captureNative(presenter.window(),output/(L"panel-"+std::to_wstring(theme)+L"-"+name+L"-list.bmp"));
                }
                frame.preedit.clear(); frame.cursor = 0; frame.selected = 1;
            }
        }
        presenter.hide(); assert(!presenter.shown());
    }
    DestroyWindow(callback); FreeLibrary(module); CoUninitialize();
    std::puts("PASS: in-process native candidates, mouse selection, stale click sequence, no activation, no frame, hide/reopen");
}
