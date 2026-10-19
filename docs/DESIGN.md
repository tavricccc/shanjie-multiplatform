---
name: 善解 Windows
description: WinUI 3 原生設定與 Rust 同程序候選窗
colors:
  candidate-selected-background: "#006EE6"
  candidate-selected-foreground: "#FFFFFF"
  candidate-background: "#FFFFFF"
  candidate-border: "#D3D3D8"
  candidate-foreground: "#222225"
  candidate-label: "#65656E"
  candidate-dark-background: "#202020"
  candidate-dark-border: "#505055"
  candidate-dark-foreground: "#F5F5F7"
  candidate-dark-label: "#B8B8C2"
typography:
  chinese:
    fontFamily: Microsoft JhengHei UI
  candidate:
    fontFamily: Microsoft JhengHei UI
    fontSize: 16 DIP
    fontWeight: 400
  candidate-label:
    fontFamily: Segoe UI
    fontSize: 9 DIP
    fontWeight: 400
spacing:
  form: 12 DIP
  section: 12 DIP
  content: 20 DIP
  candidate-surface: 3 DIP
  candidate-item-gap: 2 DIP
  candidate-leading: 4 DIP
  candidate-label-gap: 6 DIP
  candidate-trailing: 7 DIP
  candidate-caret-gap: 6 DIP
  mode-caret-gap: 6 DIP
components:
  candidate-surface:
    backgroundColor: "{colors.candidate-background}"
    textColor: "{colors.candidate-foreground}"
    padding: "{spacing.candidate-surface}"
  candidate-item:
    typography: "{typography.candidate}"
    padding: 0 DIP 7 DIP 0 DIP 4 DIP
  candidate-item-selected:
    backgroundColor: "{colors.candidate-selected-background}"
    textColor: "{colors.candidate-selected-foreground}"
    typography: "{typography.candidate}"
    padding: 0 DIP 7 DIP 0 DIP 4 DIP
  candidate-surface-dark:
    backgroundColor: "{colors.candidate-dark-background}"
    textColor: "{colors.candidate-dark-foreground}"
    padding: "{spacing.candidate-surface}"
  mixed-reading-hint:
    backgroundColor: "{colors.candidate-selected-background}"
    textColor: "{colors.candidate-selected-foreground}"
    typography: "{typography.candidate}"
  mode-indicator:
    width: 28 DIP
    height: 28 DIP
    padding: 0 DIP
---


# Design System: 善解 Windows

## Overview

**Creative North Star: "原生設定，善解膠囊候選"**

設定程式沿用 WinUI 3 原生控制項與系統主題。候選窗依上游 macOS 版（`CandidateCells.swift`、`InputController.swift`）的膠囊比例：不透明底、細邊線、小序號、藍色選取。中英混打在正文保留原始按鍵，下方以藍底白字提示注音。目標是好讀、低資源、不打斷文件焦點。架構與尺寸規格見 [ARCHITECTURE.md](ARCHITECTURE.md)。

## Colors

候選窗的底、字、序號、選取全部不透明（alpha 1）。設定程式用 Windows 原生主題資源。

- **選取藍**：`candidate-selected-background` 配 `candidate-selected-foreground`，用於目前候選與混打注音提示。
- **淺色**：`candidate-background`、`candidate-border`、`candidate-foreground`、`candidate-label`。
- **深色**：`candidate-dark-*` 四色；選取藍與白字共用。
- **設定程式**：`SolidBackgroundFillColorBaseBrush`、`LayerFillColorDefaultBrush`、`TextFillColorSecondaryBrush` 等 ThemeResource；AccentButtonStyle 與 NavigationView 保留系統 accent。

**The Candidate Palette Rule.** 「介面與選字框主題」對應 `appearance.theme`：0 跟隨 Windows（讀 `AppsUseLightTheme`），1 淺色，2 深色，同時套用到設定程式、候選窗與加詞浮層。高對比時整組改讀 `GetSysColor` 的 `COLOR_WINDOW`、`COLOR_WINDOWTEXT`、`COLOR_HIGHLIGHT`、`COLOR_HIGHLIGHTTEXT`，邊線用 window text 色。

## Typography

設定程式用 Microsoft JhengHei UI，標題用 TitleTextBlockStyle／SubtitleTextBlockStyle。候選字詞用 `candidate`，序號與聯想的 Shift 標記用 `candidate-label`，都是正常字重。

候選字級可調 14–32 DIP；序號為字級 × 9／16，最小 8 DIP，其他內距與幾何按字級／16 縮放。

