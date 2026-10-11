#include "preferences.h"
#include "rust_bridge.h"
extern "C" unsigned long long sj_profile_stamp();
unsigned long long preferencesStamp(){return sj_profile_stamp();}
Preferences loadPreferences() {
    Preferences out;auto content=sj_profile_load_text();if(!content)return out;
    auto doc=sj_document_open(content);sj_string_free(content);
    auto read=[doc](const char* section,const char* key,const char* initial){auto value=sj_document_get(doc,section,key,initial);std::string text(value);sj_string_free(value);return text;};
    auto number=[&read](const char* s,const char* k,unsigned v){return (unsigned)std::stoul(read(s,k,std::to_string(v).c_str()));};
    auto flag=[&read](const char* s,const char* k,bool v){return read(s,k,v?"1":"0")=="1";};
    out.windowsKeys=read("profile","interaction","windows")=="windows";
    auto trigger=read("candidates","open","down");out.trigger=trigger=="space"?CandidateTrigger::Space:trigger=="both"?CandidateTrigger::Both:CandidateTrigger::Down;
    out.rightAnchor=read("candidates","anchor","right")=="right";
    out.ranking=read("engine","ranking","chat")=="formal"?1:0;
    auto escape=read("input","escape","staged");out.escape=escape=="clear"?EscapeAction::Clear:escape=="ignore"?EscapeAction::Ignore:EscapeAction::Staged;
    out.model=5;
    out.shiftSide=number("shortcuts","shift_side",0);out.keyboard=number("input","keyboard",0);
    out.theme=number("appearance","theme",0);out.fontSize=number("appearance","font_size",18);
    out.dictionary=shanjie::wide(read("input","dictionary","dictionary.tsv").c_str());
#define FLAG(field,section,key) out.field=flag(section,key,out.field)
    FLAG(shiftToggle,"shortcuts","shift_toggle");FLAG(ctrlSpaceToggle,"shortcuts","ctrl_space_toggle");
    FLAG(widthToggle,"shortcuts","shift_space_toggle");FLAG(ctrlPunctuation,"shortcuts","ctrl_punctuation");
    FLAG(prediction,"engine","prediction");FLAG(demote,"engine","demote");FLAG(useContext,"input","context");
    FLAG(initialChinese,"input","chinese");FLAG(initialFullWidth,"input","full_width");FLAG(capsLockEnglish,"input","caps_lock_english");
    FLAG(spaceAfterCommit,"input","space_after_commit");FLAG(pinyinSpaceConverts,"input","pinyin_space_converts");
    FLAG(spacePages,"candidates","space_pages");FLAG(backquoteSymbols,"shortcuts","backquote_symbols");FLAG(codeInput,"shortcuts","code_input");
    FLAG(learning,"input","learning");FLAG(associations,"input","associations");FLAG(alphanumeric,"input","alphanumeric");
    FLAG(big5,"candidates","big5");FLAG(enterCommits,"candidates","enter_commits");FLAG(upCycles,"candidates","up_cycles");
    FLAG(backspaceDeletes,"candidates","backspace_deletes");FLAG(wrapPages,"candidates","wrap_pages");FLAG(symbolKeyboard,"shortcuts","symbol_keyboard");
    FLAG(shiftPunctuation,"input","shift_punctuation");FLAG(plainPunctuation,"input","plain_punctuation");FLAG(standaloneTones,"input","standalone_tones");
    FLAG(ctrlBackslash,"shortcuts","ctrl_backslash");FLAG(forgetCandidate,"shortcuts","forget_candidate");FLAG(modeIndicator,"appearance","mode_indicator");
#undef FLAG
    sj_document_free(doc);return out;
}
