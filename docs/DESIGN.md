---
name: 善解 Windows
description: WinUI 3 玻璃設定與 Rust 同程序候選窗；個人外觀可選系統／自訂強調色、深色純黑與動畫政策
colors:
  candidate-selected-background: "#5874C9"
  candidate-selected-foreground: "#000000"
  candidate-background: "#F8F8FA"
  candidate-border: "#000000"
  candidate-foreground: "#141418"
  candidate-label: "#141418"
  candidate-dark-background: "#1C1C20"
  candidate-dark-border: "#FFFFFF"
  candidate-dark-foreground: "#FFFFFF"
  candidate-dark-label: "#FFFFFF"
opacity:
  candidate-background: 0.97
  candidate-border: 0.08
  candidate-foreground: 0.92
  candidate-label: 0.62
  candidate-number: 0.42
  candidate-dark-background: 0.97
  candidate-dark-border: 0.08
  candidate-dark-foreground: 0.94
  candidate-dark-label: 0.64
  candidate-dark-number: 0.4
typography:
  chinese:
    fontFamily: Noto Sans when installed; otherwise system Traditional Chinese UI font
  candidate:
    fontFamily: Noto Sans when installed; otherwise Microsoft JhengHei UI
    fontSize: 16 DIP
    fontWeight: 400
  candidate-label:
    fontFamily: Noto Sans when installed; otherwise Segoe UI
    fontSize: 9 DIP
    fontWeight: 400
spacing:
  candidate-surface: 5 DIP
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
    backgroundOpacity: 0.14 in light theme; 0.3 in dark theme
    textColor: "{colors.candidate-selected-background}"
    typography: "{typography.candidate}"
  mode-indicator:
    width: 28 DIP
    height: 28 DIP
    padding: 0 DIP
---


# 善解 Windows 設計系統

## Overview

**設計方向：霧藍玻璃設定與輕量候選窗。**

設定、安裝與更新畫面使用 WinUI 3、自訂玻璃視窗與共用的霧藍主題；候選窗由 Rust 在宿主程序內繪製，採接近不透明的圓角底、細邊線、小序號、霧藍選取與柔和陰影。原鍵混打在 App 組字區保留原始按鍵，組字面板以霧藍字與淡色底提示注音。目標是好讀、低資源、不打斷文件焦點。架構見 [ARCHITECTURE.md](ARCHITECTURE.md)。

本文的色值與比例以目前原始碼為準：候選窗在 [`candidate.rs`](../windows/src/candidate.rs)，設定介面資源在 [`Sj.xaml`](../ui/Themes/Sj.xaml)，字型選擇在 [`font.rs`](../windows/src/font.rs)。以上藍色示例是預設外觀；可改用 Windows 強調色或自訂 HEX，選取文字自動依對比選黑或白。深色可選不透明純黑；高對比優先採系統色。動畫可跟隨系統、強制開啟或關閉。

## Colors

上方色表列 RGB，透明度另列於 `opacity`；候選窗的一般底面 alpha 為 0.97，文字、序號、邊線及提示各有透明度。設定程式使用 `Sj*` ThemeResource 與共用玻璃視窗，並保留 WinUI 控制項的互動與可及性語意。

- **霧藍**：候選預設使用霧藍底，文字依對比選黑或白；混打注音提示使用可讀的強調色與淡色底，依深淺主題調整底色透明度。
- **淺色**：`candidate-background`、`candidate-border`、`candidate-foreground`、`candidate-label`。
- **深色**：`candidate-dark-*` 四色；選取強調色與自動對比文字共用。
- **設定程式**：`SjText*Brush`、`SjStrokeBrush`、`SjFill*Brush` 等主題資源；強調色預設霧藍 `#5874C9`，可改為跟隨 Windows 或自訂 HEX，與候選窗、加詞浮層共用（`ui/AppearanceController.cs`）。

**The Candidate Palette Rule.** 「介面與選字框主題」對應 `appearance.theme`：0 跟隨 Windows（讀 `AppsUseLightTheme`），1 淺色，2 深色，同時套用到設定程式、候選窗與加詞浮層。高對比時整組改讀 `GetSysColor` 的 `COLOR_WINDOW`、`COLOR_WINDOWTEXT`、`COLOR_HIGHLIGHT`、`COLOR_HIGHLIGHTTEXT`，邊線用 window text 色。

## Typography

已安裝時優先用 Noto Sans 家族，依序尋找 TC、CJK TC、HK、CJK HK 及 Noto Sans。沒有可用家族時，WinUI 保留系統字型；候選字詞用 Microsoft JhengHei UI，序號與聯想的 Shift 標記用 Segoe UI，都是正常字重。標題與說明使用 `Sj.xaml` 的樣式。

候選字級可調 14–32 DIP；序號為字級 × 9／16，最小 8 DIP，其他內距與幾何按字級／16 縮放。

