# 上游差異與更新紀錄

這份文件是後續更新善解上游的維護入口。上游目前固定於 0.2.1 的 `d1f32dac0b5dcea1c3a727c400ed748747021516`，以 `UPSTREAM.json` 與 Git 子模組實際 commit 為準。

## 2026-10-07 更新 0.2.1

使用正式 `v0.2.1` tag 的 peeled commit；不追蹤目前 main 或 model-v2。與舊 pin `a96f5c553f6ffcf9ac23670363451c49c9faf138` 比較，`core/`、`data/lexicon/`、`LICENSE` 與 `LICENSES/` 完全相同，差異為文件及評估資料。本次保留 model-v1 和既有 Windows 覆蓋；版本更新不代表換了評分模型。

## 哪些東西有改

後續模型擴充：Windows 安裝包提供 model-v1／model-v2，選擇由 `[engine] model=1|2` 控制；兩版來源、SHA-256 和映射檔名記在 `MODEL-SOURCES.json`。`prepare-models.ps1` 分別核對與轉換，並非更新子模組 main 或把 model-v2 當輸入法版本。模型切換不改 Rust 演算法、詞庫或學習格式。

`upstream/shanjie` 的追蹤檔案保持原樣。`scripts/prepare-core.ps1` 每次將上游 core 複製到 `build/core`，再套用以下程式變更。**子模組沒有被改，不代表建置出的核心完全等於上游。**

| 上游來源 | Windows 建置副本的變更 | 維護來源 |
| --- | --- | --- |
| `core/src/lib.rs` | Windows 改用平台學習儲存模組；加入唯讀映射模組 | `prepare-core.ps1`、`platform/learning_windows.rs` |
| `core/src/learn_store.rs` | 抽取原有紀錄型別、常數與解析驗證；改用 Windows 檔案處理、原子替換與同步 | `platform/learning_windows.rs` |
| `core/src/learn.rs` | Windows 日期改用 Win32 API；原有學習計分與衰減維持上游 | `platform/clock_windows.rs` |
| `core/src/lm.rs` | 9 個大型陣列由 Vec 改為可持有唯讀映射；`.winmap` 加入專用讀取路徑 | `platform/mapped_array.rs`、`platform/mapped_lm.rs` |
| `core/src/engine.rs` | 新增可設定的候選方向：右側從游標開始往右列詞，左側走原有方法；右側選完移至詞尾 | `prepare-candidates.ps1`、`platform/candidates_windows.rs` |
| `core/src/ffi.rs` | 追加 Windows 專用 `shanjie_windows_set_candidate_anchor`；沒有改動上游既有函式簽名 | `platform/candidates_windows_ffi.rs`、`src/windows_core.h` |
| `core/Cargo.toml` | 讓產生的 core 可獨立建置，加入打包時使用的模型轉換工具 | `prepare-core.ps1`、`platform/map_model.rs` |

右側候選範圍是**核心行為變更**，不能把它描述為單純換 config。原始左側模式仍是核心的預設；Windows 前端依 profile 呼叫擴充 ABI 開啟右側模式。

## 哪些只在 Windows 前端

| 範圍 | 程式位置 | 是否改上游核心 |
| --- | --- | --- |
| Windows 方向鍵與 Ctrl 標點適配 | `src/key_policy.cpp`、`src/service.cpp` | 否 |
| Shift／Ctrl＋空白中英切換、全半形、系統模式按鈕 | `src/service.cpp`、`src/mode_button.*` | 否 |
| TSF 組字、提交、游標、排版通知、外部選取變更同步 | `src/service.*` | 否 |
| 候選直列、橫列、格狀、滑鼠點選與 DPI 定位 | `src/candidates.*` | 否 |
| 個人 INI 讀取與 WinUI 設定窗 | `src/preferences.*`、`ui/Models/`、`ui/ViewModels/`、`ui/MainWindow.*` | 否 |
| 設定儲存跨實例通知與重載 | `src/profile_watcher.*`、`Service::refreshPreferences`、`ProfileStore.Save`、`apply-profile.ps1` | 否 |
| 註冊、套用輸入法清單、版本目錄安裝與乾淨移除 | `scripts/install.ps1`、`profiles.ps1`、`uninstall.ps1` | 否 |

`src/dll.cpp` 的 category 註冊包含 `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`，讓 Windows 將模式按鈕納入系統匣。這是 Windows 註冊差異，不是 Rust 核心修改。

