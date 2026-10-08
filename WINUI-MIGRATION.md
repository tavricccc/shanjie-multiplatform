# Rust 原生候選與 WinUI 3 設定

2026-10-09：Windows 可移植的產品邏輯已移至 Rust；候選依使用者 macOS 截圖重製為單行膠囊，直向與格狀移除。WinUI 3 設定仍保留 Windows 原生控制項與既有導覽。

## 模組邊界

| 模組 | 職責 |
| --- | --- |
| `windows/src/tsf/` | Rust COM factory、TSF composition／edit session、sink、按鍵與 candidate UI element、工作列模式按鈕 |
| `engine.rs`／`input.rs` | 核心 handle、學習 mutex／epoch、注音／拼音、符號／字碼、聯想与按鍵政策 |
| `candidate.rs`／`native.rs` | Direct2D／DirectWrite layered popup、DPI／工作區、透明模式提示與設定通知 |
| `profile.rs`／`bridge.rs` | 共用 INI 遷移、範本、原子保存、詞庫、窄 C ABI |
| `registration.rs`／`installer.rs` | TSF／COM 註冊、套件校驗、更新與回復、舊版清理、完整解除安裝 |
| `ui/` | C#／XAML WinUI 控制項、草稿綁定、原生對話框与測試擷取 |
| `tests/support/` | C++ 測試 adapter，沒有打包至產品 |

生產 DLL 為 Rust cdylib，靜態 CRT，ICO resources 101／102 經 SDK rc 嵌入。Windows core 仍由 `prepare-core.ps1` 組裝，與前端 crate 分離；不修改上游追蹤檔或產生目錄。沒有新增候選子程序、WebView 或 WinUI runtime 至文件 App。

## 候選契約

完整視覺契約見 [docs/candidate-surface.md](docs/candidate-surface.md)。預設字級 16／序號 9 DIP、選取高 24／外框至少 30 DIP、內距 3 DIP。單框透明圓角、藍色選取，兩種主題與系統高對比。按詞量測，窄螢幕使用左右箭頭瀏覽，同一核心頁的索引不改變；超長單詞才省略。被動預測無序號／選取，Tab 進入後才顯示。

視窗為 TSF 同程序的 Win32 popup、NOACTIVATE、無動畫；mouse sequence 與非同步 edit session 的 revision 分別檢查，失焦與取消立即關閉。彩色 emoji 使用 Windows 安裝字型，不宣稱所有旗幟字形可用。模式提示由 tiny-skia 依原 SVG 幾何畫 28 DIP，透明、1400 ms。

## 設定與打包

WinUI 為 x64 .NET 10／Windows App SDK self-contained unpackaged 程式。parser、schema 4 遷移、範本套用、詞庫校驗與原子寫入均呼叫同一份 Rust；C# 保留原生綁定與控制項。新 schema 固定 model-v4＋classes-v2／horizontal，保留其他個人偏好與學習。

整份 DLL、UI、工具、詞庫與最新模型位於不可覆寫的唯一 version 目錄。Rust manager 提權並驗證套件、註冊兩架構、核對路徑／雜湊後才清舊版；使用中檔案排入 reboot queue。Installed Apps 可移除，產品在任何刪除前詢問保留個人詞庫／學習、全部刪除或取消。

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

原生背景擷取在 `build/native-review/{x64,x86}`，RGBA 透明 PNG 審查副本在 `.impeccable/review/`；WinUI 真實離屏 XAML PNG 在 `build/ui-review-final`。這些不等同已註冊 TIP 的前景 App、實際工作列、多螢幕、高對比或重新啟動解除安裝驗收。實際通過與未驗收項目見 VERIFICATION.md。
