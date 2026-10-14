#include "engine.h"
#include "key_policy.h"
#include "special_input.h"
#include "pinyin_input.h"
#include "associated_candidates.h"
#include "rust_bridge.h"
#include <cassert>
#include <fstream>
#include <iostream>
#include <algorithm>
int wmain(int argc,wchar_t** argv) {
    assert(argc == 2);
    Preferences preferences;
    shanjie::Snapshot frame; frame.candidates = {L"你",L"妳"}; frame.selected = 0;
    ShanjieKey down{}; down.kind = 10;
    assert(shanjie::windowsKey(down,frame,preferences).kind == 8);
    ShanjieKey tab{}; tab.kind = 13;
    assert(shanjie::windowsKey(tab,frame,preferences).kind == 8);
    frame.prediction = true; frame.selected = -1; assert(shanjie::windowsKey(tab,frame,preferences).kind == 13);
    frame.prediction = false; frame.selected = 0;
    SpecialInput special;
    ShanjieKey key{}; key.kind = 1; key.ch = '`';
    assert(special.starts(key,preferences)); special.key(key,preferences);
    key.ch = 'u'; special.key(key,preferences);
    for (auto ch : "1f600") { key.ch = ch; special.key(key,preferences); }
    key.kind = 2;
    assert(special.key(key,preferences).commit == L"\U0001f600");
    key.kind = 1; key.ch = '`'; special.key(key,preferences); key.ch = 'b'; special.key(key,preferences);
    for (auto ch : "a440") { key.ch = ch; special.key(key,preferences); }
    key.kind = 2; assert(special.key(key,preferences).commit == L"一");
    preferences.backquoteSymbols = true; key.kind = 1; key.ch = '`'; special.key(key,preferences); key.ch = ',';
    assert(special.key(key,preferences).commit == L"，");
    key.ch = ','; key.modifiers = 6; special.key(key,preferences); key.modifiers = 0; key.kind = 10;
    assert(special.key(key,preferences).selected == 1);
    assert(special.pick(1).commit == L"。");
    preferences.symbolKeyboard = false; key.kind = 1; key.ch = ','; key.modifiers = 6;
    assert(!special.starts(key,preferences)); preferences.symbolKeyboard = true;
    key.ch = '['; key.modifiers = 2; assert(shanjie::windowsPunctuation(key)); assert(shanjie::windowsPunctuationText(key) == L"「");
    key.modifiers = 3; assert(shanjie::windowsPunctuationText(key) == L"『");
    key.ch = '1'; key.modifiers = 2; assert(!shanjie::windowsPunctuation(key));
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
    input.close();
    assert(text.find("ㄋㄧˇ") != std::string::npos); assert(text.find("ㄏㄠˇ") != std::string::npos);
    shanjie::Snapshot pending; key.kind = 1; key.modifiers = 0;
    for (auto ch : std::string("su3")) { key.ch = ch; assert(first.key(key,pending)); }
    key.kind = 10; assert(first.key(key,pending));
    auto different = std::find_if(pending.candidates.begin(),pending.candidates.end(),[&pending](const auto& word) { return word != pending.preedit; });
    assert(different != pending.candidates.end()); assert(first.pick((unsigned)(different - pending.candidates.begin()),pending));
    std::ofstream(path.parent_path() / L"learning.epoch") << "cleared";
    auto archive=path.parent_path()/L".deleted_files"; std::filesystem::create_directory(archive);
    std::filesystem::rename(path,archive/L"learning-before-clear.tsv");
    shanjie::Snapshot clearOutput; key.kind = 3; assert(first.key(key,clearOutput)); assert(!clearOutput.commit.empty()); assert(!std::filesystem::exists(path));
    shanjie::Snapshot suggestions; assert(first.associate(L"你",suggestions)); assert(!suggestions.candidates.empty());
    AssociatedCandidates associated; associated.open(suggestions); key.kind = 1; key.ch = '1'; key.modifiers = 1;
    assert(associated.handles(key)); assert(!associated.key(key).commit.empty());
    shanjie::Snapshot composing; composing.preedit = L"你"; composing.cursor = 1;
    associated.open(suggestions,composing);
    assert(associated.view().preedit == L"你");
    key.kind = 3; key.modifiers = 0; assert(!associated.handles(key));
    key.kind = 6; assert(associated.key(key).preedit == L"你"); assert(!associated.active());
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
    {
        shanjie::Engine engine; assert(engine.open(argv[1],0)); PinyinInput pinyin; assert(pinyin.load(argv[1],false));
        key.kind = 1; key.modifiers = 0;
        for (auto ch : std::string("zhuyin")) { key.ch = ch; assert(pinyin.key(key,engine,{},out,true,true)); }
        key.kind = 2; assert(pinyin.key(key,engine,{},out,true,true)); assert(out.preedit == L"注音");
    }
    for (unsigned model : {5u}) {
        shanjie::Engine engine; assert(engine.open(argv[1],0,0,true,false,false,L"dictionary.tsv",model));
        assert(engine.inputOptions(false,false,false)); key.kind = 1; key.modifiers = 0; key.ch = '3';
        assert(engine.key(key,out)); assert(!out.handled && out.preedit.empty()); // Bare tone is the original digit.
        key.ch = 's'; assert(engine.key(key,out)); assert(out.candidates.empty());
        assert(engine.prediction(true,out)); assert(out.prediction && out.selected == -1 && !out.candidates.empty());
        assert(engine.key(tab,out)); assert(out.prediction && out.selected == 0 && !out.columns);
        assert(engine.prediction(false,out)); assert(out.candidates.empty() && !out.prediction && out.preedit == L"ㄋ");
        assert(engine.reset(true,out)); assert(engine.inputOptions(true,true,true)); key.ch = '3';
        assert(engine.key(key,out)); assert(out.preedit == L"ˇ"); assert(engine.reset(true,out));
        key.ch = 's'; assert(engine.key(key,out)); key.ch = 'u'; assert(engine.key(key,out)); key.ch = '3'; assert(engine.key(key,out));
        assert(engine.cycleCandidate(out)); auto cycled = out.preedit; assert(cycled != L"你" && out.candidates.empty() && out.cursor == 1);
        assert(engine.cycleCandidate(out)); assert(out.preedit != cycled && out.candidates.empty() && out.cursor == 1);
        assert(engine.reset(true,out)); key.ch = 's'; assert(engine.key(key,out)); key.ch = 'u'; assert(engine.key(key,out)); key.ch = '3'; assert(engine.key(key,out));
        assert(engine.punctuation(L"「",out)); assert(out.preedit == L"你「" && out.commit.empty());
        // Zhuyin order: final before initial (0d4) must reach the same syllable as d04 (看).
        for (auto keys : {std::string("d04"), std::string("0d4")}) {
            assert(engine.reset(true,out));
            for (auto ch : keys) { key.ch = ch; assert(engine.key(key,out)); }
            assert(out.preedit == L"看");
        }
        // Per-key switches: plain [ alone must not also turn \ into 、.
        assert(engine.reset(true,out)); assert(sj_engine_options(engine.native(),1u<<14));
        key.ch = '['; assert(engine.key(key,out)); assert(out.handled && out.preedit == L"「");
        key.ch = '\\'; assert(engine.key(key,out)); assert(out.preedit.find(L"、") == std::wstring::npos);
    }
    auto modelRoot = std::filesystem::path(argv[1]) / L"models";
    {
        shanjie::Engine engine; assert(engine.open(argv[1],0));
        assert(engine.smartInput(1,out));
        auto type = [&](const char* keys) {
            ShanjieKey k{}; k.kind = 1;
            for (auto ch : std::string(keys)) { k.ch = ch; assert(engine.key(k,out)); }
        };
        type("gj"); assert(out.preedit == L"gj" && out.readingHint && out.selected == -1 && out.candidates == std::vector<std::wstring>{L"ㄕㄨ"});
        assert(!engine.pick(0,out));
        ShanjieKey enter{}; enter.kind = 3; assert(engine.key(enter,out)); assert(out.commit == L"gj");
        type("su"); assert(out.preedit == L"su" && out.readingHint); type("3"); assert(out.preedit == L"你" && !out.readingHint);
        ShanjieKey space{}; space.kind = 2;
        assert(engine.reset(true,out)); type("callsu3"); assert(out.preedit == L"call你");
        assert(engine.reset(true,out)); type("hello"); assert(engine.key(space,out)); assert(out.preedit == L"hello ");
        type("world"); assert(engine.key(enter,out)); assert(out.commit == L"hello world");
        type("hello123"); assert(engine.key(enter,out)); assert(out.commit == L"hello123");
        type("hello,su3"); assert(engine.key(enter,out)); assert(out.commit == L"hello,你");
        assert(engine.reset(true,out)); assert(engine.smartInput(0,out)); type("sc"); assert(out.candidates.empty());
        std::cout << "PASS: raw English preview, non-selectable Zhuyin hint, tone conversion, mixed words and switch off\n";
    }
    // Reuse the converter's loader through the native adapter with an isolated data root.
    auto wrongData = std::filesystem::path(local) / L"wrong-data";
    std::filesystem::create_directory(wrongData);
    for (auto& file : std::filesystem::directory_iterator(argv[1])) if (file.is_regular_file()) std::filesystem::create_hard_link(file.path(),wrongData / file.path().filename());
    auto wrongModel = wrongData / L"models/model-v5"; std::filesystem::create_directories(wrongModel);
    std::filesystem::create_hard_link(modelRoot / L"model-v5/bigram.winmap",wrongModel / L"bigram.winmap");
    shanjie::Engine wrong; assert(!wrong.open(wrongData,0,0,true,false,false,L"dictionary.tsv",5));
    std::ofstream(wrongModel / L"classes.sjc") << "wrong class pairing";
    assert(!wrong.open(wrongData,0,0,true,false,false,L"dictionary.tsv",5));
    std::filesystem::copy_file(modelRoot / L"model-v5/classes.sjc",wrongModel / L"classes.sjc",std::filesystem::copy_options::overwrite_existing);
    assert(wrong.open(wrongData,0,0,true,false,false,L"dictionary.tsv",5));
    std::ofstream(wrongModel / L"classes.sjc") << "changed after cache warmup";
    shanjie::Engine invalidCached; assert(!invalidCached.open(wrongData,0,0,true,false,false,L"dictionary.tsv",5));
    std::filesystem::copy_file(modelRoot / L"model-v5/classes.sjc",wrongModel / L"classes.sjc",std::filesystem::copy_options::overwrite_existing);
    assert(invalidCached.open(wrongData,0,0,true,false,false,L"dictionary.tsv",5));
    std::cout << "PASS: latest model, required class pairing, passive/entered prediction, live off, key options and composed punctuation\n";
    std::cout << "PASS: candidate policy, Unicode/Big5/symbols, interleaved learning, personal dictionary, associations, pinyin\n";
}