**The Measured Text Rule.** 寬度、列高與垂直置中都取自 DirectWrite 的實際 layout metrics，量測與繪製用同一字體。字詞以 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT` 繪製，系統字型支援的 emoji 可彩色顯示。

## Layout

設定程式初始 840×680 DIP，NavigationView 展開寬度 208 DIP，內容 Padding 20,16,20,20、MaxWidth 800、表單間距 12 DIP。內容可捲動，InfoBar 與儲存列固定在內容外。開關為同列左標籤、右 ToggleSwitch，MinHeight 40 DIP，並提供 AutomationProperties.Name。

候選窗每頁最多九項，可選橫向或直式。預設字級下選取膠囊 24 DIP、外框至少 30 DIP，字體需要時加高。被動預測不預留序號空間。候選與 caret 間距為 `candidate-caret-gap`；下方空間不足時改放上方，位置限制在目前螢幕工作區內。

**The Natural Width Rule.** 橫向排列時字詞依自然寬度量測，窄工作區只顯示放得下的項目，兩端箭頭翻動，目前選取項保持可見。只有單項超過可用寬度才用 character ellipsis 省略。

中英模式提示用 `mode-indicator`，透明視窗、零內距，與 caret 間距為 `mode-caret-gap`，依所在螢幕 DPI 繪製，1400 ms、下一次輸入或失焦時隱藏。

## Elevation & Depth

設定程式用 MicaBackdrop 分層。候選窗是單一 Win32 layered popup，以 Direct2D software DC render target 繪製，`UpdateLayeredWindow` 用 premultiplied alpha 合成；alpha 只處理外側圓角，沒有 blur、陰影或背景視窗。

**The Quiet Candidate Rule.** 顯示、隱藏、翻動與選取都立即切換，不用 DWM 轉場，視窗保持 `WS_EX_NOACTIVATE`。隱藏時收到主題變更只更新 palette，不自行重開候選。

## Shapes

設定控制項沿用 Windows App SDK 預設圓角與互動狀態。候選底面圓角半徑為表面高度的一半；邊線 0.5 DIP、內縮 0.25 DIP，半徑為（表面高度 − 0.5）／2。選取膠囊半徑為列高的一半，反白涵蓋整項，不用左側 selection indicator。

## Components

### 設定與保存

九頁共用一份草稿，按「儲存」才套用並通知已啟用的實例。開關共用 `SettingToggle`；ComboBox、NumberBox、TextBox 都對應真實設定；InfoBar 顯示儲存結果與格式錯誤，清除學習用 ContentDialog。操作預設只改按鍵、選字與預測，不動詞庫、學習、外觀與模型。「重新啟動輸入法」先保存草稿，再通知各實例提交組字並重載核心，不終止 App。來源：`ui/MainWindow.xaml`、`shared/src/profile.rs`。

### 候選與預測

一般項用 `candidate-item`，目前選取用 `candidate-item-selected`，表面依主題用 `candidate-surface`／`candidate-surface-dark`。被動預測沒有序號與反白，Tab 進入後才出現，Shift＋Tab 或 Esc 退出。聯想候選顯示 Shift＋序號。滑鼠點擊帶原候選索引與 sequence，TSF 拒絕過期點擊。來源：`windows/src/candidate.rs`、`windows/src/tsf/`。

### 混打注音提示

Windows 注音順序混打時，原始按鍵（如 `gj`）留在組字區，`mixed-reading-hint` 顯示 `ㄕㄨ`：同一個候選視窗與尺寸，藍底白字，沒有序號，不能點選。聲調完成後轉中文，Enter 保留原文。來源：`platform/smart_input_windows.rs`。

### 圖示與中英模式提示

輸入法本身用單色「解」（`assets/mode/shanjie.ico`，DLL 資源 100）；工作列模式按鈕用「中」（101）與「A」（102）。游標旁提示以 tiny-skia 縮放同組 128 px 母圖。圖示由 `scripts/make-icons.ps1` 產生，見 `assets/mode/README.md`。

### 解除安裝詢問

原生 MessageBox：「是」刪除全部善解資料，「否」移除程式與設定、保留詞庫與學習，「取消」不解除安裝（預設選項）。文案在 `windows/src/installer.rs`。

## Do's and Don'ts

### Do:

- **Do** 設定程式用 ThemeResource，保留原生控制項狀態、共用草稿與固定儲存列。
- **Do** 候選窗與設定程式共用主題偏好，保持不透明底與 0.5 DIP 細邊線；高對比改讀系統色。
- **Do** 候選窗維持單一 NOACTIVATE HWND，以 sequence 驗證滑鼠選取。

### Don't:

- **Don't** 在候選窗加入毛玻璃、半透明正文、背景 HWND、陰影、動畫、WebView 或子程序。
- **Don't** 恢復九欄格狀排列，或提供任意詞類表選單。
- **Don't** 把離屏 render 或背景擷取說成前景 App、高對比、多螢幕、讀屏或實際安裝的驗收。

介面沒有使用生成影像；素材來源見 `ui/Assets/PROVENANCE.md` 與 `assets/mode/README.md`。
