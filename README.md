# 善解（Windows／Linux）

[善解注音](https://github.com/Nanako0129/shanjie)的 Windows 與 Linux 移植。輸入核心、詞庫與模型都來自上游，本專案負責平台整合，並加了一些我們想要的功能，例如微軟注音鍵位、中英混打、雙 Ctrl 加詞、線上模型管理。

- **Windows**：原生 Rust TSF 輸入法，設定程式為 WinUI 3。目前為測試版 **0.12.0**，支援 x64 Windows（含 32 位元 App），尚不支援 ARM。
- **Linux**：Fedora GNOME 上的 IBus 輸入法，測試版 **0.12.0**。
- **上游**：使用善解 v0.4.7 Release 的核心、詞庫與模型。

版本歷程見 [Changelog](Changelog.md)，各版說明在 [docs/releases/](docs/releases/)。

## Windows 安裝

1. 完整解壓 `Shanjie-Windows.zip`，雙擊 `安裝善解.exe`。安裝畫面會顯示這次是全新安裝還是升級。
2. 選擇標準注音或倚天注音（升級時預選目前的配置），按「安裝」後 Windows 會要求 UAC，畫面逐步顯示進度；失敗時會顯示回復結果與紀錄位置。之後要換鍵盤可再執行安裝，或執行 `重新選擇鍵盤.cmd`。
3. 登出再登入，在輸入法清單選「善解注音」。

命令列安裝仍可用 `install.cmd`，或 `tools/shanjie-manager.exe --elevate install --keyboard standard`／`--keyboard eten`。

安裝程式使用系統內建的 Windows PowerShell 5.1，不需要另裝 PowerShell 7。登入時由工作排程器的 `ShanjieCore` 登入工作在背景啟動常駐核心，預載詞庫與模型，所有 App 共用同一個核心；切換到善解時若核心還沒啟動，輸入法也會在背景喚醒它。

更新時先校驗並註冊新版，成功後才清除舊版；舊檔被 App 占用就在重新開機時刪除。已開啟的 App 要重新開啟才會用到新版。任何一步失敗都會嘗試完整回復。

### 解除安裝

從「已安裝的應用程式」移除，或在設定的「關於與相容性」頁按「解除安裝善解…」。解除安裝畫面會先把自己複製到暫存資料夾再執行，讓安裝目錄可以完整移除；畫面中可以選：

- **保留個人詞庫與選字學習**（預設）：移除程式與設定，詞庫與學習供重新安裝使用。
- **一併移除個人詞庫、學習與設定**。

檔案會移到 `.deleted_files`，不直接永久刪除。`uninstall.cmd` 仍可用，會以對話框詢問同樣的選擇。命令列可用 `tools/shanjie-manager.exe --elevate uninstall --keep-user-data` 或 `--delete-user-data`。

### 疑難排解

| 紀錄 | 位置 |
| --- | --- |
| 安裝錯誤原因 | `%ProgramFiles%\Shanjie\.installer-recovery\<識別碼>\error.txt` |
| 安裝階段與回復結果 | 同目錄 `events.jsonl` |
| 檔案清單校驗 | 同目錄 `validation.json` |
| 早期失敗 | `%LOCALAPPDATA%\Shanjie\installer-logs\<識別碼>\` |

錯誤對話框會附上紀錄位置。DLL 目前沒有數位簽章。

## 設定

雙擊 `settings.cmd` 開啟設定，修改後按「儲存」才套用，不需要管理員權限。也可以用 `apply-windows.cmd`／`apply-shanjie.cmd` 直接套用操作範本。兩份範本的主要差異：

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
- **模型**：可在設定中下載上游發布的其他相容模型。

個人設定在 `%LOCALAPPDATA%\Shanjie\profile.ini`。直接改檔後，按設定頁的「重新啟動輸入法」或重新啟用善解才會讀取；這個按鈕不會終止 App，也換不掉 App 已載入的舊版 DLL。

## Linux（Fedora GNOME／IBus）

IBus 引擎、GTK 4＋libadwaita 設定程式與安裝腳本都是 Rust／shell，與 Windows 共用 `shared/` 和同一份 `build/core`。差異見 [Linux 0.10.2 版本說明](docs/releases/RELEASE-NOTES-linux-0.10.2.md)。

從原始碼安裝需要 PowerShell 7（prepare 腳本）、Rust、curl、`gtk4-devel`、`libadwaita-devel`：

```bash
git submodule update --init
linux/scripts/install.sh      # 建置後以 sudo 安裝並加入 GNOME 輸入來源
linux/scripts/package.sh      # 產生 build/linux/dist/shanjie-linux-<版本>-<架構>.tar.gz
linux/scripts/uninstall.sh    # 移除；--delete-user-data 把個人資料移到 .deleted_files
```

個人資料在 `~/.local/share/shanjie`。`linux/tests/run-scenarios.sh` 在私有 ibus-daemon 上跑按鍵情境，各 crate 另有 `cargo test`。

## 開發

需要 PowerShell 7、Git、Rust MSVC、Visual Studio Build Tools（MSVC、Windows SDK resource compiler）與 .NET 10 SDK。

```powershell
git submodule update --init
pwsh -File scripts/package.ps1      # 建置並打包成 dist/Shanjie-Windows.zip
pwsh -File scripts/build.ps1        # 只編譯；-Architecture x64|x86 只建單一架構
pwsh -File scripts/verify.ps1       # 核心與 Windows 回歸測試
```

推送到 main 且 `windows/Cargo.toml` 的版本號變更時，GitHub Actions 會建立 Release，並建置 Windows zip 與 Linux tar.gz（Fedora 44 容器）附上；手動執行 workflow 會重建並覆蓋目前版本的安裝包。

`build.ps1` 的 `-UseExistingCore` 略過 prepare，`-SkipSettings` 重用已發佈的設定程式；`package.ps1 -UseExistingBuild` 只重新整理與打包現有 `dist/`。

- [架構](docs/ARCHITECTURE.md)：模組、候選窗規格、設定程式、驗證指令。
- [上游差異](UPSTREAM-CHANGES.md)：核心覆蓋與升版流程。更新上游前先讀。
- [設計系統](docs/DESIGN.md)、[延遲與資源](docs/PERFORMANCE.md)、[待辦](TODO.md)。

上游以固定 commit 的子模組 `upstream/shanjie` 引入，保持乾淨；對核心的差異只放在 `platform/` 與 prepare 腳本，`build/core` 是產生檔。

## 授權

程式碼採 Apache-2.0。詞庫、模型與圖示各有資料授權（見 `LICENSES/`），Rust 相依套件的授權全文隨安裝包放在 `LICENSES/windows-rust/`，所以整包不能視為單一 Apache 授權。
