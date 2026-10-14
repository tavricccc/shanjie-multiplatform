# 善解 Windows

原生 Rust TSF 輸入法，基於固定版本的 [善解 core](https://github.com/Nanako0129/shanjie)。同步 2026-10-11 main（包含 v0.4.0），只提供 model-v5＋classes-v3。候選是實色橫向膠囊與 0.5 DIP 細邊線，支援淺色、深色及跟隨 Windows，不使用毛玻璃或陰影；設定保留獨立 WinUI 3 原生程式。

目前 Windows 開發版為 **0.6.0**：逐鍵標點開關、操作預設匯入／匯出、直式候選、中英混打亂序注音、重新分類的設定頁，以及 WinUI／Windows Terminal 相容註冊（尚待實機驗收），見 [RELEASE-NOTES-0.6.0.md](docs/releases/RELEASE-NOTES-0.6.0.md)。0.5.3 修正單一鍵盤升級時的 Windows 清單同步、登錄權限繼承回復，並完整記錄原始安裝錯誤。保留 0.5.2 的 cmd 首字修正、核心復原、詞典保存、只安裝標準注音或倚天，以及多行詞典、登入後常駐核心、頓號與混打 Caps Lock。新版說明見 [RELEASE-NOTES-0.5.3.md](docs/releases/RELEASE-NOTES-0.5.3.md)，背景驗證與實際系統驗收的界線見 [VERIFICATION.md](VERIFICATION.md)。

## 建置與打包

需要 PowerShell 7、Git、Rust MSVC、Visual Studio Build Tools（MSVC、Windows SDK resource compiler）與 .NET 10 SDK。Rust target 與 Cargo／NuGet 相依套件由建置安裝／restore。

```powershell
git submodule update --init
pwsh -File scripts/package.ps1
```

輸出 `dist/Shanjie-Windows.zip`：x64／x86 Rust DLL、x64 self-contained WinUI 設定、原生 Rust 安裝管理程式、最新模型與詞類表、授權全文及 SHA256SUMS。下載核對官方 SHA-256，再驗證映射陣列及語言分數等價；來源見 [MODEL-SOURCES.json](MODEL-SOURCES.json)。不保留舊模型或執行期 fallback。

只編譯用 `pwsh -File scripts/build.ps1`，預設 x64 與 x86 都建置（x86 供 64 位元 Windows 上的 32 位元程式載入）；只要單一架構加 `-Architecture x64` 或 `x86`。`-UseExistingCore` 略過 prepare，`-SkipSettings` 重用已發佈的設定 UI 並更新其 Rust DLL。已有完整 `dist/` 時，`pwsh -File scripts/package.ps1 -UseExistingBuild` 重新整理、校驗及打包。

## 安裝與解除安裝

完整解壓後雙擊 `install.cmd`，或右鍵「以系統管理員身分執行」。Rust manager 透過 UAC 提權，背景使用系統內建 Windows PowerShell 5.1 同步語言清單，使用者不須另外安裝 PowerShell 7。目標是 x64 Windows；先選標準注音或倚天，只註冊選定配置的 x64／x86 profile。已有選擇的升級沿用配置；沒有紀錄的舊版會先盤點並詢問，取消則保留現狀。ARM64 尚未支援。安裝後登出再登入，選已安裝的善解配置。更換配置可執行 `重新選擇鍵盤.cmd`；命令列支援 `--keyboard standard`／`--keyboard eten`。

更新會先驗證所有安裝檔，放入新的 `versions/<DLL hash>-<unique stamp>/`（包含 UI、核心與模型），核對實際 COM DLL 路徑及雜湊，才將舊版本與殘留移到各自原目錄旁的 `.deleted_files`。變更前保存註冊、配置、完整語言清單、啟動檔案、Installed Apps 與 Run，任一步驟失敗會嘗試完整回復。回復未完成時保留相關版本與 `.installer-recovery` 紀錄，明確列出失敗項目。使用中而不能移動的項目先保留，不安排永久刪除或自動重新啟動；已開啟 App 需要重新開啟才使用新版。

安裝錯誤先存於 `%ProgramFiles%\Shanjie\.installer-recovery\<識別碼>\error.txt`，階段與回復結果見 `events.jsonl`，完整清單校驗見 `validation.json`；早期失敗另存於 `%LOCALAPPDATA%\Shanjie\installer-logs\<識別碼>\`。對話框會附紀錄位置。0.5.3 安裝時可根據已保留的 0.5.2 失敗 checkpoint 修復本程式機碼已確認的繼承旗標遺失；只在其餘權限內容完全相同時處理，詳見新版說明。

安裝時設定 `ShanjieCore` 登入啟動項目，登入後在背景預先載入共用詞庫與模型，沒有主視窗。x64／x86 前端各使用獨立組字會話。登入啟動尚未執行時，前端會啟動同一核心並等待就緒；讀寫與復原均有期限。忙碌時按順序保留按鍵；核心連線中斷則確認請求結果，只重建已確認的未提交組字。結果未知時保留並提示，避免重複提交或學習。個人設定與學習位置沿用。

可由 Windows「已安裝的應用程式」解除安裝，或執行安裝／解壓目錄的 `uninstall.cmd`。開始前詢問：

- 「是」：移除程式、註冊與全部個人資料；檔案保留至 `.deleted_files`。
- 「否」：移除程式、註冊與設定，保留詞庫／學習供重新安裝使用。
- 「取消」：不進行解除安裝。

清除包含兩架構 COM、TSF profiles／categories、本帳號輸入法清單、Installed Apps 與登入啟動登錄。會停止本登入工作階段已知的核心，並將安裝檔案移到 `.deleted_files`；尚在使用而無法移動的檔案先保留。命令列可指定 `tools/shanjie-manager.exe --elevate uninstall --keep-user-data` 或 `--delete-user-data`。DLL 尚未簽章。

## 設定與操作

雙擊 `settings.cmd`。設定草稿須按儲存才套用，`apply-windows.cmd`／`apply-shanjie.cmd` 套用同一組操作範本。儲存與命令列套用都由 Rust 共用 parser／遷移／原子寫入／通知；不需管理員權限。

| 項目 | Windows profile | 善解 profile |
| --- | --- | --- |
| 候選外觀 | 橫向膠囊，每頁最多 9 項 | 相同；沒有直向或格狀 |
| 開啟候選 | ↓；可改空白或兩者 | ↓／空白 |
| 候選範圍 | 游標右側 | 游標左側 |
| 移動候選 | ↑↓ 或 ←→；Tab 下一項 | ←→ 或 ↑↓；Tab 下一項 |
| 選字 | 數字確認；Enter 確認並提交 | 數字／Enter 確認後繼續組字 |
| 單按 Shift | 切換中英 | 保留上游行為 |
| Ctrl＋空白 | 預設關閉，可啟用 | 相同 |
| Shift＋空白／Ctrl＋標點 | 全半形／全形標點 | 預設關閉 |
| 即時讀音預測 | 關閉 | 開啟，Tab 進入／Shift＋Tab 退出 |
| 選字後聯想 | 關閉，可啟用 | 相同 |

Windows 範本保留先前指定值：學習開啟；Caps Lock 英文、提交後額外空白、拼音單空白轉換與反引號標點關閉。候選字級預設 16 DIP，可調 14–32；既有外觀偏好保留。窄工作區用候選列兩端箭頭瀏覽，不展開格狀。候選維持不透明實色底；「介面與選字框主題」同時影響 WinUI 與候選，可選跟隨 Windows、淺色或深色，高對比模式仍優先遵循系統色。

個人設定在 `%LOCALAPPDATA%/Shanjie/profile.ini`。schema 5 將舊模型遷移至 v5、舊排列遷移至 horizontal，移除格狀相關開關；其他個別偏好、詞庫與學習保留。儲存立即通知正在使用的實例；不監控 INI，也不在每次按鍵查檔案。直接改 INI 後，按設定頁的「重新啟動輸入法」或重新啟用善解才讀取。

「重新啟動輸入法」先保存未儲存草稿；沒有草稿時直接讀取磁碟設定，保留手動改檔的內容。它通知各 App 內已啟用的善解提交組字、清除備用核心並在背景重新載入，不終止 App。核心在 TSF 啟用時背景預載；失焦、提交與暫時停用後保留在 App 程序內，同設定重新啟用可重用。每個程序最多保留一個備用核心，各啟用實例的組字狀態獨立。全新 App 若在預載完成前立刻輸入，第一鍵仍須等待同一份載入工作；Win 快捷鍵與閒置失焦不等待。此按鈕不能換掉 App 已載入的舊版 DLL，安裝更新後仍須重新開啟 App。模型／詞類損壞明確拒絕載入。

完整設定與鍵位見 [MICROSOFT-IME-DIFFS.md](docs/MICROSOFT-IME-DIFFS.md)，範本在 `profiles/`。未安裝鍵盤在設定中反灰；舊偏好退回跟隨已安裝配置並提示。個人詞庫採內容雜湊命名，先核對或完整原子發布詞庫再替換 INI；損壞檔移到同目錄 `.deleted_files` 後修復。學習以使用者範圍 mutex／epoch 協調多程序。

## 維護與授權

`windows/src/` 按 engine、input、profile、candidate、native、registration、installer、tsf 分模組。Windows 的 TSF、按鍵、候選繪製、設定／詞庫／學習及安裝交易由 Rust 處理；`ui/` 的 C#／XAML 保留 WinUI 控制項與草稿綁定；`tests/support/` 的 C++ 僅是測試 ABI adapter，沒有產品 C++。CMD／PowerShell 安裝入口只呼叫 manager；`scripts/user-language-list.ps1` 編入 manager，專責呼叫 Windows International 語言設定介面。

上游是固定 commit 子模組，核心差異保留在 `platform/` 與 prepare 腳本；`build/core` 為產生檔。更新前先讀 [UPSTREAM-CHANGES.md](UPSTREAM-CHANGES.md)、[UPSTREAM.json](UPSTREAM.json)、[VERIFICATION.md](VERIFICATION.md)。架構見 [WINUI-MIGRATION.md](docs/WINUI-MIGRATION.md)，資源量測見 [PERFORMANCE.md](docs/PERFORMANCE.md)。

程式碼沿用 Apache-2.0；詞庫、模型與 icons 分別有資料授權。Rust 相依套件的鎖定版本與全文也保存在包內 `LICENSES/windows-rust/`，不能把整包稱為單一 Apache 授權。
## 可選智慧輸入

設定 →「組字與提交」可開啟「中英混打自動判斷」，預設關閉，適用標準／倚天注音。未完成輸入先顯示原始按鍵，例如 `gj`，游標下方另顯示 `ㄕㄨ` 提示；聲調完成後轉中文，Enter 保留原文。已移除免聲調／簡拼模式與開關；舊設定會清除該項。操作範例見 `docs/MICROSOFT-IME-DIFFS.md`。
