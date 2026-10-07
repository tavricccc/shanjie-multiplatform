#pragma once
#include <windows.h>
#include <d2d1.h>
#include <dwrite.h>
#include <wrl/client.h>
#include "engine.h"
#include "preferences.h"
#include "candidate_palette.h"
class CandidateHost {
    HWND window_ = nullptr, callback_ = nullptr;
    LONG sequence_ = 0;
    shanjie::Snapshot state_;
    Preferences preferences_;
    float scale_ = 1, cell_ = 0, row_ = 0;
    unsigned columns_ = 1;
    float textWidth_ = 0, textHeight_ = 0;
    bool paletteReady_ = false;
    CandidatePalette palette_{};
    Microsoft::WRL::ComPtr<ID2D1Factory> factory_;
    Microsoft::WRL::ComPtr<IDWriteFactory> textFactory_;
    Microsoft::WRL::ComPtr<ID2D1DCRenderTarget> target_;
    Microsoft::WRL::ComPtr<ID2D1SolidColorBrush> brush_;
    std::vector<Microsoft::WRL::ComPtr<IDWriteTextLayout>> words_, labels_;
    Microsoft::WRL::ComPtr<IDWriteInlineObject> ellipsis_;
    static LRESULT CALLBACK procedure(HWND,UINT,WPARAM,LPARAM);
    bool measure(float);
    void fit(float);
    HRESULT paint(HDC);
public:
    ~CandidateHost();
    bool start(HWND);
    void publish(const shanjie::Snapshot&, const Preferences&, const RECT&, UINT, bool);
    void hide();
    LONG sequence() const { return sequence_; }
    bool shown() const { return window_ && IsWindowVisible(window_); }
    HWND window() const { return window_; }
};
