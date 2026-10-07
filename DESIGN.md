---
name: 善解 Windows
description: WinUI 3 原生設定與同程序 Win32 注音候選
colors:
  candidate-light-background: "#F5F6F7"
  candidate-light-border: "#D8DADD"
  candidate-light-foreground: "#24272B"
  candidate-dark-background: "#24272B"
  candidate-dark-border: "#4A4F55"
  candidate-dark-foreground: "#F5F6F7"
typography:
  chinese:
    fontFamily: Microsoft JhengHei UI
  candidate:
    fontFamily: Microsoft JhengHei UI
    fontSize: 18 DIP
  candidate-label:
    fontFamily: Segoe UI
    fontSize: 13.333 DIP
rounded:
  candidate-surface: 6 DIP
  candidate-selected: 4 DIP
spacing:
  form: 12 DIP
  section: 12 DIP
  content: 20 DIP
  candidate-surface: 6 DIP 8 DIP
  candidate-item: 4 DIP
  candidate-label-gap: 4 DIP
components:
  candidate-surface-light:
    backgroundColor: "{colors.candidate-light-background}"
    textColor: "{colors.candidate-light-foreground}"
    rounded: "{rounded.candidate-surface}"
    padding: "{spacing.candidate-surface}"
  candidate-surface-dark:
    backgroundColor: "{colors.candidate-dark-background}"
    textColor: "{colors.candidate-dark-foreground}"
    rounded: "{rounded.candidate-surface}"
    padding: "{spacing.candidate-surface}"
  candidate-item:
    typography: "{typography.candidate}"
    rounded: "{rounded.candidate-selected}"
    padding: "{spacing.candidate-item}"
---

# Design System: 善解 Windows

## Overview

使用者指定 Windows 原生風格；設定保留 WinUI 3，候選依已核准的實作變更改為 TSF 同程序的 Win32 popup。兩者採 Operate 模式，以可辨讀、系統一致與文件輸入連續性為主。候選方向契約見 docs/candidate-surface.md；既有外觀 tokens 保留。

## Colors

設定色彩以 XAML ThemeResource 為來源：SolidBackgroundFillColorBaseBrush、LayerFillColorDefaultBrush、TextFillColorSecondaryBrush。AccentButtonStyle 與 NavigationView 使用系統 accent，不固定 RGB。淺／深色切換與系統主題由原生資源解析，高對比尚未實際驗收。

候選中性色依使用者指定的 `local-ime-reranker-public/rime/weasel.custom.yaml` 外觀，使用 frontmatter 的淺／深底色、邊框與文字色。選取反白跟隨 Windows `UISettings`：淺色用 `AccentDark1` 配白字，深色用 `AccentLight2` 配黑字；選取後的 pointer-over／pressed 共用該背景。高對比透過 `GetSysColor` 讀取 `COLOR_WINDOW`、`COLOR_WINDOWTEXT`、`COLOR_HIGHLIGHT` 與 `COLOR_HIGHLIGHTTEXT`，不固定 accent RGB；目前僅核對來源，尚未實際高對比驗收。

## Typography

繁體中文 TextBlock 使用 Microsoft JhengHei UI；標題採 TitleTextBlockStyle／SubtitleTextBlockStyle，其餘採原生控制項字體階層。候選字級可調 14–32 DIP，預設 18 DIP。

候選文字使用 `candidate` 字體角色，實際字級仍由個人設定控制；數字與聯想 Shift 標記使用 `candidate-label`。DirectWrite 以實際 text layout metrics 決定列高、欄寬與垂直置中，量測與繪製共用字體。

## Layout

設定初始 840×680 DIP，NavigationView 展開寬度 208 DIP，內容 Padding 20,16,20,20、MaxWidth 800、表單間距 12 DIP。內容可捲動；InfoBar 和儲存／取消列固定在內容外。候選與游標相隔 4 DIP，外框內距、候選內距及數字與字詞間距依 frontmatter；列高至少 32 DIP，依實際字體測量增高。保留直列、橫列與九欄格狀：直列最低寬 96 DIP、橫列／格狀每格最低 52 DIP，長詞依實際文字測量加寬，受工作區限制。候選沒有底部注音按鈕或頁次提示，底部不足時向上放。

窄工作區會縮減每格寬度，每格各自 clip，來源設定 DirectWrite character ellipsis；本次 720 px captures 只證明裁切不跨格，未驗證省略號可見。空間足夠時小字級長詞完整呈現。游標旁中英切換提示為 40 DIP popup、內距 6 DIP，1400 ms 後隱藏，也於下一次輸入或失焦時隱藏。

