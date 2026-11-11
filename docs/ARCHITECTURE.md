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
| `ui/` | WinUI 3 設定程式（C#／XAML）、快速設定、模型下載與更新／安裝畫面；資料保存與安裝變更透過原生介面執行 |
| `linux/` | IBus 引擎、GTK 4＋libadwaita 設定程式、安裝腳本（停止維護，未驗證） |
| `tests/support/` | C++ 測試 adapter，不進安裝包 |

Rust DLL 使用靜態 CRT，x64 與 x86 各一份（x86 供 32 位元 App 載入）。所有前端透過本機管道連到同一個 x64 常駐核心 `ShanjieCore`，詞庫與模型只載入一次。核心忙碌或斷線時，前端依請求序號確認結果，只重建已確認的未提交組字。語意校正在核心的背景執行緒跑，前端查到完成後才以有序號的請求套用，因此重建時也會重放。

核心端點以使用者 SID、工作階段、核心檔案的 NT 路徑與內容、正式個人資料位置區分。NT 身分由已開啟的同一檔案取得，避免 AppContainer 無法解析 DOS 裝置路徑；啟動仍使用原 Windows 執行路徑。一般客戶端以伺服器的 NT image path 比對；受限宿主沿用僅能連至同使用者端點的既有隔離政策。

個人資料由明確的目前程序權杖呼叫 Windows Known Folder API，停用封裝重新導向；核心、TSF 與 WinUI 共用原生介面，不依宿主傳入的 LOCALAPPDATA／USERPROFILE 決定位置。只有明確的隔離測試旗標允許測試資料重導向。既有個人資料不重寫或自動合併其他位置副本。

唯讀加詞由一般使用者權限的 `shanjie-word-helper.exe` 協調，在可回收的子程序取得 UIA 選取。helper 在任何視窗建立前啟用 PerMonitorV2；浮層依自身 HWND DPI 單次換算，使用實體錨點，並隨選取位置更新。native owner 留在 helper 內，以 NOACTIVATE／TOOLWINDOW 暫時置頂，來源失效即關閉，取消非同步處理；不變更 TSF 宿主程序的 DPI 或服務權限。

## 候選窗

單一 Win32 layered popup，由 Rust 以 Direct2D software DC render target／DirectWrite 繪製，在 TSF 同程序內，不啟動子程序或載入 WinUI。視窗 `WS_EX_NOACTIVATE`，不搶焦點；以 premultiplied alpha 合成接近不透明的底面、細邊線、上緣亮邊與柔和陰影，不使用系統背景模糊。開啟時淡入並微幅縮放，選取高亮滑動，顯示中大小或位置改變時滑到新位置；隱藏立即完成，動畫政策關閉或跨螢幕 DPI 改變時直接定位。高對比改用不透明系統色並移除陰影。外觀來源與細節見 [DESIGN.md](DESIGN.md)。

