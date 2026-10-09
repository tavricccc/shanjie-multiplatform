---
name: 善解 Windows
description: WinUI 3 原生設定與 Rust 同程序橫向膠囊候選
colors:
  candidate-selected-background: "#006EE6"
  candidate-selected-foreground: "#FFFFFF"
  candidate-background: "#FFFFFF"
  candidate-border: "#D3D3D8"
  candidate-foreground: "#222225"
  candidate-label: "#65656E"
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
  mode-indicator:
    width: 28 DIP
    height: 28 DIP
    padding: 0 DIP
---

# Design System: 善解 Windows

## Overview

**Creative North Star: "原生設定，善解橫向膠囊候選"**

設定維持使用者核准的 WinUI 3 原生幾何與操作；候選沿用使用者提供的 macOS 善解照片與上游 `CandidateCells.swift`／`InputController.swift` 的橫向膠囊比例，依最新指定採純白不透明背景、很細的邊線、小型序號與藍色選取面。兩者服務日常輸入，以辨讀、低資源與不中斷文件焦點為主。候選方向契約見 `docs/candidate-surface.md`。

Windows 產品邏輯由 Rust 實作，包含 TSF、候選、模式提示、設定保存、安裝與解除安裝。C#／XAML 保留 WinUI 3 控制項與繫結；C++ 僅在 tests/support 供測試使用，CMD／PowerShell 為薄啟動入口。上游引擎仍由固定子模組與平台覆蓋提供，不將此架構變更描述為上游演算法重寫。

**Key Characteristics:**

- WinUI 3 原生設定、固定儲存回饋與共用草稿。
- 候選只保留橫向膠囊，字詞按自然寬度排列。
- 候選與中英提示均不取得焦點。
- 使用實際原生繪製證據，分清背景驗證與實機驗收。

## Colors

一般候選固定為純白不透明背景、淺灰細邊線與選取藍，背景、文字、序號及選取面均為 alpha 1。此配色以使用者本輪明確指定為準，不跟隨設定介面深色主題。設定窗繼續使用 Windows 原生主題資源。

### Primary

- **選取藍**：`candidate-selected-background` 只用於目前候選的膠囊反白；序號與字詞使用 `candidate-selected-foreground`。

### Neutral

- **純白候選底面與細邊線**：`candidate-background` 與 `candidate-border` 構成單一清楚的膠囊邊界，不加入陰影或背景材質。
- **候選字詞與序號**：`candidate-foreground` 提供主要閱讀對比；`candidate-label` 使用次要灰色，配合較小字級區分序號。
- **設定主題**：以 `SolidBackgroundFillColorBaseBrush`、`LayerFillColorDefaultBrush`、`TextFillColorSecondaryBrush` 等 XAML ThemeResource 為來源。設定的 AccentButtonStyle 與 NavigationView 保留系統 accent。

**The Candidate Palette Rule.** 一般候選固定使用 frontmatter 的白底、細邊線與選取藍，忽略 appearance.theme；「設定介面主題」只控制 WinUI 設定窗。高對比整組改讀 `GetSysColor` 的 `COLOR_WINDOW`、`COLOR_WINDOWTEXT`、`COLOR_HIGHLIGHT` 與 `COLOR_HIGHLIGHTTEXT`，邊線也使用 window text 色。高對比目前僅核對來源，尚未實機驗收。

## Typography

繁體中文設定 TextBlock 使用 Microsoft JhengHei UI；標題採 TitleTextBlockStyle／SubtitleTextBlockStyle，其餘使用原生控制項字體階層。候選字詞使用 `candidate`、數字與聯想 Shift 標記使用 `candidate-label`，兩者皆為正常字重。

新安裝候選字級為 frontmatter 的預設，個人設定可調 14–32 DIP。序號字級為候選字級乘 9／16，最低 8 DIP；其他候選內距與幾何按字級／16 的比例調整。舊 schema 升至 4 時保留已存字級；若舊設定沒有字級欄位，遷移會寫入舊預設 18 DIP，以保留原本外觀。

