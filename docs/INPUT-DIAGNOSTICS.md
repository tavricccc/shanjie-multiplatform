# Windows 輸入事件診斷

`input-diagnostics` 是明確啟用的 Cargo feature，一般建置不包含收集後端與控制台。它只觀察 TSF 與執行緒既有訊息 hook，不更動按鍵結果、註冊設定或宿主文字。這不是 Shift／Excel 問題的修復。

診斷包基於 0.12.1，附 `診斷控制台.exe`、完整安裝檔案及使用說明。另一份 `Shanjie-Windows-0.12.1-Before-Diagnostics.zip` 由目前已安裝版本的公開 manifest 檔案還原，提供同版覆蓋安裝的回復途徑；不複製個人資料。製作包不等於已在使用者電腦安裝。

第二版識別為 `0.12.1-diag.20261022.2-a9767ab`，控制台標題包含「第二版」。安裝後以 DLL 雜湊核對目標程序是否載入本版，舊版報告保留。補測只需 Win+R／AyuGram 各三次右 Shift，以及 Excel 方向鍵換格直接輸入／先 F2 再輸入的對照；不必重測記事本。

第三版識別為 `0.12.1-diag.20261023.3-a9767ab`，控制台標題包含「第三版」，新增「開始：加詞浮層測試」。第二版補錄已確認 AyuGram 的雙 Ctrl 成立並進入加詞讀取流程，記事本對照可出現浮層；第三版只補後續退出階段，不套用修法，也不需重測 Shift／123。

## 使用

先自行安裝診斷包，再重新開啟目標程式。控制台的「檢查診斷版與載入狀態」以套件 DLL 的 SHA-256 核對已安裝檔案與目標程序實際載入的模組。舊 DLL、尚未載入與無法檢查是不同狀態，不以程序存在當作已啟用。Win+R 所在的 Explorer 若仍使用舊 DLL，保存工作後登出再登入；控制台不終止程序、不要求提權、不自行安裝。

- **右 Shift**：選「開始：右 Shift 測試」，在 AyuGram、Win+R 與記事本空白欄位單按並放開右 Shift。只包含 `AyuGram.exe`、`explorer.exe`、`notepad.exe`。
- **Excel 首鍵**：選「開始：Excel 首鍵測試」，在空白活頁簿方向鍵換格後輸入 `123`，再與先按 F2 後輸入 `123` 對照。只包含 `EXCEL.EXE`。
- **加詞浮層**：選「開始：加詞浮層測試」，在 AyuGram、Win+R、Excel 編輯中的文字與記事本對照，反白已提交的中文字，連按兩下左 Ctrl；出現浮層時按 Esc 取消，不按加入。只包含上述四個程序，不需重測 Shift／123。
- 回控制台停止，或關閉控制台；最長五分鐘後由 DLL 自行停止，即使控制台意外結束也有效。正向快取最多 100 ms，控制台停止時等待快取失效。
- 「停止」會計算事件數並寫出 summary；零事件表示尚無證據，不能當作測試通過。

## 資料邊界

控制檔為 `%LOCALAPPDATA%\Shanjie\InputDiagnostics\control.json`，只接受固定 schema、32 位十六進位識別碼、三種測試及最多五分鐘期限。紀錄路徑由程式決定，不接受外部指定輸出位置。控制台的 CLI `--output` 僅是自動測試的結果輸出，不影響 DLL 收集路徑。

每次測試寫入 `InputDiagnostics\reports\<識別碼>`。各程序／執行緒分檔，單檔最多 2 MiB。沒有網路傳送、全域鍵盤 hook、截圖、剪貼簿、視窗標題或文字全文；紀錄不含使用者詞庫／學習，也不把 profile 複製進報告。

按鍵只標示 Shift／Control／Alt／Win、`PROCESS_KEY`、1／2／3、方向鍵、F2、Enter／Escape／Tab／Backspace，其餘統一為 `OTHER`，不保存掃描碼數值。第二版另記是否存在掃描碼、延伸鍵／按下／放開旗標；Shift 試探記正規化左右側、Ctrl／Alt 與左右 Shift 狀態，保留實際試探回覆作對照。組字及提交只保存 UTF-16 長度、游標與選取位置；不為紀錄呼叫 GetText。context／composition 與文件管理員身分以程序內雜湊表示。

第二版的 `document_focus.details` 比對事件的新舊文件管理員與目前組字輸入區的文件管理員，只呼叫 GetDocumentMgr 取得身分。`retirement.*` 分別記錄舊組字排入收尾、edit session 進入／略過、取得範圍、SetText、EndComposition 與請求返回結果。請求成功只表示已接受，最終寫入與結束結果需看各階段事件；未進入執行不宣稱完成。這些紀錄不改變原有焦點、按鍵或錯誤處理分支。

第三版另記 `ctrl.gesture` 的左右側、純修飾鍵判斷與成立結果，以及 `word.*` 的讀取請求、選取範圍／文字長度、密碼／唯讀等退出原因、文字位置、派送與建立浮層結果。紀錄不保存所選詞條；GetText 仍僅在原本加詞流程呼叫，診斷不額外讀取文字。排入派送與建立成功分別記錄，不將手勢成立等同浮層已顯示。

另保存程序名稱、PID、DLL 路徑與雜湊、來源版本與 feature build ID。無權檢查的管理員程序會明確標示，不繞過權限。

## 建置與驗證

使用正常 MSVC／Windows SDK 環境，先執行 `scripts/prepare-core.ps1`，以 `--features input-diagnostics` 建置 x64 與 x86 的 Windows crate。建置環境變數 `SHANJIE_DIAGNOSTICS_BUILD` 提供識別字串；圖示資源依原建置腳本設定。

```powershell
cargo test --manifest-path windows/Cargo.toml --lib --features input-diagnostics --target x86_64-pc-windows-msvc
cargo build --release --manifest-path windows/Cargo.toml --features input-diagnostics --target x86_64-pc-windows-msvc
cargo build --release --manifest-path windows/Cargo.toml --features input-diagnostics --target i686-pc-windows-msvc
pwsh -File scripts/package-input-diagnostics.ps1 -OutputRoot <新的輸出資料夾>
```

打包腳本驗證公開基底的每個 manifest 檔案與路徑範圍，只複製 manifest 列出的檔案，拒絕未知根目錄；不掃描 `%LOCALAPPDATA%\Shanjie` 的詞庫與學習。診斷版與原版還原包分開，均有 manifest 與 ZIP SHA-256。`DIAGNOSTICS-BUILD.json` 記錄來源 HEAD、上游 pin、feature 及實際修改檔案的雜湊。

背景驗證必須使用隔離 LOCALAPPDATA、SHANJIE_TEST_PROFILE=1 及停用學習，並確認 session 開／關、逾時、宿主與按鍵白名單、事件內容、正常輸入結果及兩架構。CLI 的測試 profile 可略過已安裝版檢查，不改正常 GUI 的安裝驗證。隔離 harness 通過不代表真實 AyuGram／Run／Excel 的問題已重現或修復。
