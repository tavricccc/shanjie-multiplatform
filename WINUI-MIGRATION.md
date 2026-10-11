# Rust 原生候選與 WinUI 3 設定

2026-10-09：Windows 產品邏輯為 Rust；候選只保留橫向膠囊。依使用者最新指示，移除毛玻璃，固定純白實色與 0.5 DIP 淡灰細邊線。WinUI 3 設定保留原生控制項與導覽。

## 模組邊界

| 模組 | 職責 |
| --- | --- |
| `windows/src/tsf/` | COM factory、TSF composition／edit session、sink、按鍵、候選 UI element、工作列按鈕 |
| `engine.rs`／`input.rs` | 核心 handle、學習 mutex／epoch、拼音、符號／字碼、聯想與按鍵政策 |
| `candidate.rs`／`native.rs` | 單窗 Direct2D／DirectWrite 候選、DPI／工作區、模式提示與設定通知 |
| `profile.rs`／`bridge.rs` | 共用 INI 遷移、範本、原子保存、詞庫、窄 C ABI |
| `registration.rs`／`installer.rs` | 註冊、校驗、更新與回復、舊版清理、完整解除安裝 |
| `ui/` | C#／XAML WinUI 控制項、草稿綁定、原生對話框與擷取 |
| `tests/support/` | C++ 測試 adapter，不打包至產品 |

Rust cdylib 使用靜態 CRT，SDK rc 嵌入 ICO 資源 101／102。核心由 prepare 組裝，與 Windows crate 分離；不修改上游追蹤檔或 build/core。候選不啟動子程序或載入 WinUI runtime。

## 候選契約

完整契約見 [docs/candidate-surface.md](docs/candidate-surface.md)。正文純白不透明，邊線 #D3D3D8／0.5 DIP；字詞 16／序號 9 DIP，選取高至少 24／表面至少 30 DIP，內距 3 DIP。設定介面淺深主題不改候選白底，高對比仍採系統色。

只有單一 Win32 layered popup，alpha 只供外角抗鋸齒；backdrop.rs、額外背景窗、private acrylic API 與透明度配方皆移除。NOACTIVATE、無陰影、無動畫；自然詞寬、窄區箭頭、mouse sequence、revision、失焦與取消規則保持。被動預測無序號，Tab 進入後才顯示。模式提示保留 tiny-skia SVG 幾何、28 DIP、1400 ms。

## 設定與打包

WinUI 為 x64 .NET 10／Windows App SDK self-contained unpackaged 程式。parser、schema 5 遷移、範本、詞庫與保存共用 Rust；C# 僅控制項與綁定。「設定介面主題」控制 Root.RequestedTheme。

DLL、UI、工具與最新模型置於唯一 version 目錄。Rust manager 驗證、註冊並核對路徑／雜湊後清舊版；使用中檔案排入 reboot queue。解除安裝先詢問保留詞庫／學習、全刪或取消。個人資料、原子詞庫與學習協調契約不變。

## 驗證入口

```powershell
pwsh -File scripts/verify.ps1
pwsh -File scripts/verify.ps1 -NativeOnly -Interaction shanjie
pwsh -File scripts/verify-interaction.ps1
pwsh -File scripts/verify-mode.ps1
pwsh -File scripts/verify-ui.ps1
cargo test --manifest-path windows/Cargo.toml --lib
pwsh -File scripts/package.ps1
```

WM_PRINTCLIENT 白底／邊線證據在 build/native-review/{x64,x86}，straight-alpha PNG 在 .impeccable/review；WinUI 離屏 XAML PNG 在 build/ui-review-final。原生背景結果不等同前景 App、實際工作列、讀屏、多螢幕、高對比或安裝驗收；已通過與未驗收界線見 VERIFICATION.md。過往毛玻璃結果只保留為歷史。
