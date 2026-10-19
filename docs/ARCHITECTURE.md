# 架構

Windows 版是原生 Rust TSF 輸入法，Linux 版是 IBus 引擎，兩者共用同一份由上游產生的核心。輸入內容只在本機處理，不上網傳送。

## 模組

| 位置 | 職責 |
| --- | --- |
| `upstream/shanjie` | 固定 commit 的上游子模組，保持乾淨 |
| `platform/`、`scripts/prepare-*.ps1` | Windows 對核心的擴充；prepare 時套入 `build/core`（產生檔，不手改） |
| `shared/` | Windows 與 Linux 共用：按鍵決策、設定檔解析與遷移、詞庫合併、模型驗證、核心通訊協定 |
| `windows/src/tsf/` | COM factory、組字與 edit session、sink、按鍵、候選 UI element、工作列模式按鈕 |
| `windows/src/engine.rs`、`input.rs` | 核心 handle、學習 mutex／epoch、拼音、符號／字碼、聯想與按鍵政策 |
| `windows/src/candidate.rs`、`native.rs` | Direct2D／DirectWrite 候選窗、DPI 與工作區定位、中英模式提示、設定通知 |
| `windows/src/profile.rs`、`bridge.rs` | 設定範本、原子保存、個人詞庫、給 WinUI 的窄 C ABI |
| `windows/src/registration.rs`、`installer.rs` | 註冊、校驗、更新與回復、舊版清理、解除安裝 |
| `ui/` | WinUI 3 設定程式（C#／XAML），只負責控制項與草稿綁定 |
| `linux/` | IBus 引擎、GTK 4＋libadwaita 設定程式、安裝腳本 |
| `tests/support/` | C++ 測試 adapter，不進安裝包 |

Rust DLL 使用靜態 CRT，x64 與 x86 各一份（x86 供 32 位元 App 載入）。所有前端透過本機管道連到同一個 x64 常駐核心 `ShanjieCore`，詞庫與模型只載入一次。核心忙碌或斷線時，前端依請求序號確認結果，只重建已確認的未提交組字。

## 候選窗

單一 Win32 layered popup，由 Rust 以 Direct2D／DirectWrite 繪製，在 TSF 同程序內，不啟動子程序或載入 WinUI。視窗 `WS_EX_NOACTIVATE`，不搶焦點，沒有陰影與動畫；alpha 只用於外側圓角抗鋸齒，正文不透明。

- **排列**：橫向膠囊（預設）或直式，每頁最多 9 項。橫向依字詞自然寬度排列，窄工作區用兩端箭頭翻動，只有單項超長才省略。
- **尺寸**：字詞 16 DIP、序號 9 DIP，依個人字級（14–32）等比縮放；選取膠囊至少 24 DIP，外框至少 30 DIP，內距 3 DIP，與 caret 間距 6 DIP。
- **配色**：淺色 `#FFFFFF` 底、`#222225` 字、`#65656E` 序號、`#D3D3D8` 邊線；深色 `#202020` 底、`#F5F5F7` 字、`#B8B8C2` 序號、`#505055` 邊線。邊線 0.5 DIP，選取為 `#006EE6` 配白字。主題與設定程式共用，跟隨 Windows 時讀 `AppsUseLightTheme`；高對比改用系統色。
- **混打提示**：Windows 注音順序混打時，原始按鍵（如 `gj`）留在組字區，候選窗位置改顯示藍底白字的 `ㄕㄨ`，沒有序號、不能點選。
- **預測**：被動預測沒有序號與反白，按 Tab 進入後才出現。
- **滑鼠**：點擊帶 sequence，TSF 拒絕過期點擊；失焦、取消與 layout 銷毀都會關閉候選。

中英模式提示是另一個 NOACTIVATE 視窗，以 tiny-skia 繪製 `assets/mode/` 的圖示，顯示 1400 ms 或到下一次輸入。

## 設定程式

WinUI 3 self-contained unpackaged 程式（x64，.NET 10／Windows App SDK），分九頁：一般、中英混打與切換、快捷鍵與符號鍵、標點符號、組字與提交、選字、外觀、預測詞庫與學習、關於與相容性。

設定是草稿，按「儲存」才寫入 `%LOCALAPPDATA%\Shanjie\profile.ini` 並通知已啟用的實例。解析、schema 遷移、範本與保存都在 Rust（`shared/`）；讀取只在記憶體中遷移，明確儲存時才備份與寫回。命令列的 `apply-windows.cmd`／`apply-shanjie.cmd` 走同一套邏輯。設定程式跨程序只開一份，重複啟動會轉到既有視窗。

## 安裝與版本目錄

DLL、設定程式、工具與內建模型放在唯一的 `versions/<DLL hash>-<stamp>/`。Rust 安裝管理程式 `shanjie-manager.exe` 驗證檔案、註冊、核對 COM 路徑與雜湊後才清除舊版；使用中刪不掉的檔案排入重新開機刪除。變更前保存註冊、語言清單與啟動項目，失敗時嘗試完整回復。安裝紀錄格式見 [README](../README.md#疑難排解)。

## 驗證

```powershell
pwsh -File scripts/verify.ps1                                 # 核心、上游回歸與 Windows profile
pwsh -File scripts/verify.ps1 -NativeOnly -Interaction shanjie
pwsh -File scripts/verify-interaction.ps1
pwsh -File scripts/verify-mode.ps1
pwsh -File scripts/verify-ui.ps1                              # WinUI 離屏 render
cargo test --manifest-path windows/Cargo.toml --lib
pwsh -File scripts/package.ps1                                # 打包並校驗
```

這些是背景 TSF harness、離屏 render 與單元測試。TSF harness 用真正的 Windows context 與 edit session，但鍵盤 sink 註冊以 ThreadProxy 代替。它們不等於在真實 App 中驗收；尚待實機確認的項目列在 [TODO.md](../TODO.md)。

## 尚未支援

數位簽章、Windows on ARM。
