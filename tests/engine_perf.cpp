#include "engine.h"
#include <psapi.h>
#include <algorithm>
#include <cassert>
#include <cstdio>
double now() { LARGE_INTEGER n{},f{}; QueryPerformanceCounter(&n); QueryPerformanceFrequency(&f); return n.QuadPart*1000.0/f.QuadPart; }
PROCESS_MEMORY_COUNTERS_EX memory() { PROCESS_MEMORY_COUNTERS_EX m{}; m.cb = sizeof(m); assert(GetProcessMemoryInfo(GetCurrentProcess(),(PROCESS_MEMORY_COUNTERS*)&m,sizeof(m))); return m; }
int wmain(int argc,wchar_t** argv) {
    assert(argc == 3 || argc == 4); bool learning = argc == 4 && std::wstring(argv[3]) == L"--learning";
    unsigned model = (unsigned)_wtoi(argv[2]); auto before = memory(); double start = now();
    shanjie::Engine engine; assert(engine.open(argv[1],0,0,false,learning,false,L"dictionary.tsv",model));
    double openMs = now()-start; auto opened = memory(); std::vector<double> samples; shanjie::Snapshot output;
    for (int i=0;i<100;++i) {
        assert(engine.reset(true,output));
        for (auto ch : std::string("su3cl3")) { start = now(); assert(engine.key({1,(unsigned)ch,0},output)); samples.push_back(now()-start); }
    }
    auto typed = memory(); std::sort(samples.begin(),samples.end());
    std::printf("ENGINE model=%u learning=%u open_ms=%.3f private_delta_mib=%.2f working_set_delta_mib=%.2f typed_private_delta_mib=%.2f key_median_ms=%.3f key_p95_ms=%.3f samples=%zu\n",model,learning,openMs,(opened.PrivateUsage-before.PrivateUsage)/1048576.0,(opened.WorkingSetSize-before.WorkingSetSize)/1048576.0,(typed.PrivateUsage-before.PrivateUsage)/1048576.0,samples[samples.size()/2],samples[samples.size()*95/100],samples.size());
}