開關採同列左標籤、右原生 ToggleSwitch，MinHeight 40 DIP，標籤可換行並提供 AutomationProperties.Name。參考微軟[官方候選圖片](https://support.microsoft.com/en-us/windows/media/conversion-candidate-window-1.png)的緊密數字與字詞列；圖片是視覺參考，32 DIP 等數值是本專案設計決策，並非微軟公布規格。

## Elevation & Depth

設定使用 MicaBackdrop 與系統底色／操作列分層。候選為無標題 Win32 工具視窗，在 TSF 程序內以 Direct2D software DC render target 與 DirectWrite 繪製，沒有獨立候選 host 或 IPC。

候選只由 Direct2D 提供單一不透明中性色外框，不加陰影；`WS_POPUP` 不包含 caption／resize frame，整個視窗作為 client content，避免再包一層原生框。選取面由系統 accent 與外框底色形成層次。

## Shapes

原生控制項採 Windows App SDK 預設圓角、focus、hover、pressed 行為，不另外發明網站樣式。

候選外框與選取面的圓角依 frontmatter，外框線寬為 1 DIP。Direct2D rounded rectangle 提供整塊反白，不包含左側 selection indicator；HWND region 依外框圓角裁切。

## Components

NavigationView 的一般、按鍵、選字、詞庫與學習、關於頁共用草稿。ComboBox、ToggleSwitch、NumberBox、TextBox 與固定按鈕對應真實設定。InfoBar 展示儲存與格式錯誤；清除學習採 ContentDialog。候選由同程序 Win32 popup 直接繪製，每格共用 frontmatter 內距，沒有 XAML item container 疊加留白。

一般頁的「語言模型」沿用全寬原生 ComboBox，提供 `model-v1（原版）` 與 `model-v2（上游新版）`，預設 v1，不自動改用另一版。其下以可換行的次要文字說明兩版均在本機、儲存後先提交目前組字再切換，以及保留個人詞庫與選字學習；模型來源與 hash 以 `MODEL-SOURCES.json` 為準。變更仍在共用草稿內，按儲存才套用。

詞庫與學習頁保留聯想開關，名稱及 AutomationProperties.Name 都為「預測尚未輸入的後續詞」，沿用左標籤、右原生 ToggleSwitch。次要文字明確區分關閉時只顯示已輸入讀音的候選字、開啟時數字或滑鼠選字後提示聯想詞；聯想可用 Shift＋數字或滑鼠選取，Enter 提交並關閉候選。學習偏好開關、清除學習與個人詞庫輸入維持原有原生控制項。

候選外框使用 `candidate-surface-light`／`candidate-surface-dark`；每個數字與字詞列共用 `candidate-item`。顯示、隱藏與選取立即切換，DWM 視窗轉場停用，保持 `WS_EX_NOACTIVATE`；滑鼠選字帶入 sequence 供 TSF 核對。反白與高對比契約以 `src/candidate_palette.cpp` 為準，視窗、量測、每格裁切、繪製與定位以 `src/candidate_host.h`／`.cpp` 為準。這是原生桌面元件，sidecar 記錄資源與行為，不提供 HTML/CSS 模擬。

工作列模式按鈕與游標旁提示共用彩色「中」／「A」ICO，直接引用使用者指定參考專案的 `rime/icons/chinese.ico`／`english.ico`，嵌入 DLL resource 101／102。來源與資產契約見 `assets/mode/README.md`；行為見 `src/mode_button.cpp` 與 `src/mode_indicator.cpp`。提示位於同一 TSF 程序，沒有額外 host。

## Do's and Don'ts

- 設定使用 ThemeResource，保留鍵盤與滑鼠的原生控制項狀態；候選讀取 UISettings accent 與系統高對比色。
- 候選維持 NOACTIVATE，以 sequence 驗證滑鼠提交。
- 候選只保留單一外框，且顯示、隱藏與選取不加動畫；此契約不改變設定窗原生控制項行為。
- Windows 的橫列和直列都用 ↑／↓ 選字；善解操作說明隨 profile 更新。
- 設定保留 WinUI 3 原生控制項；候選與模式提示依已核准契約使用 Win32，不新增 WebView。
- 不把離屏 render 宣稱為讀屏、多 DPI 或真實文件驗收。

既有 WinUI 設定審查：獨立 reviewer 確認固定回饋、profile 說明及來源界線已解決；確認圖另發現預設回饋沿用錯誤 severity，已重設為 Informational，最後限定確認 verdict 為 accept／ship。驗證圖片均為本機真實 XAML render；範本 placeholder 來源記錄在 ui/Assets/PROVENANCE.md，介面未使用生成影像。

原 WinUI 候選外觀證據為 `build/ui-review-final/candidates-1-0.png`（淺色直列）與 `candidates-2-2.png`（深色九欄格狀）。它們是舊實作的背景離屏 XAML render，保留作外觀參考，不代表目前 Win32 候選驗收。設定窗本次維持原樣。

目前 Win32 候選與模式提示的背景原生 capture 分別在 `build/native-review/x64`／`x86`（BMP）與 `build/mode-review`；這些是原生 HWND 繪製，無 HTML detector。來源可核對單框、無動畫、NOACTIVATE 與每格裁切；靜態圖片無法證明轉場或實際提交。窄工作區長詞修正已重擷取，24 張一般與 4 張 720 px captures 有效；獨立 reviewer 確認裁切不跨格，兩項修正完成，限於本次 fix list。高對比、實際深色工作列、前景應用程式與多螢幕尚未驗收。

後續模型與聯想設定的外觀證據為 `build/ui-review-final/settings-1-models.png`／`settings-2-models.png` 與 `settings-1-3.png`／`settings-2-3.png`，分別呈現淺／深色的一般頁模型控制項與詞庫學習開關。圖片為背景 XAML render；文案與資料繫結另核對 `ui/MainWindow.xaml`、同名 `.xaml.cs` 與 `ui/ViewModels/SettingsViewModel.cs`。此輪沿用既有設定控制項，候選 tokens、外觀與內距維持原樣；背景證據不等同前景應用程式中的實際提交與模型切換驗收。
