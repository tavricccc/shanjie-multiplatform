---
name: 善解 Windows
description: WinUI 3 原生設定與不搶焦點的注音候選
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

使用者指定 Windows 原生 WinUI 3。設定與候選採 Operate 模式，以可辨讀、系統一致與文件輸入連續性為主。方向契約見 WINUI-MIGRATION.md。

## Colors

色彩以 XAML ThemeResource 為來源：SolidBackgroundFillColorBaseBrush、LayerFillColorDefaultBrush、TextFillColorSecondaryBrush。AccentButtonStyle 與 NavigationView 使用系統 accent，不固定 RGB。淺／深色切換與系統主題由原生資源解析，高對比尚未實際驗收。

候選中性色依使用者指定的 `local-ime-reranker-public/rime/weasel.custom.yaml` 外觀，使用 frontmatter 的淺／深底色、邊框與文字色。依後續指定，選取反白跟隨 Windows 設定 accent：淺色用 `SystemAccentColorDark1` 配白字，深色用 `SystemAccentColorLight2` 配黑字；選取後的 pointer-over／pressed 共用該背景。高對比改用 `SystemColorWindowColor`、`SystemColorWindowTextColor`、`SystemColorHighlightColor` 與 `SystemColorHighlightTextColor`，不固定 accent RGB。

## Typography

繁體中文 TextBlock 使用 Microsoft JhengHei UI；標題採 TitleTextBlockStyle／SubtitleTextBlockStyle，其餘採原生控制項字體階層。候選字級可調 14–32 DIP，預設 18 DIP。

候選文字使用 `candidate` 字體角色，實際字級仍由個人設定控制；數字與聯想 Shift 標記使用 `candidate-label`。視窗量測也使用 Microsoft JhengHei UI，以實際字高、字寬決定列高與欄寬。

## Layout

設定初始 840×680 DIP，NavigationView 展開寬度 208 DIP，內容 Padding 20,16,20,20、MaxWidth 800、表單間距 12 DIP。內容可捲動；InfoBar 和儲存／取消列固定在內容外。候選與游標相隔 4 DIP，外框內距、候選內距及數字與字詞間距依 frontmatter；列高至少 32 DIP，依實際字體測量增高。保留直列、橫列與九欄格狀：直列最低寬 96 DIP、橫列／格狀每格最低 52 DIP，長詞依實際文字測量加寬，受工作區限制。候選沒有底部注音按鈕或頁次提示，底部不足時向上放。

開關採同列左標籤、右原生 ToggleSwitch，MinHeight 40 DIP，標籤可換行並提供 AutomationProperties.Name。參考微軟[官方候選圖片](https://support.microsoft.com/en-us/windows/media/conversion-candidate-window-1.png)的緊密數字與字詞列；圖片是視覺參考，32 DIP 等數值是本專案設計決策，並非微軟公布規格。

## Elevation & Depth

設定使用 MicaBackdrop 與系統底色／操作列分層。候選為無標題工具視窗，使用原生 WinUI brush／border，不自行繪製陰影或 GDI 表面。

候選採單層不透明中性色外框，不加陰影；選取面由系統 accent 與外框底色形成層次。

## Shapes

原生控制項採 Windows App SDK 預設圓角、focus、hover、pressed 行為，不另外發明網站樣式。

候選外框與選取 presenter 的圓角依 frontmatter，外框線寬為 1 DIP。`ListViewItemPresenter` 明確設定選取圓角，並關閉原生左側 selection indicator，維持整塊反白。

## Components

NavigationView 的一般、按鍵、選字、詞庫與學習、關於頁共用草稿。ComboBox、ToggleSwitch、NumberBox、TextBox 與固定按鈕對應真實設定。InfoBar 展示儲存與格式錯誤；清除學習採 ContentDialog。候選使用 ListView／GridView，item container 的 padding／margin 歸零，避免原生預設再疊加一層留白。

候選 Surface 使用 `candidate-surface-light`／`candidate-surface-dark`；每個數字與字詞列共用 `candidate-item`，內距只由資料範本提供。反白、邊框及高對比資源契約以 `ui/CandidateWindow.xaml` 為準；presenter、量測與定位以同名 `.xaml.cs` 為準。這是原生 XAML 元件，sidecar 記錄資源與行為，不提供模擬 WinUI 的 HTML/CSS 範例。

## Do's and Don'ts

- 使用 ThemeResource，保留鍵盤與滑鼠的原生控制項狀態。
- 候選維持 NOACTIVATE，以 sequence 驗證滑鼠提交。
- Windows 的橫列和直列都用 ↑／↓ 選字；善解操作說明隨 profile 更新。
- 不將控制項以手繪 Win32／GDI 或 WebView 外觀替代。
- 不把離屏 render 宣稱為讀屏、多 DPI 或真實文件驗收。

審查：獨立 reviewer 確認固定回饋、profile 說明及來源界線已解決；確認圖另發現預設回饋沿用錯誤 severity，已重設為 Informational，最後限定確認 verdict 為 accept／ship。驗證圖片均為本機真實 XAML render；範本 placeholder 來源記錄在 ui/Assets/PROVENANCE.md，介面未使用生成影像。

本次候選外觀證據為 `build/ui-review-final/candidates-1-0.png`（淺色直列）與 `candidates-2-2.png`（深色九欄格狀）。圖片是背景離屏 XAML render，呈現目前主機 accent；它們驗證候選密度、字體、反白與外框，不能視為真實文件操作、多 DPI、讀屏或高對比驗收。沒有新增 raster 資產；設定窗本次維持原樣。
