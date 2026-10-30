# 善解（Windows）

[善解注音](https://github.com/Nanako0129/shanjie)的 Windows 移植。輸入核心、詞庫與模型都來自上游，本專案負責平台整合，另外加了微軟注音鍵位、中英混打、雙 Ctrl 加詞與線上模型管理。

- **Windows**：原生 Rust TSF 輸入法，設定程式為 WinUI 3。目前版本 **0.14.0**，支援 x64 Windows（含 32 位元 App），尚不支援 ARM。
- **上游**：使用善解 v0.6.0 的核心、詞庫與整組模型。
- **Linux**：IBus 版停在 0.12.1，已停止維護；`linux/` 的原始碼沒有再編譯或測試。

版本歷程見 [Changelog](Changelog.md)，各版說明在 [docs/releases/](docs/releases/)。

## Windows 安裝

1. 從 [Release](https://github.com/tavricccc/shanjie-multiplatform/releases/latest) 下載 `Shanjie-Windows-Setup.exe` 並執行。
2. 選擇標準注音或倚天注音（更新時預選目前的配置），按「安裝」並同意 UAC。之後要換鍵盤，重新執行安裝程式即可。
3. 登出再登入，在輸入法清單選「善解注音」。

安裝只把善解加到輸入法清單最後（沒有中文（台灣）時一併加入），不改動其他語言與輸入法。所有 App 共用一個登入時啟動的常駐核心。

更新直接執行新版安裝程式，個人詞庫、學習、設定與下載的模型都會沿用。已開啟的 App 要重新開啟才會用到新版；任何一步失敗都會嘗試完整回復。

### 解除安裝

從「已安裝的應用程式」移除，或在設定的「關於與相容性」頁按「解除安裝善解…」。仍被 App 載入的檔案在重新登入或重新開機後刪除。個人資料可以選：

- **保留個人詞庫與選字學習**（預設）：移除程式與設定，詞庫與學習供重新安裝使用。
- **一併移除個人詞庫、學習與設定**。

選擇移除時個人資料會直接刪除，無法復原。命令列可用安裝目錄的 `uninstall.cmd`，或 `tools/shanjie-manager.exe --elevate uninstall --keep-user-data`／`--delete-user-data`。

### 疑難排解

| 紀錄 | 位置 |
| --- | --- |
| 安裝錯誤原因 | `%ProgramFiles%\Shanjie\.installer-recovery\<識別碼>\error.txt` |
| 安裝階段與回復結果 | 同目錄 `events.jsonl` |
| 檔案清單校驗 | 同目錄 `validation.json` |
| 早期失敗 | `%LOCALAPPDATA%\Shanjie\installer-logs\<識別碼>\` |

錯誤對話框會附上紀錄位置。DLL 目前沒有數位簽章。

## 設定

安裝完成畫面可直接開啟設定；之後在輸入法的中英模式按鈕按右鍵，或執行安裝目錄的 `settings.cmd`。修改後按「儲存」才套用，不需要管理員權限。兩份操作範本的主要差異：

| 項目 | Windows 範本 | 善解範本 |
| --- | --- | --- |
| 開啟候選 | ↓（可改空白或兩者） | ↓／空白 |
| 候選範圍 | 游標右側 | 游標左側 |
| 移動候選 | ↑↓ 或 ←→，Tab 下一項 | ←→ 或 ↑↓，Tab 下一項 |
| 選字 | 數字確認；Enter 確認並提交 | 數字／Enter 確認後繼續組字 |
| Enter 提交範圍 | 只提交游標前，固定詞不拆開 | 整句（可另開游標前提交） |
| 單按 Shift | 切換中英 | 保留上游行為 |
| Shift＋空白／Ctrl＋標點 | 全半形／全形標點 | 關閉 |
| 即時讀音預測 | 關閉 | 開啟，Tab 進入、Shift＋Tab 退出 |
| 選字後聯想 | 關閉 | 關閉 |

候選每頁最多 9 項，可選橫向或直式，字級 14–32 DIP。「介面與選字框主題」可選跟隨 Windows、淺色或深色，同時套用到設定程式、候選窗與加詞浮層；高對比模式優先用系統色。完整鍵位與每個設定見 [微軟注音對照與設定索引](docs/MICROSOFT-IME-DIFFS.md)。

- **中英混打**：「中英混打與切換」頁可選關閉、Windows 注音順序、上游英文詞表或兩者並用，預設關閉。Windows 注音順序模式下，未完成的輸入先顯示原始按鍵（如 `gj`），下方提示 `ㄕㄨ`，聲調完成後轉中文，Enter 保留原文。上游英文詞表限標準注音，Shift＋字母開始英文段。
- **加詞**：反白 1–32 個漢字後雙按 Ctrl，在詞旁確認讀音並加入個人詞庫；加入前會比對內建詞庫、個人詞庫與目前模型。
- **模型**：內建上游 App 的整組模型（含語意校正與口語模型），也可在設定中下載上游發布的其他相容模型。
- **上游功能**：資訊與 AI 詞包、模糊輸入、語意校正強度、中英文之間自動加空格、依 App 自動切換聊天／書面都可在設定中開關。

個人設定在 `%LOCALAPPDATA%\Shanjie\profile.ini`。直接改檔後，按設定頁的「重新啟動輸入法」或重新啟用善解才會讀取；這個按鈕不會終止 App，也換不掉 App 已載入的舊版 DLL。

## 開發

需要 PowerShell 7、Git、Rust MSVC、Visual Studio Build Tools（MSVC、Windows SDK resource compiler）與 .NET 10 SDK。

```powershell
git submodule update --init
pwsh -File scripts/package.ps1      # 建置並打包成 dist/Shanjie-Windows-Setup.exe
pwsh -File scripts/build.ps1        # 只編譯；-Architecture x64|x86 只建單一架構
pwsh -File scripts/verify.ps1       # 核心與 Windows 回歸測試
```

推送到 main 且 `windows/Cargo.toml` 的版本號變更時，GitHub Actions 會建立 Release，並建置 Windows 單檔安裝程式附上；手動執行 workflow 預設只建置驗證，不覆寫既有 Release。

`build.ps1` 的 `-UseExistingCore` 略過 prepare，`-SkipSettings` 重用已發佈的設定程式；`package.ps1 -UseExistingBuild` 只重新整理與打包現有 `dist/`。

- [架構](docs/ARCHITECTURE.md)：模組、候選窗規格、設定程式、驗證指令。
- [上游差異](UPSTREAM-CHANGES.md)：核心覆蓋與升版流程。更新上游前先讀。
- [設計系統](docs/DESIGN.md)、[延遲與資源](docs/PERFORMANCE.md)、[待辦](TODO.md)。
- [維護規則](AGENTS.md)：上游子模組、commit 與發布、該跑哪些驗證。

## 授權

程式碼採 Apache-2.0。詞庫、模型與圖示各有資料授權（見 `LICENSES/`），Rust 相依套件的授權全文隨安裝包放在 `LICENSES/windows-rust/`，所以整包不能視為單一 Apache 授權。
