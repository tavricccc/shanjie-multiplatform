#pragma once
#include "engine.h"
#include <unordered_map>
class PinyinInput {
    struct Entry { std::string keys; double score; };
    std::unordered_map<std::string,Entry> index_;
    std::string pending_;
    shanjie::Snapshot base_;
    bool convert(shanjie::Engine&,shanjie::Snapshot&,bool alphanumeric);
    shanjie::Snapshot preview() const;
public:
    bool load(const std::filesystem::path& data,bool taiwan);
    bool enabled() const { return !index_.empty(); }
    bool active() const { return !pending_.empty(); }
    bool handles(const ShanjieKey&) const;
    bool key(const ShanjieKey&,shanjie::Engine&,const shanjie::Snapshot&,shanjie::Snapshot&,bool alphanumeric,bool spaceConverts = false);
    void clear() { pending_.clear(); base_ = {}; }
};
