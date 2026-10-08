# 善解 Windows

原生 C++ TSF DLL，消費固定版本的 [善解 Rust core](https://github.com/Nanako0129/shanjie)。選字框在 DLL 內用 Win32／Direct2D／DirectWrite 呈現；設定頁使用獨立的 WinUI 3 程式。

目前是開發版：可建置的 Windows frontend，尚未宣稱已通過日常 App 相容性驗收。

## 建置與打包

需要 PowerShell 7、Git、Rust MSVC 工具鏈、Visual Studio Build Tools 的 C++ 桌面工具、Windows SDK 與 .NET 10 SDK。NuGet.Config 已指定套件來源，建置自動 restore UI 依賴。

```powershell
git submodule update --init
pwsh -File scripts/package.ps1
```

輸出 `dist/Shanjie-Windows.zip`，包含 x64／x86 DLL、x64 WinUI 設定程式、四版模型及兩份詞類表。v3 必須配 classes-v1，v4 必須配 classes-v2；下載核對六個官方 SHA-256，再驗證模型與詞類陣列、語言分數的映射等價。來源見 [MODEL-SOURCES.json](MODEL-SOURCES.json)。大型資產採四個並行範圍下載，保留未完成分段；完整檔案通過官方雜湊後才使用。
`build.ps1` 只編譯，不下載模型；需要完整模型的安裝包請用 `package.ps1`。
已有完整的 `dist/` 建置與模型時，`pwsh -File scripts/package.ps1 -UseExistingBuild` 只整理文件、安裝檔與 ZIP，略過重建及模型轉換。

## 安裝與移除

完整解壓安裝包，右鍵 `install.cmd` →「以系統管理員身分執行」。移除時右鍵安裝目錄（或解壓目錄）的 `uninstall.cmd`，選同一個選項。雙擊也會要求 UAC 提升權限。入口會偵測 MSI 或 Microsoft Store 版的 64 位元 PowerShell 7；缺少時透過 Windows App Installer 提供的 `winget` 自動安裝 Microsoft 官方套件，再繼續執行。若 Windows App Installer 也未安裝，腳本會明確提示安裝該系統元件。

也可以從系統管理員 PowerShell 7 執行：

```powershell
pwsh -File ./install.ps1
```

安裝到 `%ProgramFiles%/Shanjie` 並註冊兩個繁體中文 profile。登出再登入後於 Windows 輸入法清單選擇「善解注音（標準）」或「善解注音（倚天）」。不更動系統預設輸入法。
解除安裝會取消 x64 / x86 TSF profile、category 與 COM 註冊，並刪除 `%ProgramFiles%/Shanjie` 整個目錄、本帳號的 `profile.ini` 及 schema 遷移備份。DLL 或模型若仍被應用程式使用，會安排在 Windows 重新啟動時刪除，腳本會顯示是否需要重新啟動。個人詞庫與學習檔會保留，學習可在設定啟用。解壓下載的安裝包由你自行保留或刪除。

開發 DLL 尚未簽章。正式發佈需簽章與 Windows App 容器相容性驗證。安裝腳本使用對應架構的 regsvr32 分別註冊 x64 與 x86 DLL；目標作業系統為 x64 Windows，ARM64 尚未支援。

## 操作 profile

雙擊 `settings.cmd` 開啟原生設定窗，可調整開啟候選鍵、游標左右側、候選直列／橫列／格狀、方向鍵政策與中英切換。設定存到同一份個人 INI，不需要管理員權限。Windows 預設使用微軟注音的常用操作；也可雙擊 `apply-windows.cmd` 或 `apply-shanjie.cmd` 套用預設。

| 設定 | Windows profile | Shanjie profile |
| --- | --- | --- |
| 初始候選排列 | 直列 | 橫列，每列 9 個 |
| 開啟候選 | ↓，可改成空白或兩者 | 上游 ↓／空白 |
| 候選範圍 | 從游標右側往右 | 以游標左側結尾 |
| 候選移動 | ↑／↓；→／Tab 展開格狀列表 | 上游 ←／→；↓ 展開 |
| 選字 | 數字確認；Enter 確認並提交，可獨立關閉提交 | 數字／Enter 確認後繼續組字 |
| 單按 Shift | 中英切換 | 保留上游行為 |
| Ctrl＋空白 | 預設關閉，保留 App 快捷鍵；可啟用 | 預設關閉；可啟用 |
| Shift＋空白 | 全半形切換（英文 ASCII 輸出） | 保留上游行為 |
| Ctrl＋標點 | 對應全形標點 | 預設關閉；Ctrl＋反斜線為原有頓號操作 |
| 即時讀音預測 | 關閉 | 開啟；Tab 進入、Shift＋Tab 退出 |
| 選字後聯想 | 關閉 | 關閉；可另行啟用 |
| Shift／直接標點、單獨聲調 | 關閉 | 保留上游操作 |

個人設定存於 `%LOCALAPPDATA%/Shanjie/profile.ini`。設定窗儲存或套用 profile 會通知已啟用的善解立即更新；直接編輯 INI 會在下一次按鍵讀取。切換鍵盤、詞庫、排名、學習或候選核心設定時會先提交目前組字，再重建引擎；單純調整外觀與開啟鍵不需重建。也可以編輯 INI 或套用自己的檔案：

「一般 → 語言模型」可選 v1／v2／v3＋classes-v1／v4＋classes-v2，皆離線運作。新安裝預設 v4；舊設定保留原選擇，舊 schema 未指定模型則保留 v1。INI 對應 `[engine] model=1|2|3|4`。切換操作預設保留模型、詞句偏好、鍵盤、個人詞庫、學習與外觀；指定模型或配對詞類表缺少／損壞會載入失敗，不改用其他版本。

「預測、詞庫與學習」分開提供即時讀音預測、選字後聯想、前文、敏感詞降權與學習。Windows 預設兩種預測皆關閉；儲存後立即清掉停用的預測列，不影響正常選字。「按鍵、組字與提交、選字與外觀」提供 Enter、↑、Tab、→、Backspace、Esc、空白、符號鍵盤與額外標點等個別選項，完整索引與判斷依據見 [MICROSOFT-IME-DIFFS.md](MICROSOFT-IME-DIFFS.md)。

設定窗第一次讀取 schema 1／2 時，保留 `profile.ini.schemaN.bak`，將原設定與當時隱含的行為遷移成 schema 3；不因升版開啟預測。重新載入會捨棄未儲存草稿；取消關閉設定窗。命令列預設與 WinUI 使用同一份 `profiles/*.ini`。

```powershell
pwsh -File ./apply-profile.ps1 -ProfileFile ./profiles/windows.ini
```

`[profile] interaction` 選擇方向鍵政策，個別開關獨立保存。`[candidates] layout=vertical|horizontal|grid`、`anchor=right|left`、`open=down|space|both`；`[input] escape=staged|clear|ignore`；布林開關填 `1`／`0`，`[engine] ranking=chat|formal`。完整範本位於 `profiles/`，核心覆蓋與更新流程見 [上游差異紀錄](UPSTREAM-CHANGES.md)。

安裝會呼叫 Windows `InstallLayoutOrTip` 套用目前工作階段的輸入法清單，不重寫整份語言設定。DLL 放在 `versions/<DLL+UI hash>/`，兩架構共用根目錄的詞庫與模型；更新後已開啟的程式可能仍使用舊 DLL，需重新開啟。解除安裝會清除所有版本及本帳號的 `profile.ini`。

## 架構與資源

原生候選、WinUI 設定與打包決策見 [WINUI-MIGRATION.md](WINUI-MIGRATION.md)。微軟注音選項與實際差異見 [MICROSOFT-IME-DIFFS.md](MICROSOFT-IME-DIFFS.md)。目前設定包含漢語／臺灣拼音、Caps Lock 英文、左右 Shift、聯想詞、Unicode／Big5 字碼、Big5 候選、個人詞庫與學習清除。依使用者要求，候選底部注音按鈕與頁次提示已移除。

`src/` 使用上游 `shanjie.h` 與隔離於 `windows_core.h` 的候選方向擴充。TSF 編輯只能在 edit session 中執行；候選資料複製完成即釋放 ABI output。
`upstream/shanjie` 是固定 commit 的 Git 子模組；`platform/` 包含 Windows 儲存、日期、唯讀 LM 映射及候選方向覆蓋層；`scripts/prepare-core.ps1` 組裝建置副本。

LM 的大陣列使用 Win32 唯讀 file mapping，程序共用檔案頁面；每個程序仍有自己的引擎狀態與詞庫。這不表示整套詞庫都已共享，也不保證特定 RAM 數字。測量應分開觀察 Private Bytes 與 Working Set，後者包含共享頁面。
模型被載入期間不允許改寫或刪除；升級需先關閉使用該 DLL 的程序或登出。

選字學習預設關閉，可在設定啟用。Windows 用使用者目錄衍生的 mutex 協調多程序，每次操作重新讀取學習；清除時更新 epoch，丟棄各引擎未提交的舊學習。背景交錯引擎測試通過，App 容器與真實多 App 尚待驗收。

## 跟進上游

更新前先讀 [UPSTREAM-CHANGES.md](UPSTREAM-CHANGES.md)，其中區分上游原檔、建置副本的核心改動、Windows 前端與 INI 功能。`AGENTS.md` 也將此文件列為維護入口。

1. 在子模組 fetch 上游並選定 commit，檢查 C header、LM parser、learning serialization 和資料授權的差異。
2. 同時更新子模組 pin 與 `UPSTREAM.json`；建置要求兩者相同且上游 checkout 乾淨。
3. 執行 `scripts/verify.ps1`，再 `scripts/package.ps1`。
4. 驗證文字編輯器、Chrome/Edge、Office、終端機、密碼欄位、不同 DPI、焦點切換與 32 位元 App；完成後才標記為可日常使用。

覆蓋層抽取上游序列化區段並保留上游 parser / scorer。上游若變更對應函式、欄位或 module 宣告，建置需重新審查；這是明確的維護邊界，不承諾上游任意改動都能零修改升級。

## 授權

程式碼沿用 Apache-2.0。詞庫與模型另有 MIT、CC BY-SA 等資料授權；包內 `LICENSES` 是資料授權與歸屬依據，不能把整包稱為單一 Apache 授權。