上游 `Chat`／`Formal` 是既有 LM 權重模式。Windows 設定窗只是呼叫上游 `shanjie_engine_set_profile`，沒有新增或修改其 scoring 權重。詞庫、beam decoder、bigram scorer 與詞頻資料不因操作 profile 改寫。

## 設定檔能改什麼

2026-10-07 新增 `platform/input_windows.rs`、`input_windows_ffi.rs`：Big5 候選篩選、個人詞庫 constructor、聯想詞查詢及注音原文提交，經 `prepare-candidates.ps1` 追加到建置副本。候選方向與這些功能都屬於核心擴充；上游 C header 及子模組未修改。覆蓋還核對兩處 `Cands::new`、個人詞庫合併與排序錨點。

`src/pinyin_input.*`、`special_input.*`、`associated_candidates.*` 處理 Windows 拼音、符號／字碼與聯想操作；`src/engine.*` 串行化選字學習及 epoch。實際 WinUI presentation 位於 `ui/`，`src/candidate_host.*` 用匿名 mapping/event 橋接兩架構 DLL。完整 schema 2 範本與差異見 `profiles/`、`MICROSOFT-IME-DIFFS.md`。

設定存於 `%LOCALAPPDATA%/Shanjie/profile.ini`；兩份範本在 `profiles/`。

- `[profile] interaction=windows|shanjie`：方向鍵操作政策。
- `[candidates] anchor=right|left`：啟用已實作的核心候選方向。
- `[candidates] open=down|space|both`：完整組字後的選字開啟鍵；待輸入注音的空白聲調不受影響。
- `[candidates] layout=vertical|horizontal|grid`：初始候選排列；格狀每列 9 個，數字選目前列。
- `[shortcuts] shift_toggle`、`ctrl_space_toggle`、`shift_space_toggle`、`ctrl_punctuation`：1 開啟、0 關閉。
- `[engine] ranking=chat|formal`：上游既有評分模式。

修改設定只會選擇**已經寫好的功能**，不是靠 INI 自動產生新的核心邏輯。設定窗不直接改 Rust 或上游檔案。

## 更新上游的流程

不要手改 `build/core`；下次 prepare 會覆寫。也不要直接把上游整棵專案 merge 到 Windows 根目錄。使用以下流程：

1. 確認 Windows repo 與子模組狀態，先讀這份差異表。
2. 在子模組 fetch，選定欲更新的 commit，比較舊 pin 與新 commit 的 C header、engine 候選範圍、learning 序列化、LM parser 與授權。
3. 更新子模組 checkout 與 `UPSTREAM.json` 的 commit，保持一致。
4. 執行 prepare。學習抽取區段、LM 陣列欄位、候選函式或初始化契約若改變，必須重新審查覆蓋層；不要移除失敗檢查來強行通過。
5. 跑核心與兩份操作 profile 的必要驗證，再打包。不要把 CI／編譯通過當成真實 App 驗收。
6. 分批 commit：上游 pin／覆蓋調整、Windows 功能與文件；在此文件記錄新 pin、差異與未驗證項目。

```powershell
git status --short
git -C upstream/shanjie status --short
git -C upstream/shanjie fetch origin
# 自行選定並 checkout 新 commit，然後更新 UPSTREAM.json。
pwsh -File scripts/prepare-core.ps1
pwsh -File scripts/verify.ps1
pwsh -File scripts/verify.ps1 -NativeOnly -Interaction shanjie
pwsh -File scripts/package.ps1
```

`prepare-candidates.ps1` 核對欄位、初始化、原有 `open_candidates`、兩個候選 span 與選字固定範圍的確切錨點。將來作者若提供選字方向 API，應遷移到正式 API 並刪除這個覆蓋層，而非永久維護兩套實作。

## 現有驗證界線

上游 109 個行為測試仍驗證原有左側模式；兩個 Windows 核心測試驗證右側、句尾邊界及多字詞。TSF harness 驗證 Windows／Shanjie profile，但鍵盤 sink 註冊仍使用測試代理，詳見 `VERIFICATION.md`。Unix 權限測試與 macOS 效能測試沒有變成 Windows 已通過項目。

目前沒有提交上游 PR。右側候選擴充與平台 backend 可在和作者討論 API 後另行整理；目前版本以本機可用為目標。
