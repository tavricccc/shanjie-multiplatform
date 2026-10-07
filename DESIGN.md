---
name: 善解 Windows
description: WinUI 3 原生設定與不搶焦點的注音候選
typography:
  chinese:
    fontFamily: Microsoft JhengHei UI
spacing:
  form: 20 DIP
  section: 24 DIP
  content: 32 DIP
---

# Design System: 善解 Windows

## Overview

使用者指定 Windows 原生 WinUI 3。設定與候選採 Operate 模式，以可辨讀、系統一致與文件輸入連續性為主。方向契約見 WINUI-MIGRATION.md。

## Colors

色彩以 XAML ThemeResource 為來源：SolidBackgroundFillColorBaseBrush、LayerFillColorDefaultBrush、TextFillColorSecondaryBrush。AccentButtonStyle 與 NavigationView 使用系統 accent，不固定 RGB。淺／深色切換與系統主題由原生資源解析，高對比尚未實際驗收。

## Typography

繁體中文 TextBlock 使用 Microsoft JhengHei UI；標題採 TitleTextBlockStyle／SubtitleTextBlockStyle，其餘採原生控制項字體階層。候選字級可調 14–32 DIP，預設 18 DIP。

## Layout

設定初始 960×760 DIP，NavigationView 展開寬度 208 DIP，內容 Padding 32,24,32,32、MaxWidth 800、表單間距 20 DIP。內容可捲動；InfoBar 和儲存／取消列固定在內容外。候選大小由文字與工作區計算，顯示位置遵循 caret，底部不足時往上放。

## Elevation & Depth

設定使用 MicaBackdrop 與系統底色／操作列分層。候選為無標題工具視窗，使用原生 WinUI brush／border，不自行繪製陰影或 GDI 表面。

## Shapes

原生控制項採 Windows App SDK 預設圓角、focus、hover、pressed 行為，不另外發明網站樣式。

## Components

NavigationView 的一般、按鍵、選字、詞庫與學習、關於頁共用草稿。ComboBox、ToggleSwitch、NumberBox、TextBox 與固定按鈕對應真實設定。InfoBar 展示儲存與格式錯誤；清除學習採 ContentDialog。候選使用 ListView／GridView，原生 Button 提供注音原文。

## Do's and Don'ts

- 使用 ThemeResource，保留鍵盤與滑鼠的原生控制項狀態。
- 候選維持 NOACTIVATE，以 sequence 驗證滑鼠提交。
- Windows 的橫列和直列都用 ↑／↓ 選字；善解操作說明隨 profile 更新。
- 不將控制項以手繪 Win32／GDI 或 WebView 外觀替代。
- 不把離屏 render 宣稱為讀屏、多 DPI 或真實文件驗收。

審查：獨立 reviewer 確認固定回饋、profile 說明及來源界線已解決；確認圖另發現預設回饋沿用錯誤 severity，已重設為 Informational，最後限定確認 verdict 為 accept／ship。驗證圖片均為本機真實 XAML render；範本 placeholder 來源記錄在 ui/Assets/PROVENANCE.md，介面未使用生成影像。