**The Measured Text Rule.** 寬度、列高與垂直置中都取自 DirectWrite 的實際 layout metrics，量測與繪製用同一字體。字詞以 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT` 繪製，系統字型支援的 emoji 可彩色顯示。

## Layout

設定程式初始 840×680 DIP，左側導覽寬 216 DIP，使用自訂導覽列與選取高亮；右側內容可捲動，狀態列與儲存列固定在內容外。設定分組用 `SettingCard`，每列用 `SettingRow`；開關為左標籤、右側狀態文字與自訂 `Switch`，並提供 AutomationProperties.Name。實際間距與最小尺寸以 `MainWindow.xaml` 及控制項實作為準。

候選窗收合時一律直式，每頁最多九項；展開時為三欄多列，依工作區可用高度捲動。預設字級下候選列至少 24 DIP，上下各留 5 DIP，只有一列候選且沒有組字面板時底面至少 34 DIP，字體需要時加高；陰影另有外側空間。被動預測不預留序號空間。候選與 caret 間距為 `candidate-caret-gap`；下方空間不足時改放上方，位置限制在目前螢幕工作區內。

**The Natural Width Rule.** 字詞依自然寬度量測，直式清單共用最寬一項的寬度，格狀清單各欄分別量測；候選的最小寬度、組字面板與工作區限制由 `candidate.rs::fit_view` 決定。只有組字面板時，寬度隨文字與內距伸縮。超過可用寬度時用 character ellipsis 省略。

中英模式提示用 `mode-indicator`，透明視窗、零內距，與 caret 間距為 `mode-caret-gap`，依所在螢幕 DPI 繪製，1400 ms、下一次輸入或失焦時隱藏。

## Elevation & Depth

設定與安裝視窗是毛玻璃風格（內容視窗＋Rust 玻璃視窗）。候選窗是單一 Win32 layered popup，以 Direct2D software DC render target 繪製，`UpdateLayeredWindow` 用 premultiplied alpha 合成；陰影與上緣亮邊畫在 bitmap 裡，沒有 blur 或背景視窗。

**The Quiet Candidate Rule.** 不用 DWM 轉場，視窗保持 `WS_EX_NOACTIVATE`。開啟時短暫淡入並從游標側輕微放大，選取高亮滑動，顯示中尺寸或位置改變時面板滑到新的大小與位置；隱藏立即切換；Windows 動畫效果關閉時全部立即切換。隱藏時收到主題變更只更新 palette，不自行重開候選。

## Shapes

設定控制項使用 `Sj.xaml` 與 `ui/Controls/` 的共用圓角、互動狀態及動畫。候選底面圓角半徑為 14 DIP × 字級／16；邊線 0.5 DIP、內縮 0.25 DIP。選取高亮涵蓋整項並在候選之間滑動，不另加左側 selection indicator。

## Components

### 設定與保存

九頁共用一份草稿，按「儲存」才套用並通知已啟用的實例。開關共用 `SettingToggle`；ComboBox、NumberBox、TextBox 都對應真實設定；InfoBar 顯示儲存結果與格式錯誤，清除學習用 ContentDialog。操作預設只改按鍵、選字與預測，不動詞庫、學習、外觀與模型。「重新啟動輸入法」先保存草稿，再通知各實例提交組字並重載核心，不終止 App。來源：`ui/MainWindow.xaml`、`shared/src/profile.rs`。

### 狀態列

設定與安裝程式的 InfoBar 共用 `ui/Themes/Sj.xaml` 的隱含樣式：卡片同款玻璃底（`SjFillControlBrush`）、12 圓角、1 點邊線，疊一層很淡的嚴重度色（`SjStatus*TintBrush`）並把邊線染成同色（`SjStatus*StrokeBrush`）。左邊是 18 點 Lucide 線條圖示，形狀分辨嚴重度：成功圓勾、資訊 i（霧藍）、警告三角、錯誤圓叉；文字 13 點 `SjText1Brush`，關閉鍵用 `SjIconButton`。深淺色各有一組較亮／較深的圖示色以維持對比；高對比時只在狀態列內改用 `SystemColorWindow*`／`SystemColorHighlight`，不加底色。出現時上移 6 點並淡入（`Motion.Normal`、Smooth），關閉時淡出（`Motion.Fast`），由 `StatusMotion` 的隱含動畫負責，Windows 動畫效果關閉時立即切換。

### 快速設定

與設定共用一張玻璃，蓋住導覽與內容，一個畫面只問一件事：歡迎 → 打字方式 → 空白鍵 → Tab／→ → 中英混打（注音時）→ 單按 Shift → 選字學習 → 組字面板 → 完成與試打。頂部是標題、進度膠囊（136×4，霧藍填色）與「n／m」、視窗按鈕；底部固定略過（純文字鈕）、上一步、繼續（主按鈕；最後一題是「套用」，完成頁是「開始使用」）。

問題頁左邊是霧藍小標、28 點標題、14 點短說明與答案卡片，右邊是 360 寬的示範與一行註記。答案卡片是設定頁同款玻璃卡（14 圓角），右側空心圓；選中的卡片疊一圈 1.5 點霧藍外框與很淡的霧藍底，勾選圓點填滿。歡迎頁置中，放大的「解」圖示後面有一層淡霧藍光暈；完成頁置中，上方是勾選徽章，下面是試打框與兩欄的「你的選擇」。文字用一般人看得懂的話，描述結果，不用設定鍵名。

動態全部是 composition 動畫（`Choreography`，同一段長度內以關鍵影格錯開）：
- **換頁**：舊頁 240 ms 往反方向滑出並淡出，新頁的各部分（小標、標題、說明、卡片、註記、示範）每 40 ms 依序從前進的方向滑入 44 點並淡入，Smooth 曲線；上一步時方向相反。進度膠囊以 Glide 曲線滑到新位置。
- **選擇**：外框從原本的卡片滑到新卡片（320 ms Glide），勾選以彈簧放大，卡片輕壓一下；示範換成新答案時整個場景淡入上移。
- **示範**：候選窗從游標側淡入放大、選取膠囊在項目間滑動、換頁與展開淡入、中／A 提示彈出、送出的字落進文字框。
- **完成**：霧藍圓環從頂端順時針畫出，內部填滿並彈起，對比色勾分兩筆畫出，徽章再輕彈一次；同時背後的光暈亮起，一圈光往外擴散淡去，十幾顆霧藍、淡藍與一點淡金的小點向外飄散消失；標題與試打區隨後依序升起。約 2 秒。
- **交回設定**：流程 280 ms 淡出並微縮，設定頁的導覽與內容依序滑入。

動畫預設跟隨 Windows，也可強制開啟或關閉；動畫政策關閉時以上全部立即到位，示範停在關鍵畫面。高對比時外框、勾選與進度改用系統醒目色，不畫光暈與小點。

示範場景裡的候選窗沿用 `candidate.rs` 的配色與比例，字級略小以放得下；格狀與直向清單會截短。組字面板的標記與真實視窗相同：微弱的藍色底，游標在句尾時不標。

### 候選與預測

一般項用 `candidate-item`，目前選取用 `candidate-item-selected`，表面依主題用 `candidate-surface`／`candidate-surface-dark`。被動預測沒有序號與反白，Tab 進入後才出現，Shift＋Tab 或 Esc 退出。聯想候選顯示 Shift＋序號。滑鼠點擊帶原候選索引與 sequence，TSF 拒絕過期點擊。來源：`windows/src/candidate.rs`、`windows/src/tsf/`。

### 混打注音提示

原鍵混打時，原始按鍵（如 `gj`）留在 App 組字區，組字面板的 `mixed-reading-hint` 以霧藍字與淡色底顯示 `ㄕㄨ`，沒有序號，不能點選。只有提示列時用候選字級；上方提示與候選同時顯示時，提示列字級縮小。聲調完成後轉中文，Enter 保留原文。來源：`platform/smart_input_windows.rs`、`windows/src/candidate.rs`。

### 圖示與中英模式提示

輸入法本身用單色「解」（`assets/mode/shanjie.ico`，DLL 資源 100）；工作列模式按鈕用「中」（101）與「A」（102）。游標旁提示以 tiny-skia 縮放同組 128 px 母圖。圖示由 `scripts/make-icons.ps1` 產生，見 `assets/mode/README.md`。

### 解除安裝詢問

一般入口為 `Shanjie.UI.exe --uninstall` 的 WinUI 畫面，預設勾選「保留個人詞庫與選字學習」；另一選項是「一併移除個人詞庫、學習與設定」，明示直接刪除且無法復原。開始執行前可取消，實際移除由 manager 經 UAC 執行。來源：`ui/SetupWindow.xaml`。命令列沒有指定資料處理選項時，manager 另有「是／否／取消」的原生確認框（`windows/src/installer.rs`）。

## Do's and Don'ts

### Do:

- **Do** 設定程式使用共用 `Sj*` ThemeResource 與控制項，保留可及性語意、共用草稿與固定儲存列。
- **Do** 候選窗與設定程式共用主題偏好，保持接近不透明底與 0.5 DIP 細邊線；高對比使用不透明系統色。
- **Do** 候選窗維持單一 NOACTIVATE HWND，以 sequence 驗證滑鼠選取。

### Don't:

- **Don't** 在 TSF 候選窗加入系統背景模糊、額外背景 HWND、WebView 或子程序；陰影與動畫維持同一視窗的 bitmap 繪製。
- **Don't** 恢復九欄格狀排列，或提供任意詞類表選單。

介面沒有使用生成影像；素材來源見 `ui/Assets/PROVENANCE.md` 與 `assets/mode/README.md`。
