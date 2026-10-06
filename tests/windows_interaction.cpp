#include "engine.h"
#include "key_policy.h"
#include "special_input.h"
#include <cassert>
#include <fstream>
#include <iostream>
#include <algorithm>
int wmain(int argc,wchar_t** argv) {
    assert(argc == 2);
    shanjie::Snapshot frame; frame.candidates = {L"你",L"妳"}; frame.selected = 0;
    ShanjieKey down{}; down.kind = 10;
    assert(shanjie::windowsKey(down,frame).kind == 8);
    ShanjieKey tab{}; tab.kind = 13;
    assert(shanjie::windowsKey(tab,frame).kind == 10);
    Preferences preferences; SpecialInput special;
    ShanjieKey key{}; key.kind = 1; key.ch = '`';
    assert(special.starts(key,preferences)); special.key(key,preferences);
    key.ch = 'u'; special.key(key,preferences);
    for (auto ch : "1f600") { key.ch = ch; special.key(key,preferences); }
    key.kind = 2;
    assert(special.key(key,preferences).commit == L"\U0001f600");
    key.kind = 1; key.ch = '`'; special.key(key,preferences); key.ch = 'b'; special.key(key,preferences);
    for (auto ch : "a440") { key.ch = ch; special.key(key,preferences); }
    key.kind = 2; assert(special.key(key,preferences).commit == L"一");
    key.kind = 1; key.ch = '`'; special.key(key,preferences); key.ch = ',';
    assert(special.key(key,preferences).commit == L"，");
    key.ch = ','; key.modifiers = 6; special.key(key,preferences); key.modifiers = 0; key.kind = 10;
    assert(special.key(key,preferences).selected == 1);
    assert(special.pick(1).commit == L"。");
    shanjie::Engine first,second;
    assert(first.open(argv[1],0,0,true,true)); assert(second.open(argv[1],0,0,true,true));
    first.context(L"我"); second.context(L"你");
    auto learn = [](shanjie::Engine& engine,const char* reading) {
        shanjie::Snapshot out; ShanjieKey k{}; k.kind = 1;
        for (auto ch : std::string(reading)) { k.ch = ch; assert(engine.key(k,out)); }
        k.kind = 10; assert(engine.key(k,out)); assert(out.candidates.size() > 1);
        auto selected = std::find_if(out.candidates.begin(),out.candidates.end(),[&out](const auto& word) { return word != out.preedit; });
        assert(selected != out.candidates.end());
        assert(engine.pick((unsigned)(selected - out.candidates.begin()),out)); k.kind = 3; assert(engine.key(k,out));
    };
    learn(first,"su3"); learn(second,"cl3");
    wchar_t local[32768]; GetEnvironmentVariableW(L"LOCALAPPDATA",local,32768);
    auto path = std::filesystem::path(local) / L"Shanjie" / L"learning.tsv";
    std::ifstream input(path); std::string text((std::istreambuf_iterator<char>(input)),{});
    assert(text.find("ㄋㄧˇ") != std::string::npos); assert(text.find("ㄏㄠˇ") != std::string::npos);
    std::cout << "PASS: candidate policy, Unicode/Big5/symbols, interleaved learning\n";
}
