#pragma once
#include "engine.h"
struct WinFrame {
    unsigned handled, associated, prediction;
    const char *commit, *preedit;
    unsigned cursor, columns, first, total;
    int selected; unsigned count; const char** candidates;
};
extern "C" {
void sj_frame_free(WinFrame*);
void* sj_engine_new(const char*,unsigned,unsigned,unsigned,const char*); void sj_engine_free(void*);
WinFrame* sj_engine_action(void*,unsigned,ShanjieKey,unsigned,const char*);
void sj_engine_context(void*,const char*); bool sj_engine_options(void*,unsigned);
bool sj_translate(size_t,intptr_t,ShanjieKey*);
ShanjieKey sj_candidate_key(ShanjieKey,const WinFrame*,bool);
const char* sj_punctuation(ShanjieKey); bool sj_is_punctuation(ShanjieKey); bool sj_pending(const WinFrame*);
void* sj_special_new(); void sj_special_free(void*); unsigned sj_special_mode(const void*);
void sj_special_clear(void*); bool sj_special_starts(ShanjieKey,const char*);
WinFrame* sj_special_key(void*,ShanjieKey,const char*); WinFrame* sj_special_pick(void*,unsigned);
void* sj_associated_new(); void sj_associated_free(void*); bool sj_associated_active(const void*);
void sj_associated_clear(void*); void sj_associated_open(void*,const WinFrame*,const WinFrame*);
WinFrame* sj_associated_view(const void*); bool sj_associated_handles(const void*,ShanjieKey);
WinFrame* sj_associated_key(void*,ShanjieKey); WinFrame* sj_associated_pick(void*,unsigned);
void* sj_pinyin_new(); void sj_pinyin_free(void*); bool sj_pinyin_load(void*,const char*,bool);
bool sj_pinyin_enabled(const void*); bool sj_pinyin_active(const void*);
bool sj_pinyin_handles(const void*,ShanjieKey); void sj_pinyin_clear(void*);
WinFrame* sj_pinyin_key(void*,ShanjieKey,void*,const WinFrame*,bool,bool);
void* sj_host_new(intptr_t); void sj_host_free(void*);
void sj_host_publish(void*,const WinFrame*,unsigned,unsigned,RECT,bool);void sj_host_vertical(void*,bool);void sj_host_hide(void*);
bool sj_host_shown(const void*); intptr_t sj_host_window(const void*); int sj_host_sequence(const void*);
void* sj_document_open(const char*);void sj_document_free(void*);
char* sj_document_get(const void*,const char*,const char*,const char*);
char* sj_profile_load_text(); void sj_string_free(char*);
}
struct FrameView {
    std::string commit,preedit; std::vector<std::string> words; std::vector<const char*> pointers; WinFrame raw;
    explicit FrameView(const shanjie::Snapshot& s):commit(shanjie::utf8(s.commit)),preedit(shanjie::utf8(s.preedit)) {
        for (auto& word:s.candidates) words.push_back(shanjie::utf8(word)); for (auto& word:words) pointers.push_back(word.c_str());
        raw={s.handled,s.associated,unsigned(s.prediction) | (unsigned(s.readingHint) << 1) | (unsigned(s.phoneticPending) << 2),commit.c_str(),preedit.c_str(),s.cursor,s.columns,s.first,s.total,s.selected,(unsigned)pointers.size(),pointers.data()};
    }
};
inline shanjie::Snapshot takeFrame(WinFrame* f) {
    shanjie::Snapshot s;s.handled=f->handled!=0;s.associated=f->associated!=0;s.prediction=(f->prediction&1)!=0;s.readingHint=(f->prediction&2)!=0;s.phoneticPending=(f->prediction&4)!=0;
    s.commit=shanjie::wide(f->commit);s.preedit=shanjie::wide(f->preedit);s.cursor=f->cursor;
    s.columns=f->columns;s.first=f->first;s.total=f->total;s.selected=f->selected;
    for(unsigned i=0;i<f->count;++i)s.candidates.push_back(shanjie::wide(f->candidates[i]));sj_frame_free(f);return s;
}
