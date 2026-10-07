#include "engine.h"
#include "key_policy.h"
#include "special_input.h"
#include "pinyin_input.h"
#include "associated_candidates.h"
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
    std::ofstream(path.parent_path() / L"learning.epoch") << "cleared";
    std::filesystem::remove(path);
    shanjie::Snapshot clearOutput; assert(first.reset(false,clearOutput)); assert(!std::filesystem::exists(path));
    shanjie::Snapshot suggestions; assert(first.associate(L"你",suggestions)); assert(!suggestions.candidates.empty());
    AssociatedCandidates associated; associated.open(suggestions); key.kind = 1; key.ch = '1'; key.modifiers = 1;
    assert(associated.handles(key)); assert(!associated.key(key).commit.empty());
    auto dictionary = std::filesystem::path(local) / L"Shanjie" / L"dictionary.tsv";
    std::ofstream(dictionary) << "ㄋㄧˇ-ㄏㄠˇ\t妳郝\t-0.01\n";
    shanjie::Engine personal; assert(personal.open(argv[1],0,0,true));
    shanjie::Snapshot out; key.kind = 1; key.modifiers = 0;
    for (auto ch : std::string("su3cl3")) { key.ch = ch; assert(personal.key(key,out)); }
    key.kind = 11; assert(personal.key(key,out)); key.kind = 10; assert(personal.key(key,out));
    assert(std::find(out.candidates.begin(),out.candidates.end(),L"妳郝") != out.candidates.end());
    assert(personal.commitReading(out)); assert(out.commit == L"ㄋㄧˇㄏㄠˇ");
    auto pinyinCheck = [&](bool taiwan,const std::string& typing) {
        shanjie::Engine engine; assert(engine.open(argv[1],0)); PinyinInput pinyin; assert(pinyin.load(argv[1],taiwan));
        shanjie::Snapshot current, next; ShanjieKey k{}; k.kind = 1;
        for (auto ch : typing) { k.ch = ch; assert(pinyin.key(k,engine,current,next,true)); current = next; }
        k.kind = 3; assert(pinyin.key(k,engine,current,next,true)); assert(next.commit == L"注音");
    };
    pinyinCheck(false,"zhu4yin1"); pinyinCheck(false,"zhuyin"); pinyinCheck(true,"jhu4yin1");
    std::cout << "PASS: candidate policy, Unicode/Big5/symbols, interleaved learning, personal dictionary, associations, pinyin\n";
}
