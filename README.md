# 善解（Windows）

[善解注音](https://github.com/Nanako0129/shanjie)的 Windows 移植。輸入核心、詞庫與模型都來自上游，本專案負責平台整合，另外加了微軟注音鍵位、中英混打、雙 Ctrl 加詞與線上模型管理。

- **Windows**：原生 Rust TSF 輸入法，設定程式為 WinUI 3。目前版本 **0.14.0**，支援 x64 Windows（含 32 位元 App），尚不支援 ARM。
- **上游**：使用善解 v0.6.1 的核心、詞庫與整組模型。
- **Linux**：IBus 版停在 0.12.1，已停止維護；`linux/` 的原始碼沒有再編譯或測試。

版本歷程見 [Changelog](Changelog.md)，各版說明在 [docs/releases/](docs/releases/)。

## Windows 安裝

1. 從 [Release](https://github.com/tavricccc/shanjie-multiplatform/releases/latest) 下載 `Shanjie-Windows-Setup.exe` 並執行。
2. 選擇標準注音或倚天注音（更新時預選目前的配置），按「安裝」並同意 UAC。之後要換鍵盤，重新執行安裝程式即可。
3. 登出再登入，在輸入法清單選「善解注音」。

安裝只把善解加到輸入法清單最後（沒有中文（台灣）時一併加入），不改動其他語言與輸入法。所有 App 共用一個登入時啟動的常駐核心。

善解預設在登入時與之後每天檢查新版（只查詢 GitHub，不傳送輸入內容），有新版時在螢幕右下角通知，不會自動下載；可在設定的「關於」關閉「自動檢查更新」。也可在「關於」按「檢查更新」，下載並核對後自動安裝並重新開啟設定；「更新通道」預設只看正式版，改成測試版才會列出較新的測試版；也可以直接執行新版安裝程式。個人詞庫、學習、設定與下載的模型都會沿用。已開啟的 App 要重新開啟才會用到新版；任何一步失敗都會嘗試完整回復。

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
| 移動候選 | ↑↓ 或 ←，Tab／→ 展開三欄（可改） | ←→ 或 ↑↓，Tab 關閉候選並交給 App |
| 選字 | 數字確認；Enter 確認並提交 | 數字／Enter 確認後繼續組字 |
| Enter 提交範圍 | 只提交游標前，固定詞不拆開 | 整句（可另開游標前提交） |
| 空白鍵 | 插入可編輯空格（可改操作候選） | 開候選、候選中換頁 |
| 單按 Shift | 切換中英 | 切換中英 |
| Caps Lock | 照打注音 | 暫時輸入英文 |
| 選字學習 | 開啟 | 開啟 |
| Shift＋空白／Ctrl＋標點 | 全半形／全形標點 | 關閉 |
| 即時讀音預測 | 關閉 | 開啟，Tab 進入、Shift＋Tab 退出 |
| 選字後聯想 | 關閉 | 關閉 |

候選一律直式，每頁最多 9 項，字級 14–32 DIP。「介面與選字框主題」可選跟隨 Windows、淺色或深色，同時套用到設定程式、候選窗與加詞浮層；高對比模式優先用系統色。完整鍵位與每個設定見 [微軟注音對照與設定索引](docs/MICROSOFT-IME-DIFFS.md)。

開啟「記憶選字」後，數字鍵、Enter 或滑鼠確認候選就會建立暫存記憶，組字提交時保存；改選、刪字或取消會撤回尚未確認的記憶。「Enter 只提交游標前」也會保留完整詞組的選字記錄，例如將「風之谷」改成「楓之谷」，分段送出仍能記住整個詞，無須先按 End。連續分段累計超過 128 音節時，先保存已送出的部分，再開始下一段學習。

- **中英混打**：「中英混打與切換」頁三選一：關閉、原鍵混打或上游判斷，預設上游判斷（上游未支援的鍵盤配置下不生效）。原鍵混打模式下，未完成的輸入先顯示原始按鍵（如 `gj`），下方提示 `ㄕㄨ`，聲調完成後轉中文，Enter 保留原文；這個模式一定會開啟組字面板。上游判斷限標準注音：注音不可能是想打的字時整段變回按鍵當英文（馬上按 Esc 變回注音），Shift＋字母開始英文段。
- **組字面板**：「組字與提交」頁的選項，預設關閉，組字只顯示在 App 內，沒有候選時不顯示面板，與上游相同。開啟後在候選窗位置顯示整句組字，游標移進句子時標出游標所在的字，使用者可依需要開啟（游標在句尾時不標；原鍵混打標的是正在打的注音）。
- **純注音輸出**：「組字與提交」頁可切換，預設關閉。開啟後，標準／倚天注音逐鍵輸出符號與聲調，例如 `su3cl3` 得到「ㄋㄧˇㄏㄠˇ」，不自動轉成漢字或開啟候選、預測及聯想，也不加入選字學習。空白鍵輸入空格；方向鍵、刪除、Enter 提交與 Esc 沿用組字操作。切換時先提交現有組字；關閉後恢復原有輸入設定。拼音配置不啟用此模式，數字鍵盤仍直接輸入數字。
- **加詞**：反白 1–32 個漢字後雙按 Ctrl，在詞旁確認讀音並加入個人詞庫；加入前會比對內建詞庫、個人詞庫與目前模型。
- **模型**：內建會意 SJ-SR-8-15M 語意校正模型與上游 v0.6.1 的其餘完整配套（含口語模型），也可在設定中下載其他相容模型。配置名稱可換行，展開清單可捲動並標示目前選項。
- **貢獻者**：「關於」頁列出貢獻者（原作者 Nanako0129 置頂並標示，其餘依 GitHub 提交數排序，不顯示次數），滑鼠移入姓名可看貢獻摘要，也可開啟個人頁面與提交紀錄。頭像先連線載入，失敗時使用隨程式附上的頭像；統計更新方式見[貢獻者資料](docs/CONTRIBUTORS.md)。
- **上游功能**：資訊與 AI 詞包、模糊輸入、語意校正強度、中英文之間自動加空格、依 App 自動切換聊天／書面都可在設定中開關。

個人設定在 `%LOCALAPPDATA%\Shanjie\profile.ini`。直接改檔後，按設定頁的「重新啟動輸入法」或重新啟用善解才會讀取；這個按鈕不會終止 App，也換不掉 App 已載入的舊版 DLL。

## 開發

需要 PowerShell 7、Git、Rust MSVC、CMake 4.2 以上、Visual Studio 2026（MSVC、Windows SDK resource compiler）與 .NET 10 SDK。

```powershell
git submodule update --init
pwsh -File scripts/package.ps1      # 建置測試用候選包，位於 build/release-candidates/
pwsh -File scripts/release.ps1      # 完整 RELEASE 或重用有效紀錄，再產出正式安裝包
pwsh -File scripts/build.ps1        # 只編譯；-Architecture x64|x86 只建單一架構
pwsh -File scripts/test.ps1 -Level FAST       # 日常快速檢查
pwsh -File scripts/test.ps1 -Level AFFECTED   # 依變更範圍跑相關回歸
pwsh -File scripts/test.ps1 -Level FULL       # x64 完整回歸
pwsh -File scripts/test.ps1 -Level RELEASE    # x64/x86、UI 與安裝包驗收
```

CTest 測試分類、案例清冊、報告與基準比較方法見 [Windows 驗收流程](docs/TESTING.md)。

推送到 main 且 `windows/Cargo.toml` 的版本號變更時，GitHub Actions 會建立 Release，並建置 Windows 單檔安裝程式附上；手動執行 workflow 預設只建置驗證，不覆寫既有 Release。

`build.ps1` 的 `-UseExistingCore` 略過 prepare，`-SkipSettings` 重用已發佈的設定程式；`package.ps1 -UseExistingBuild` 只把現有 `dist/` 封裝為候選包。正式本機安裝包由 `release.ps1` 核對乾淨 commit、四處版本、完整雙架構 CTest／JUnit 與產物雜湊後提升至 `dist/Shanjie-Windows-Setup.exe`；CI-safe 或失效紀錄不能代替，流程見 [測試文件](docs/TESTING.md)。一般 PR CI 固定執行必要輕量基線，不擴大成完整驗收。

- [架構](docs/ARCHITECTURE.md)：模組、候選窗規格、設定程式、驗證指令。
- [上游差異](UPSTREAM-CHANGES.md)：核心覆蓋與升版流程。更新上游前先讀。
- [設計系統](docs/DESIGN.md)、[延遲與資源](docs/PERFORMANCE.md)。
- [維護規則](AGENTS.md)：上游子模組、commit 與發布、該跑哪些驗證。

## 致謝

- [善解注音](https://github.com/Nanako0129/shanjie)（Nanako0129，Apache-2.0）：輸入核心、詞庫與模型，本專案是它的 Windows 移植。原 repo 已無法存取，程式碼與模型 Release 改由鏡像 [tavricccc/shanjie-upstream-backup](https://github.com/tavricccc/shanjie-upstream-backup) 提供（內容與原作者發布者相同，著作權與授權仍屬原作者）。
- [Pane](https://github.com/LBEILC/Pane)（LBEILC）：介面的視覺風格參考了它。
- [Lucide](https://lucide.dev)：圖示。其他詞庫與演算法的授權見 `LICENSES/`。

## 授權

程式碼採 Apache-2.0。詞庫、模型與圖示各有資料授權（見 `LICENSES/`），Rust 相依套件的授權全文隨安裝包放在 `LICENSES/windows-rust/`，所以整包不能視為單一 Apache 授權。