**The Measured Text Rule.** DirectWrite 的實際 layout metrics 決定字詞寬度、列高與垂直置中，量測與繪製共用字體。字詞 DrawTextLayout 啟用 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`；Windows 已安裝字型支援的 emoji、膚色與 ZWJ 序列可彩色呈現，旗幟字形依作業系統，不宣稱所有 emoji 均支援。

## Layout

設定初始 840×680 DIP，NavigationView 展開寬度 208 DIP，內容 Padding 20,16,20,20、MaxWidth 800、表單間距 12 DIP。內容可捲動；InfoBar 和儲存／取消列固定在內容外。開關採同列左標籤、右原生 ToggleSwitch，MinHeight 40 DIP，標籤可換行並提供 AutomationProperties.Name。

候選只有單行橫向排列，每頁最多九項。以預設字級計，選取膠囊基準高 24 DIP，外框最小高 30 DIP；字體 metrics 需要時增加列高。外框內距、項間距、序號前距、序號與字詞間距、尾端留白均以 frontmatter 為準；被動預測不預留序號空間。候選與 caret 的間距固定為 `candidate-caret-gap`，依 DPI 換算；底部不足時改放上方，位置夾在目前螢幕工作區內。沒有底部注音按鈕或頁次提示。

**The Natural Width Rule.** 字詞依自然寬度量測，窄工作區只呈現放得下的項目，兩端箭頭翻動可見範圍；鍵盤目前選取項始終保持可見。不把所有詞一起縮窄，只有單項超過可用寬度時才以 DirectWrite character ellipsis 省略。

游標旁中英提示使用 `mode-indicator`：透明視窗與零內距，caret 間距依 `mode-caret-gap`；在所在螢幕 DPI 重新繪製，1400 ms 後、下一次輸入或失焦時隱藏。

## Elevation & Depth

設定使用 MicaBackdrop 與系統底色／操作列分層。候選只使用 TSF 同程序的單一 Win32 layered popup，由 Rust Direct2D software DC render target／DirectWrite 繪製；沒有 owner／背景 HWND、候選子程序或 IPC。

候選正文純白且不透明，layered alpha 只處理膠囊外側透明圓角與抗鋸齒。單一細邊線提供表面邊界，選取膠囊維持不透明藍色；沒有 blur、陰影或額外背景視窗。`WS_POPUP` 沒有 caption 或 resize frame，`UpdateLayeredWindow` 以 premultiplied alpha 合成。已移除毛玻璃模組、私有 composition API 與相應 feature dependencies，不保留替代材質或 renderer。

**The Quiet Candidate Rule.** 顯示、隱藏、可見範圍移動與選取立即切換，DWM 視窗轉場停用，單一候選 HWND 保持 `WS_EX_NOACTIVATE`。隱藏時的設定／主題／DWM 色彩通知只更新 palette；已顯示時才重新 display，不自行重開候選。這項候選規則不更改設定窗原生控制項動畫。

## Shapes

設定控制項採 Windows App SDK 預設圓角、focus、hover 與 pressed 行為。候選白底 FillRoundedRectangle 從 0 到整個視窗邊界，圓角半徑為實際表面高度的一半；邊線寬 0.5 DIP，stroke rectangle 內縮 0.25 DIP，半徑為（表面高度 − 0.5 DIP）／2。選取膠囊半徑為實際列高的一半。圓角依實際量測高度產生，字級、內距與列高維持既有測量契約；反白涵蓋整項，不使用左側 selection indicator。

## Components

### 原生設定與保存

NavigationView 的「一般」、「按鍵」、「組字與提交」、「選字與外觀」、「預測、詞庫與學習」、「關於與相容性」六頁共用草稿。開關共用 `SettingToggle`，ComboBox、NumberBox、TextBox 與固定按鈕對應真實設定；InfoBar 展示儲存與格式錯誤，清除學習使用 ContentDialog。設定保存與遷移來源為 `windows/src/profile.rs`，WinUI 3 控制項來源為 `ui/MainWindow.xaml`；按儲存後才套用並通知已啟用實例。

一般頁以文字顯示固定 `model-v4＋classes-v2`，只提供最新模型與配對詞類表，沒有模型選單或任意配表控制項。模型在本機執行，個人詞庫與選字學習保留；來源與 hash 以 `MODEL-SOURCES.json` 為準。操作預設在共用草稿內只調整按鍵、選字與預測，保留詞句偏好、詞庫、學習與外觀。設定保存採 schema 4；舊設定先備份再遷移至最新模型及橫向候選，保留個人資料與其餘已存偏好，移除已退役的候選展開設定。

預測頁保留「未完成讀音的即時預測」與「選字後的聯想詞」兩個獨立開關。Windows 操作預設兩者皆關閉；載入操作預設仍須儲存才套用。前文、敏感詞降權、學習偏好、忘記目前候選學習、清除學習與個人詞庫輸入維持原生控制項。

### 橫向候選與預測

候選表面使用 `candidate-surface`，一般項與目前選取項分別使用 `candidate-item`／`candidate-item-selected`。左右、上下與 Tab 均沿同一橫向候選序列操作，不展開直向或格狀；profile 的其他提交、空白翻頁與取消規則仍由個人設定控制。

被動即時預測不顯示序號、反白或序號預留欄；Tab 進入後才出現序號與目前反白，Shift＋Tab 或 Esc 退出。聯想候選顯示 Shift＋序號，以 Shift＋數字或滑鼠選取。滑鼠回傳原候選索引與 sequence，TSF 拒絕過期點擊，箭頭僅翻動可見範圍。palette、量測、自然寬度、alpha、繪製與定位來源為 `windows/src/candidate.rs`；候選 UIElement、焦點與提交契約為 `windows/src/tsf/`。`windows/src/lib.rs` 與 `windows/Cargo.toml` 不再載入毛玻璃模組或 UI_ViewManagement／UI_Controls features。sidecar 記錄原生資源與行為，不提供 HTML/CSS 模擬。

### 中英模式提示

工作列模式按鈕保留 `rime/icons/chinese.ico`／`english.ico`，嵌入 DLL resource 101／102。游標旁提示由 Rust tiny-skia 依 `assets/mode/chinese.svg`／`english.svg` 的既有 24×24 幾何繪製中／A，不縮放 ICO；原始 MIT 授權保留於 `assets/mode/LICENSE`。`windows/src/native.rs` 使用 layered alpha、NOACTIVATE 與原生計時器；模式按鈕來源為 `windows/src/tsf/mode.rs`，兩者均在 TSF 同程序內。

### 原生解除安裝詢問

Rust 安裝器更新時清除舊版本、舊模型與退役 payload，鎖定檔案可延至重開機刪除。原生 MessageBox 的「是」完整刪除所有善解資料；「否」移除程式與設定，保留詞庫和學習供重新安裝使用；「取消」不進行解除安裝，且為預設選項。產品文案與處理來源為 `windows/src/installer.rs`，不在啟動腳本或 WinUI 繫結層複製這套邏輯。

## Do's and Don'ts

### Do:

- **Do** 設定使用 ThemeResource，保留原生控制項狀態、既有幾何、六頁共用草稿與固定儲存回饋。
- **Do** 候選沿單行自然寬度排列，窄工作區以箭頭移動可見範圍，只省略超長單項。
- **Do** 一般候選固定純白不透明底與 0.5 DIP 細邊線，設定介面主題獨立控制 WinUI；高對比改讀系統色。
- **Do** 單一候選 HWND 維持 NOACTIVATE，以 sequence 驗證滑鼠提交，保留 hide／reopen 與 hidden theme 通知規則。

### Don't:

- **Don't** 恢復直向、九欄格狀、模型版本選單或任意詞類表選單。
- **Don't** 在候選恢復毛玻璃、半透明正文、額外背景 HWND、陰影、動畫、WebView 或候選子程序。
- **Don't** 將歷史截圖、離屏 render 或來源檢查宣稱為前景 App、工作列、高對比、多螢幕、讀屏、實際安裝或重開機驗收。

候選證據路徑：`.impeccable/review/{x64,x86}/` 各 18 張，包含 `candidates-{1,2}-{horizontal,horizontal-14,horizontal-32,narrow,associated,emoji}.png`、`prediction-{1,2}-{passive,entered}.png` 及 `reference-{1,2}-conversion.png`。檔名的 1／2 保留兩種設定主題測試案例，兩者候選都應是白底；這些是同一 Windows 120 DPI 背景原生 WM_PRINTCLIENT 結果，premultiplied BGRA 轉 straight alpha 後保存 RGBA PNG，不是 AI 素材或 HTML detector。設定證據為 `build/ui-review-final/settings-{1,2}-{0,3,appearance}.png` 六張離屏 XAML render。本輪新鮮 reviewer 逐張核對 36 張候選與 2 張設定 appearance PNG，disposition ship，限於背景原生繪製／離屏審查，沒有擴大安裝或前景 App 驗收。

本輪 x64／x86 建置與原生候選／WinUI 背景驗證均 PASS，x64 TSF 背景檢查亦 PASS；測試保留 null owner、NOACTIVATE、過期點擊、滑鼠選字、hide／reopen 與隱藏時主題通知斷言，移除舊雙 HWND、composition policy 與 region 專屬斷言。前景 App、實際工作列、高對比、多螢幕、實際新安裝與重開機刪除仍未實機驗收。舊毛玻璃、alpha 0.82／0.55／0.74、雙視窗與其 review ship 已退役，只是歷史結果；更早舊 WinUI 直向／格狀、C++ BMP 與模型選單圖也不代表本輪驗收。介面未使用生成影像，既有素材 provenance 見 `ui/Assets/PROVENANCE.md` 與 `assets/mode/LICENSE`。