- **排列**：所有範本一律直式，一列一個候選，收合時每頁最多 9 項；舊設定的 `[candidates] layout` 讀入時丟棄，下次儲存就消失（範本保留這一行是因為 Linux 前端會讀）。右方向鍵選「展開」時顯示三欄多列候選；↑↓ 沿同一直排移動，最上列 ↑ 收回，左右鍵及 Windows 互動模式的 Tab 逐項移動。格狀欄寬依內容取最大值，超過工作區時縮欄並省略。核心輸出視窗與數字標籤仍以九項為一組，1–9 選取反白所在的九項群組。
- **尺寸與字型**：字詞預設 16 DIP、序號 9 DIP，依個人字級（14–32）等比縮放；候選列至少 24 DIP，上下各留 5 DIP，與 caret 間距 6 DIP。已安裝時優先用 Noto Sans 家族；否則候選字用 Microsoft JhengHei UI，序號用 Segoe UI（`windows/src/font.rs`）。
- **配色**：一般主題預設霧藍 `#5874C9`，可跟隨 Windows 強調色或選自訂 HEX；預設藍的選取文字為白色，其他強調色依對比選黑或白。淺色底為 `#F8F8FA`、深色底為 `#1C1C20`，底面 alpha 均為 0.97，字、邊線與標記另有透明度。深色可選不透明純黑，其他底面維持原色。共用政策在 `shared/src/appearance.rs`，Windows 系統色在 `windows/src/appearance.rs`，色值由 `candidate.rs::Palette` 管理；主題偏好與設定程式共用，跟隨 Windows 時讀 `AppsUseLightTheme`，高對比優先使用系統色。
- **組字面板**（`[candidates] reading_panel`，預設關閉）：關閉時與上游相同，組字只在 App 內，沒有候選也沒有提示就不顯示面板，有候選時也不畫整句列（`shanjie_shell::input::panel_view`）。開啟後在候選窗位置顯示整句組字，游標移進句子時標出游標所在的字，使用者可依需要開啟（游標在句尾時不標；原鍵混打標的是正在打的注音）。原鍵混打需要它，`Document::reading_panel()` 在這個模式下讀出為開，不改寫設定值；舊 `[candidates] reading` 仍由遷移刪除，與它無關。
- **混打提示**：原鍵混打時，原始按鍵（如 `gj`）留在 App 組字區，組字面板以霧藍字與淡色底標出 `ㄕㄨ`，沒有序號、不能點選。顯示名稱集中在 `SettingsViewModel.RawKeyMixingName`；儲存的鍵仍是 `[input] mixed_input`。
- **預測**：被動預測沒有序號與反白，按 Tab 進入後才出現。
- **滑鼠**：點擊帶 sequence，TSF 拒絕過期點擊；失焦、取消與 layout 銷毀都會關閉候選。

中英模式提示是另一個 NOACTIVATE 視窗，以 tiny-skia 繪製 `assets/mode/` 的圖示，顯示 1400 ms 或到下一次輸入。

## 設定程式

WinUI 3 self-contained unpackaged 程式（x64，.NET 10／Windows App SDK），分九頁：一般、中英混打與切換、快捷鍵與符號鍵、標點符號、組字與提交、選字、外觀、預測詞庫與學習、關於與相容性。

設定是草稿，按「儲存」才寫入 `%LOCALAPPDATA%\Shanjie\profile.ini` 並通知已啟用的實例。解析、schema 遷移、範本與保存都在 Rust（`shared/`）；讀取只在記憶體中遷移，明確儲存時才備份與寫回。命令列的 `apply-windows.cmd`／`apply-shanjie.cmd` 走同一套邏輯。設定程式跨程序只開一份，重複啟動會轉到既有視窗。

更新檢查由 `Shanjie.UI.exe --check-update` 這個短命程序負責，常駐核心不連網。隱藏排程 `ShanjieUpdateCheck`（登入後 2 分鐘、之後每 24 小時）與開啟設定程式都走這個入口。自動檢查關閉、沒有新版或離線時，程序直接結束；排程檢查在這一版 24 小時內已通知過，或系統處於全螢幕或勿擾時也直接結束。開啟設定程式時（`--from-settings`）只要有新版就通知。否則顯示 `UpdateCard`：與設定程式相同的玻璃視窗，以 `WS_EX_NOACTIVATE` 置於主螢幕右下角，不搶焦點，一分鐘後自動關閉。通知時間存在 `update-notice.txt`；出更新的版本不用等 24 小時。「查看更新」以 `--about` 啟動設定程式，已開啟時經單一實例管道轉送 `ABOUT` 請求，保留未儲存的草稿；下載與安裝仍由使用者在「關於」頁操作。

### 快速設定

全新安裝或從舊版升級後，第一次以一般方式開啟設定程式時，主視窗會先蓋上快速設定（`QuickSetupView`）：歡迎、選範本、一個畫面一題調整細節、完成後試打。

