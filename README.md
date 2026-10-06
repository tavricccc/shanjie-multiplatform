# 善解 Windows

原生 C++ TSF DLL，消費固定版本的 [善解 Rust core](https://github.com/Nanako0129/shanjie)，不使用 Electron、WebView 或常駐模型服務。

目前是開發版：可建置的 Windows frontend，尚未宣稱已通過日常 App 相容性驗收。

## 建置與打包

需要 PowerShell 7、Git、Rust MSVC 工具鏈、Visual Studio Build Tools 的 C++ 桌面工具與 Windows SDK。

```powershell
git submodule update --init
pwsh -File scripts/package.ps1
```

輸出 `dist/Shanjie-Windows.zip`，包含 x64 與 x86 DLL，兩者共用一份模型與詞庫。打包腳本下載上游 model-v1，核對上游 SHA-256，再轉成唯讀映射格式；轉換時驗證模型陣列與語言分數一致。
`build.ps1` 只編譯，不下載模型；需要完整模型的安裝包請用 `package.ps1`。

## 安裝與移除

完整解壓安裝包，右鍵 `install.cmd` →「以系統管理員身分執行」。移除時右鍵安裝目錄（或解壓目錄）的 `uninstall.cmd`，選同一個選項。雙擊也會要求 UAC 提升權限。入口會偵測 MSI 或 Microsoft Store 版的 64 位元 PowerShell 7；缺少時透過 Windows App Installer 提供的 `winget` 自動安裝 Microsoft 官方套件，再繼續執行。若 Windows App Installer 也未安裝，腳本會明確提示安裝該系統元件。

也可以從系統管理員 PowerShell 7 執行：

```powershell
pwsh -File ./install.ps1
```

安裝到 `%ProgramFiles%/Shanjie` 並註冊兩個繁體中文 profile。登出再登入後於 Windows 輸入法清單選擇「善解注音（標準）」或「善解注音（倚天）」。不更動系統預設輸入法。
解除安裝會取消 x64 / x86 TSF profile、category 與 COM 註冊，並刪除 `%ProgramFiles%/Shanjie` 整個目錄及本帳號的 `profile.ini`。DLL 或模型若仍被應用程式使用，會安排在 Windows 重新啟動時刪除，腳本會顯示是否需要重新啟動；不必手動刪除。目前學習功能關閉。解壓下載的安裝包由你自行保留或刪除。

開發 DLL 尚未簽章。正式發佈需簽章與 Windows App 容器相容性驗證。安裝腳本使用對應架構的 regsvr32 分別註冊 x64 與 x86 DLL；目標作業系統為 x64 Windows，ARM64 尚未支援。

## 操作 profile

雙擊 `settings.cmd` 開啟原生設定窗，可調整開啟候選鍵、游標左右側、候選直列／橫列／格狀、方向鍵政策與中英切換。設定存到同一份個人 INI，不需要管理員權限。Windows 預設使用微軟注音的常用操作；也可雙擊 `apply-windows.cmd` 或 `apply-shanjie.cmd` 套用預設。

| 設定 | Windows profile | Shanjie profile |
| --- | --- | --- |
| 初始候選排列 | 直列 | 橫列，每列 9 個 |
| 開啟候選 | ↓，可改成空白或兩者 | 上游 ↓／空白 |
| 候選範圍 | 從游標右側往右 | 以游標左側結尾 |
| 候選移動 | ↑／↓；→／Tab 展開格狀列表 | 上游 ←／→；↓ 展開 |
| 選字 | 數字或 Enter | 上游數字或 Enter |
| 單按 Shift | 中英切換 | 保留上游行為 |
| Ctrl＋空白 | 中英切換 | 可在設定窗啟用 |
| Shift＋空白 | 全半形切換（英文 ASCII 輸出） | 保留上游行為 |
| Ctrl＋標點 | 對應全形標點 | 保留上游行為 |

個人設定存於 `%LOCALAPPDATA%/Shanjie/profile.ini`，切換到其他輸入法再切回善解時讀取；仍使用舊設定的程式需重新開啟。也可以編輯 INI 或套用自己的檔案：

```powershell
pwsh -File ./apply-profile.ps1 -ProfileFile ./profiles/windows.ini
```

`[profile] interaction` 可填 `windows` 或 `shanjie`；`[candidates] layout` 可填 `vertical`、`horizontal` 或 `grid`，`anchor` 可填 `right`／`left`，`open` 可填 `down`／`space`／`both`。`[shortcuts]` 使用 `1`／`0`。`[engine] ranking` 可填 `chat` 或 `formal`。完整維護說明見 [上游差異紀錄](UPSTREAM-CHANGES.md)。

安裝會呼叫 Windows `InstallLayoutOrTip` 套用目前工作階段的輸入法清單，不重寫整份語言設定。DLL 放在 `versions/<binary-hash>/`，兩架構共用根目錄的詞庫與模型；更新後已開啟的程式可能仍使用舊 DLL，需重新開啟。解除安裝會清除所有版本及本帳號的 `profile.ini`。

## 架構與資源

`src/` 使用上游 `shanjie.h` 與隔離於 `windows_core.h` 的候選方向擴充。TSF 編輯只能在 edit session 中執行；候選資料複製完成即釋放 ABI output。
`upstream/shanjie` 是固定 commit 的 Git 子模組；`platform/` 包含 Windows 儲存、日期、唯讀 LM 映射及候選方向覆蓋層；`scripts/prepare-core.ps1` 組裝建置副本。

LM 的大陣列使用 Win32 唯讀 file mapping，程序共用檔案頁面；每個程序仍有自己的引擎狀態與詞庫。這不表示整套詞庫都已共享，也不保證特定 RAM 數字。測量應分開觀察 Private Bytes 與 Working Set，後者包含共享頁面。
模型被載入期間不允許改寫或刪除；升級需先關閉使用該 DLL 的程序或登出。

學習儲存 backend 已移植，但 frontend 保持上游預設的學習關閉；多程序學習寫入與 App 容器儲存協調完成之前不開啟。學習檔不是通用 Windows 使用者目錄 API 的替代品。

## 跟進上游

更新前先讀 [UPSTREAM-CHANGES.md](UPSTREAM-CHANGES.md)，其中區分上游原檔、建置副本的核心改動、Windows 前端與 INI 功能。`AGENTS.md` 也將此文件列為維護入口。

1. 在子模組 fetch 上游並選定 commit，檢查 C header、LM parser、learning serialization 和資料授權的差異。
2. 同時更新子模組 pin 與 `UPSTREAM.json`；建置要求兩者相同且上游 checkout 乾淨。
3. 執行 `scripts/verify.ps1`，再 `scripts/package.ps1`。
4. 驗證文字編輯器、Chrome/Edge、Office、終端機、密碼欄位、不同 DPI、焦點切換與 32 位元 App；完成後才標記為可日常使用。

覆蓋層抽取上游序列化區段並保留上游 parser / scorer。上游若變更對應函式、欄位或 module 宣告，建置需重新審查；這是明確的維護邊界，不承諾上游任意改動都能零修改升級。

## 授權

程式碼沿用 Apache-2.0。詞庫與模型另有 MIT、CC BY-SA 等資料授權；包內 `LICENSES` 是資料授權與歸屬依據，不能把整包稱為單一 Apache 授權。
