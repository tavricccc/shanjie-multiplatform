#include "pinyin_input.h"
#include <fstream>
#include <sstream>
#include <algorithm>
#include <limits>
bool PinyinInput::load(const std::filesystem::path& data,bool taiwan) {
    index_.clear(); clear();
    std::unordered_map<std::wstring,std::string> spellings;
    std::ifstream mapping(data / L"pinyin-map.tsv"); if (!mapping) return false;
    std::string line;
    while (std::getline(mapping,line)) {
        std::istringstream fields(line); std::string reading,hanyu,tongyong; fields >> reading >> hanyu >> tongyong;
        auto sound = shanjie::wide(reading.c_str()); auto roman = taiwan ? tongyong : hanyu;
        // Passport spellings omit umlauts; an input method uses v to disambiguate n/l + ü.
        if (!taiwan && (sound.starts_with(L"ㄋㄩ") || sound.starts_with(L"ㄌㄩ"))) roman = roman.substr(0,1) + "v" + roman.substr(2);
        spellings[sound] = roman;
    }
    const std::wstring symbols = L"ㄅㄆㄇㄈㄉㄊㄋㄌㄍㄎㄏㄐㄑㄒㄓㄔㄕㄖㄗㄘㄙㄧㄨㄩㄚㄛㄜㄝㄞㄟㄠㄡㄢㄣㄤㄥㄦ";
    const std::string keys = "1qaz2wsxedcrfv5tgbyhnujm8ik,9ol.0p;/-";
    auto add = [&](const std::string& roman,const std::string& sequence,double score) {
        auto [entry,inserted] = index_.try_emplace(roman,Entry{sequence,score});
        if (!inserted && score > entry->second.score) entry->second = {sequence,score};
    };
    for (auto file : {L"mcbpmf-data.txt",L"overlay-add.tsv",L"sandhi-add.tsv"}) {
        std::ifstream input(data / file); if (!input) return false;
        while (std::getline(input,line)) {
            if (line.empty() || line[0] == '#' || line[0] == '_') continue;
            std::istringstream fields(line); std::string reading,word; double score = 0;
            if (!(fields >> reading >> word >> score)) continue;
            std::string plain,toned,sequence; bool valid = true;
            std::istringstream syllables(reading); std::string part;
            while (std::getline(syllables,part,'-')) {
                auto sound = shanjie::wide(part.c_str()); unsigned tone = 1;
                for (auto mark : {L'ˊ',L'ˇ',L'ˋ',L'˙'}) {
                    auto position = sound.find(mark);
                    if (position != std::wstring::npos) { tone = mark == L'ˊ' ? 2 : mark == L'ˇ' ? 3 : mark == L'ˋ' ? 4 : 5; sound.erase(position,1); }
                }
                auto spelling = spellings.find(sound); if (spelling == spellings.end()) { valid = false; break; }
                plain += spelling->second; toned += spelling->second + (char)('0' + tone);
                for (auto symbol : sound) {
                    auto position = symbols.find(symbol); if (position == std::wstring::npos) { valid = false; break; }
                    sequence += keys[position];
                }
                sequence += tone == 1 ? ' ' : tone == 2 ? '6' : tone == 3 ? '3' : tone == 4 ? '4' : '7';
            }
            if (valid) {
                add(plain,sequence,score); add(toned,sequence,score);
                if (!taiwan) {
                    for (const auto& alias : {std::pair{"nve","nue"},std::pair{"lve","lue"}}) {
                        auto alternative = plain, tonedAlternative = toned;
                        size_t at = 0;
                        while ((at = alternative.find(alias.first,at)) != std::string::npos) { alternative.replace(at,3,alias.second); at += 3; }
                        at = 0;
                        while ((at = tonedAlternative.find(alias.first,at)) != std::string::npos) { tonedAlternative.replace(at,3,alias.second); at += 3; }
                        add(alternative,sequence,score); add(tonedAlternative,sequence,score);
                    }
                }
            }
        }
    }
    return enabled();
}
bool PinyinInput::handles(const ShanjieKey& key) const {
    return enabled() && !(key.modifiers & 30) && (active() || (key.kind == 1 && key.ch >= 'a' && key.ch <= 'z' && key.modifiers == 0));
}
shanjie::Snapshot PinyinInput::preview() const {
    auto out = base_; out.handled = true; out.commit.clear(); out.candidates.clear(); out.selected = -1;
    auto text = shanjie::wide(pending_.c_str()); out.preedit.insert(out.cursor,text); out.cursor += (unsigned)text.size(); return out;
}
bool PinyinInput::convert(shanjie::Engine& engine,shanjie::Snapshot& out,bool alphanumeric) {
    std::string roman = pending_; roman.erase(std::remove(roman.begin(),roman.end(),' '),roman.end());
    if (roman.empty()) { out = base_; pending_.clear(); return true; }
    struct Plan { double score = -std::numeric_limits<double>::infinity(); std::string keys; };
    std::vector<Plan> plans(roman.size()+1); plans[0].score = 0;
    for (size_t end = 1; end <= roman.size(); ++end) {
        for (size_t start = 0; start < end; ++start) {
            auto found = index_.find(roman.substr(start,end-start)); if (found == index_.end()) continue;
            auto score = plans[start].score + found->second.score - 0.5;
            if (score > plans[end].score) plans[end] = {score,plans[start].keys + found->second.keys};
        }
    }
    if (plans.back().keys.empty()) {
        if (!alphanumeric) { out = preview(); return true; }
        if (!engine.reset(false,out)) return false;
        out.commit += shanjie::wide(pending_.c_str()); out.handled = true; pending_.clear(); base_ = {}; return true;
    }
    std::wstring committed;
    for (auto ch : plans.back().keys) {
        ShanjieKey key{}; key.kind = ch == ' ' ? 2 : 1; key.ch = ch;
        if (!engine.key(key,out)) return false;
        committed += out.commit;
    }
    out.commit = std::move(committed);
    pending_.clear(); base_ = out; return true;
}
bool PinyinInput::key(const ShanjieKey& key,shanjie::Engine& engine,const shanjie::Snapshot& current,shanjie::Snapshot& out,bool alphanumeric) {
    if (!active()) {
        base_ = current; base_.commit.clear();
        if (!base_.candidates.empty()) { ShanjieKey escape{}; escape.kind = 6; if (!engine.key(escape,base_)) return false; }
    }
    if (key.kind == 1 && key.ch >= 'a' && key.ch <= 'z' && key.modifiers == 0) {
        if (pending_.size() < 64) pending_ += (char)key.ch; out = preview(); return true;
    }
    if (key.kind == 1 && key.ch >= '1' && key.ch <= '5' && key.modifiers == 0) { pending_ += (char)key.ch; return convert(engine,out,alphanumeric); }
    if (key.kind == 4) { if (!pending_.empty()) pending_.pop_back(); out = preview(); return true; }
    if (key.kind == 6) { pending_.clear(); out = base_; out.handled = true; return true; }
    if (key.kind == 2 && !pending_.ends_with(' ')) { pending_ += ' '; out = preview(); return true; }
    if (!convert(engine,out,alphanumeric)) return false;
    if (active() || key.kind == 2) return true;
    auto committed = out.commit;
    if (!engine.key(key,out)) return false;
    out.commit = committed + out.commit; return true;
}
