# 架構

Windows 版是原生 Rust TSF 輸入法，使用由上游產生的核心。Linux 版 IBus 引擎（`linux/`）已停止維護。輸入內容只在本機處理，不上網傳送。

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
| `linux/` | IBus 引擎、GTK 4＋libadwaita 設定程式、安裝腳本（停止維護，未驗證） |
| `tests/support/` | C++ 測試 adapter，不進安裝包 |

Rust DLL 使用靜態 CRT，x64 與 x86 各一份（x86 供 32 位元 App 載入）。所有前端透過本機管道連到同一個 x64 常駐核心 `ShanjieCore`，詞庫與模型只載入一次。核心忙碌或斷線時，前端依請求序號確認結果，只重建已確認的未提交組字。語意校正在核心的背景執行緒跑，前端查到完成後才以有序號的請求套用，因此重建時也會重放。

核心端點以使用者 SID、工作階段、核心檔案的 NT 路徑與內容、正式個人資料位置區分。NT 身分由已開啟的同一檔案取得，避免 AppContainer 無法解析 DOS 裝置路徑；啟動仍使用原 Windows 執行路徑。一般客戶端以伺服器的 NT image path 比對；受限宿主沿用僅能連至同使用者端點的既有隔離政策。

個人資料由明確的目前程序權杖呼叫 Windows Known Folder API，停用封裝重新導向；核心、TSF 與 WinUI 共用原生介面，不依宿主傳入的 LOCALAPPDATA／USERPROFILE 決定位置。只有明確的隔離測試旗標允許測試資料重導向。既有個人資料不重寫或自動合併其他位置副本。

唯讀加詞由一般使用者權限的 `shanjie-word-helper.exe` 協調，在可回收的子程序取得 UIA 選取。helper 在任何視窗建立前啟用 PerMonitorV2；浮層依自身 HWND DPI 單次換算，使用實體錨點，並隨選取位置更新。native owner 留在 helper 內，以 NOACTIVATE／TOOLWINDOW 暫時置頂，來源失效即關閉，取消非同步處理；不變更 TSF 宿主程序的 DPI 或服務權限。

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

DLL、設定程式、工具與內建模型放在唯一的 `versions/<DLL hash>-<stamp>/`。Rust 安裝管理程式 `shanjie-manager.exe` 驗證檔案、註冊、核對 COM 路徑與雜湊後才清除舊版；刪除一律是真的刪除，不另留備份；使用中刪不掉的檔案（通常是 App 仍載入的 `Shanjie.dll`）只留下那幾個，同時排入 Windows 重新開機刪除，並登記一次性的 SYSTEM 排程 `ShanjieCleanup`（`installer/cleanup_task.rs`），在下次任一使用者登入或開機時只刪那些路徑，完成後自行移除。輸入法清單只經 Windows 的 `InstallLayoutOrTip` 加入或移除善解自己的 TIP（由 Windows 加在最後，缺少中文（台灣）時一併加入），不改寫其他語言與輸入法；之後從 `HKCU\Control Panel\International\User Profile` 讀回，只核對善解的配置。某個語言只剩善解時拒絕解除安裝，因為移除最後一個輸入法會讓 Windows 連語言一起拿掉。變更前保存註冊（含上述清單）、啟動檔案與登入啟動排程，失敗時嘗試完整回復。常駐核心由工作排程器根目錄的隱藏工作 `ShanjieCore` 在任一使用者登入時啟動；安裝時移除舊版的 `Run` 啟動值，解除安裝時一併移除排程。安裝紀錄格式見 [README](../README.md#疑難排解)。

安裝與解除安裝畫面是設定程式的 `--setup`／`--uninstall` 模式。安裝包根目錄的 `安裝善解.exe`（`shanjie-setup`）在解壓後的安裝包旁只負責開啟 `ui\Shanjie.UI.exe --setup`；Release 的 `Shanjie-Windows-Setup.exe` 是同一個 stub 在檔尾附上整個安裝包（`setup_bundle.rs`：檔案依序寫成「路徑、大小、內容」串流，`ui\` 與 `tools\` 排在最前面，每 16 MiB 以系統 cabinet.dll 的 LZMS 壓縮，檔尾記錄長度、這兩個資料夾的大小與 SHA-256）。它先校驗整段內容，以進度對話框把 `ui\`、`tools\` 解開到 `%TEMP%\ShanjieSetup-<隨機>`，隨即從那裡開啟同一個安裝畫面，其餘檔案在背景繼續解開，進度寫在同資料夾的 `setup-extract.txt`（`running <已寫> <總量>`、`done` 或 `failed <訊息>`，整檔替換）。畫面按下安裝時若還沒解完，先顯示解開進度再開始安裝；畫面關閉後停止解開並刪除暫存；manager 安裝時仍會依 `SHA256SUMS.txt` 再校驗一次。打包由建置用的 `shanjie-pack` 完成，不放進安裝包。畫面不提權，只是 `shanjie-manager` 的外殼：它以一般權限呼叫 `--elevate install`／`uninstall`，由 manager 自己要求 UAC，安裝後也由 manager 的未提權那一半啟動使用者的核心。提權後的 manager 經 `--progress \.\pipe\ShanjieSetup-<隨機>` 把每個階段寫成一行 JSON，最後一行是結果；管道由畫面建立、只開一個實例，manager 以 SECURITY_IDENTIFICATION 連線，不讓管道端冒用提權身分。`install-status` 唯讀回報已安裝版本與鍵盤配置。「已安裝的應用程式」的解除安裝指向 `Shanjie.UI.exe --uninstall`，它先把 `ui` 與 manager 複製到 `%TEMP%\ShanjieUninstall-<隨機>` 再從那裡執行，結束後刪掉這份暫存。解除安裝直接刪除安裝目錄，剩下的使用中檔案交給 `ShanjieCleanup`，也清掉 0.12.1 以前留在 `.deleted_files\<時間戳>-<名稱>` 的副本；選擇移除時個人資料直接刪除。命令列腳本放在安裝包的 `scripts\`，`settings.cmd` 與 `uninstall.cmd` 安裝時複製到安裝目錄。

## 驗證

```powershell
pwsh -File scripts/verify.ps1                                 # 建置、上游核心全部測試、TSF 冒煙與真實 TSF 回歸
pwsh -File scripts/test-core.ps1                              # 只跑上游核心測試（-Tests 指定整合測試檔）
pwsh -File scripts/verify-regressions.ps1 -Architecture x86   # 加詞浮層、英文鍵、焦點交接、浮層生命週期
pwsh -File scripts/verify.ps1 -NativeOnly -Interaction shanjie
pwsh -File scripts/verify-interaction.ps1
pwsh -File scripts/verify-mode.ps1
pwsh -File scripts/verify-ui.ps1                              # WinUI 設定與安裝畫面離屏 render
cargo test --manifest-path windows/Cargo.toml --lib
pwsh -File scripts/package.ps1                                # 打包並校驗
```

TSF harness 用真正的 Windows context 與 edit session，鍵盤 sink 註冊以 ThreadProxy 代替。依改動挑哪些要跑見 [AGENTS.md](../AGENTS.md)；尚待實機確認的項目列在 [TODO.md](../TODO.md)。

## 尚未支援

數位簽章、Windows on ARM。
