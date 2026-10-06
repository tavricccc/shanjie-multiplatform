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
解除安裝會取消 x64 / x86 TSF profile、category 與 COM 註冊，並刪除 `%ProgramFiles%/Shanjie` 整個目錄。DLL 或模型若仍被應用程式使用，會安排在 Windows 重新啟動時刪除，腳本會顯示是否需要重新啟動；不必手動刪除。目前學習功能關閉，沒有額外寫入使用者資料。解壓下載的安裝包由你自行保留或刪除。

開發 DLL 尚未簽章。正式發佈需簽章與 Windows App 容器相容性驗證。安裝腳本使用對應架構的 regsvr32 分別註冊 x64 與 x86 DLL；目標作業系統為 x64 Windows，ARM64 尚未支援。

## 架構與資源

`src/` 只使用 `shanjie.h`。TSF 編輯只能在 edit session 中執行；候選資料複製完成即釋放 ABI output。
`upstream/shanjie` 是固定 commit 的 Git 子模組；`platform/` 為 Windows 儲存、日期與唯讀 LM 映射覆蓋層；`scripts/prepare-core.ps1` 組裝建置副本。

LM 的大陣列使用 Win32 唯讀 file mapping，程序共用檔案頁面；每個程序仍有自己的引擎狀態與詞庫。這不表示整套詞庫都已共享，也不保證特定 RAM 數字。測量應分開觀察 Private Bytes 與 Working Set，後者包含共享頁面。
模型被載入期間不允許改寫或刪除；升級需先關閉使用該 DLL 的程序或登出。

學習儲存 backend 已移植，但 frontend 保持上游預設的學習關閉；多程序學習寫入與 App 容器儲存協調完成之前不開啟。學習檔不是通用 Windows 使用者目錄 API 的替代品。

## 跟進上游

1. 在子模組 fetch 上游並選定 commit，檢查 C header、LM parser、learning serialization 和資料授權的差異。
2. 同時更新子模組 pin 與 `UPSTREAM.json`；建置要求兩者相同且上游 checkout 乾淨。
3. 執行 `scripts/verify.ps1`，再 `scripts/package.ps1`。
4. 驗證文字編輯器、Chrome/Edge、Office、終端機、密碼欄位、不同 DPI、焦點切換與 32 位元 App；完成後才標記為可日常使用。

覆蓋層抽取上游序列化區段並保留上游 parser / scorer。上游若變更對應函式、欄位或 module 宣告，建置需重新審查；這是明確的維護邊界，不承諾上游任意改動都能零修改升級。

## 授權

程式碼沿用 Apache-2.0。詞庫與模型另有 MIT、CC BY-SA 等資料授權；包內 `LICENSES` 是資料授權與歸屬依據，不能把整包稱為單一 Apache 授權。