- **觸發**：只有無參數的設定程式啟動才判斷（`QuickSetupGate.Due`），所以裝完按「開啟設定」、更新後重開與手動開啟都算；`--setup`、`--uninstall`、`--check-update`、`--verify-*` 不經過這裡。已提升權限、測試 profile（`SHANJIE_TEST_PROFILE=1`）、`--no-quick-setup` 或 `SHANJIE_NO_QUICK_SETUP=1` 都不會顯示。`--quick-setup` 與「一般」頁的按鈕可重新開啟。
- **只顯示一次**：標記是 `profile.ini` 旁的 `quick-setup.txt`，屬於目前的 Windows 使用者，不放進 profile，因為範本與 schema 遷移都會重寫 profile。開始顯示時就寫入，略過或關閉也算看過；之後升級不會再出現，只有解除安裝時連同設定刪除才重來。提升權限的程序不寫，因為它的資料夾不一定屬於那個人。
- **保存**：流程用自己的 `QuickSetupDraft`，不動設定頁的草稿；在最後一題按「套用」才經 `ProfileStore.Save` 寫入（暫存檔再整檔取代）並通知實例。
  - 選了範本：寫一份全新的 profile，不合併舊檔。內容是出貨的範本檔（`profiles/windows.ini` 或 `shanjie.ini`）、從舊檔帶過來的少數鍵（`QuickSetupDraft.Carried`：`engine` 的 `model_id`、`model_revision`、`acg_pack`、`it_pack`、`quote_pack`、`chat_apps`，`input` 的 `dictionary`、`keyboard`），再加上流程裡的選擇；其他舊值（外觀、字級、模糊音、詞句偏好等）回到範本預設。
  - 保留我現在的設定：已存的 profile 加上實際改動的鍵；沒有差異就不寫。
  - 第一次寫入前把舊檔複製成 `profile.ini.before-quick-setup.bak`（只留一份，同一次流程再寫不覆蓋）；解除安裝時隨設定一起刪除。略過或關閉不碰檔案；個人詞典與學習資料是另外的檔案，不受影響。
  - 升級者預設「保留我現在的設定」；重新開啟前若設定頁有未儲存的草稿，先問要不要儲存。
- **混打**：只提供自動判斷與原鍵混打兩種，與設定頁一樣是單選；`Document::migrate` 把兩者同時開著的舊 profile 讀成原鍵混打，沒改就不寫回檔案。原鍵混打會固定開啟組字面板，該題變灰並說明。
- **按鍵**：Enter 繼續、Esc 上一步（試打框裡兩者都屬於文字），方向鍵在答案間移動；焦點進每一頁時落在目前的答案上。
- **試打區**：是設定程式裡一般的 TextBox，和個人詞典、聊天 App 清單的欄位一樣由目前的輸入法處理，不另外載入核心；內容不保存。

示範（`TypingDemo`）是原生 XAML 元素加計時器，格與格之間的動作是 composition 動畫，沒有影片也沒有新相依。每一格的內容來自真實核心與 `tests/windows_interaction.cpp` 的結果（`QuickSetupDemos`）；Windows 關閉動畫時停在該腳本的關鍵畫面，不跑計時器。換頁、選擇、進度與完成（`Choreography`、`ChoiceCards`、`StepProgress`、`FinishMark`）都在 composition 執行緒上跑，不改版面。

`verify-ui.ps1` 的 `--verify-quick-setup <資料夾> flow` 階段在獨立 profile 上檢查上述保存與按鍵、每個示範影格與最小視窗的溢出，並把每一步、換頁中途與完成動畫的影格存成 PNG（`build/ui-review-quick-setup/quick-setup`）。RenderTargetBitmap 看不到執行中的 composition 動畫，所以中途影格是把動畫停下，依同一組關鍵影格與曲線算出該時間點的值後直接設上去再擷取。

## 安裝與版本目錄

DLL、設定程式、工具與內建模型放在唯一的 `versions/<DLL hash>-<stamp>/`。Rust 安裝管理程式 `shanjie-manager.exe` 驗證檔案、註冊、核對 COM 路徑與雜湊後才清除舊版。舊版任一架構的 `Shanjie.dll` 仍被 App 載入時，保留該版目錄中的核心、模型與其他配套，讓仍在執行的舊 DLL 可繼續使用自己的資源（`installer.rs::Leftover::retire_version`）；將舊版目錄交給一次性的 SYSTEM 排程 `ShanjieCleanup`（`installer/cleanup_task.rs`），待下次登入或開機清理，完成後排程自行移除。沒有使用中 DLL 的舊版則立即清除；安裝程式的清理是直接刪除，不另留回收副本。

