#pragma once
#include "engine.h"
#include "preferences.h"
class CandidateHost {
    void* handle_ = nullptr;
public:
    ~CandidateHost(); bool start(HWND);
    void publish(const shanjie::Snapshot&,const Preferences&,const RECT&,UINT,bool);
    void vertical(bool); void hide(); LONG sequence() const; bool shown() const; HWND window() const;
};