輸入法清單只經 Windows 的 `InstallLayoutOrTip` 加入或移除善解自己的 TIP（由 Windows 加在最後，缺少中文（台灣）時一併加入），不改寫其他語言與輸入法；之後從 `HKCU\Control Panel\International\User Profile` 讀回，只核對善解的配置。某個語言只剩善解時拒絕解除安裝，因為移除最後一個輸入法會讓 Windows 連語言一起拿掉。變更前保存註冊（含上述清單）、啟動檔案、登入啟動與更新檢查排程，失敗時嘗試完整回復。常駐核心由工作排程器根目錄的隱藏工作 `ShanjieCore` 在任一使用者登入時啟動；安裝時移除舊版的 `Run` 啟動值，解除安裝時一併移除這個排程與 `ShanjieUpdateCheck`。安裝紀錄格式見 [README](../README.md#疑難排解)。

安裝與解除安裝畫面是設定程式的 `--setup`／`--uninstall` 模式。安裝包根目錄的 `安裝善解.exe`（`shanjie-setup`）在解壓後的安裝包旁只負責開啟 `ui\Shanjie.UI.exe --setup`；Release 的 `Shanjie-Windows-Setup.exe` 是同一個 stub 在檔尾附上整個安裝包（`setup_bundle.rs`：檔案依序寫成「路徑、大小、內容」串流，`ui\` 與 `tools\` 排在最前面，每 16 MiB 以系統 cabinet.dll 的 LZMS 壓縮，檔尾記錄長度、這兩個資料夾的大小與 SHA-256）。它先校驗整段內容，不顯示任何對話框，把 `ui\`、`tools\` 解開到 `%TEMP%\ShanjieSetup-<隨機>`，隨即從那裡開啟同一個安裝畫面；其餘檔案等使用者按下安裝（畫面寫出 `setup-extract-go`）才解開，進度寫在同資料夾的 `setup-extract.txt`（`running <已寫> <總量>`、`done` 或 `failed <訊息>`，整檔替換）。按下安裝後先顯示解開進度再開始安裝；畫面關閉後停止解開並刪除暫存；manager 安裝時仍會依 `SHA256SUMS.txt` 再校驗一次。打包由建置用的 `shanjie-pack` 完成，不放進安裝包。畫面不提權，只是 `shanjie-manager` 的外殼：它以一般權限呼叫 `--elevate install`／`uninstall`，由 manager 自己要求 UAC，安裝後也由 manager 的未提權那一半啟動使用者的核心。提權後的 manager 經 `--progress \.\pipe\ShanjieSetup-<隨機>` 把每個階段寫成一行 JSON，最後一行是結果；管道由畫面建立、只開一個實例，manager 以 SECURITY_IDENTIFICATION 連線，不讓管道端冒用提權身分。`install-status` 唯讀回報已安裝版本與鍵盤配置。「已安裝的應用程式」的解除安裝指向 `Shanjie.UI.exe --uninstall`，它先把 `ui` 與 manager 複製到 `%TEMP%\ShanjieUninstall-<隨機>` 再從那裡執行，結束後刪掉這份暫存。解除安裝直接刪除安裝目錄，剩下的使用中檔案交給 `ShanjieCleanup`，也清掉 0.12.1 以前留在 `.deleted_files\<時間戳>-<名稱>` 的副本；選擇移除時個人資料直接刪除。命令列腳本放在安裝包的 `scripts\`，`settings.cmd` 與 `uninstall.cmd` 安裝時複製到安裝目錄。

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

TSF harness 用真正的 Windows context 與 edit session，鍵盤 sink 註冊以 ThreadProxy 代替。依改動挑哪些要跑見 [AGENTS.md](../AGENTS.md)。

出貨的詞庫／詞包與測試的 `data`、`eval` 透過 `export-upstream.ps1` 從固定 Git commit 匯出，停用該次匯出的換行轉換；不依 Windows 工作副本的 CRLF。`engine_lm`、`engine_replay` 與 `engine_newword` 含延遲斷言，與其他核心測試分開，逐套、逐案例執行。回收的建置輸出保留在 `.deleted_files`，打包時排除這些目錄。

已在目前程序載入對應架構的 MSVC 與 Windows SDK 時，原生驗證及量測腳本可明確設定 `$env:SHANJIE_USE_CURRENT_TOOLCHAIN='1'`，不變更系統環境；一般建置仍以 `build.ps1 -UseCurrentToolchain` 選用目前工具。未指定時沿用 Visual Studio 的工具偵測。

## 尚未支援

數位簽章、Windows on ARM。
