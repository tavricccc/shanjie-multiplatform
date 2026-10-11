# 上游差異與更新紀錄

這份文件是後續更新善解上游的維護入口。上游固定於 2026-10-11 main 的 `368eebc426981852094d8c0c49b8b0618143bd2b`，以 `UPSTREAM.json` 與 Git 子模組實際 commit 為準。

## 2026-10-11 同步 main 與 model-v5

包含正式 v0.4.0 及其後已合入 main 的 Kneser–Ney side file、Rust tiny-LM／SJTL0001 研究核心。新增詞包、縮寫組字、預測起點及 Enter 提交未完成注音等上游行為。Windows 仍僅提供橫向候選；本次接入完整核心，不新增詞包／縮寫／研究模型設定與下載。

C ABI 的 ShanjieOutput 尾端新增 candidate_vertical；候選 constructor 新增方向參數。平台 Output literals 和 Cands 擴充同步更新。映射模型保持 SJWM0002，補足來源 sha 與 KN 狀態，沿用上游 side-file 拒絕／配對規則；沒有 KN 檔時保持上游預設關閉。tiny-LM 核心與既有研究 API 一同編譯，沒有自動啟用 rerank。

包只提供 model-v5＋classes-v3，schema 5 遷移舊模型並保留已存詞庫、學習與外觀。官方來源 SHA-256 與大小記於 MODEL-SOURCES.json。prepare 覆蓋契約與映射等價核對已通過，完整回歸／打包結果見本輪 VERIFICATION.md。

Windows TSF 啟用僅接入 sinks、設定與原生通道，不再同步讀詞庫／模型。首次確定接收中文／全形輸入才建立引擎；OnTestKeyDown 不載入核心，Win 組合鍵在取 context／compartment 前放行。閒置失焦不重置核心或等待學習 mutex。按鍵不查 INI；設定儲存主動通知仍保留，直接改檔需明確重啟或重新啟用。重啟只釋放引擎，不替換已載入 DLL。

學習仍在使用者範圍 mutex 內核對 epoch 及 learning.tsv 版本；檔案沒有變動時不重新解析整份 store，自己的 append／rewrite 完成後記錄版本。跨實例寫入和清除仍在下一次學習操作重讀。這項差異屬於 Windows bridge，沒有改上游學習權重。

## 2026-10-08 同步 v0.3.0，只保留最新模型

從原 model-v4 快照至正式 v0.3.0，`core/src/`、C header、詞庫與 LICENSE 沒有變更；上游更新模型引用、驗證 fixture、文件與 macOS 候選高度／定位。Windows 只打包 model-v4＋classes-v2，舊模型設定遷移至 v4，沒有執行期模型 fallback。prepare 已重新核對覆蓋契約與 v4 映射等價。下方四版模型紀錄是歷史實作，現行包以 `MODEL-SOURCES.json` 的單一模型為準。

## 2026-10-08 接入 model-v3／v4 與詞類表

v3 必須配 classes-v1，v4 必須配 classes-v2。上游新增詞類機率、前文解碼、即時讀音預測與敏感詞降權，因此這次同時更新核心與疊加詞庫，不能只更換模型檔。上游既有 C ABI 新增兩個設定函式；候選結構尺寸未變，但 selected=-1 且有候選表示尚未進入的預測列。

Windows 的 SJWM0002 映射格式保存來源模型／詞類表雜湊、原有九個模型陣列和三個詞類陣列；每組放在 `data/models/model-vN/`。v3／v4 的原版 `classes.sjc` 同目錄保存，缺少或配錯會明確拒絕載入。v1／v2 明確不使用詞類項，並非新模型載入失敗後的替代。舊 SJWM0001 由打包流程重新產生，不保留執行期相容分支。

平台覆蓋另核對新的 Fixed.key 欄位、個人詞庫的 demote.tsv 完整性、預測列 Big5 篩選及列種類擴充 ABI。Windows 在載入模型前關閉即時預測；關閉時不建立預測索引，以免不用的功能增加每個 App 的資源。模型來源與配對 SHA-256 見 `MODEL-SOURCES.json`，實際驗證結果見 `VERIFICATION.md`。

## 2026-10-07 更新 0.2.1

使用正式 `v0.2.1` tag 的 peeled commit；不追蹤目前 main 或 model-v2。與舊 pin `a96f5c553f6ffcf9ac23670363451c49c9faf138` 比較，`core/`、`data/lexicon/`、`LICENSE` 與 `LICENSES/` 完全相同，差異為文件及評估資料。本次保留 model-v1 和既有 Windows 覆蓋；版本更新不代表換了評分模型。

## 哪些東西有改

Windows 安裝包只提供 model-v5＋classes-v3，schema 5 將所有舊模型設定遷移至 v5，沒有選單或執行期 fallback。來源、配表、大小、SHA-256 和映射檔名記在 `MODEL-SOURCES.json`。`prepare-models.ps1` 核對並轉換這一組，清除舊快取與 dist 模型目錄；`download-asset.ps1` 以可續傳的並行範圍下載資產。模型資料 tag 與 App release 分開記錄。

`upstream/shanjie` 的追蹤檔案保持原樣。`scripts/prepare-core.ps1` 每次將上游 core 複製到 `build/core`，再套用以下程式變更。**子模組沒有被改，不代表建置出的核心完全等於上游。**

| 上游來源 | Windows 建置副本的變更 | 維護來源 |
| --- | --- | --- |
| `core/src/lib.rs` | Windows 改用平台學習儲存模組；加入唯讀映射模組 | `prepare-core.ps1`、`platform/learning_windows.rs` |
| `core/src/learn_store.rs` | 抽取原有紀錄型別、常數與解析驗證；改用 Windows 檔案處理、原子替換與同步 | `platform/learning_windows.rs` |
| `core/src/learn.rs` | Windows 日期改用 Win32 API；原有學習計分與衰減維持上游 | `platform/clock_windows.rs` |
| `core/src/lm.rs` | 9 個模型與 3 個詞類陣列改為可持有唯讀映射；`.winmap` 加入專用讀取路徑與來源配對驗證 | `scripts/prepare-lm.ps1`、`platform/mapped_array.rs`、`platform/mapped_lm.rs` |
| `core/src/engine.rs` | 右側候選範圍、↑ 循環換字並保留游標；個人詞庫、Big5 篩選、聯想與預測列種類查詢；預測停用時不建索引 | `scripts/prepare-candidates.ps1`、`platform/candidates_windows.rs`、`platform/input_windows.rs` |
| `core/src/engine.rs` 按鍵處理 | 可關閉 Shift／直接中文標點與單獨聲調；中文標點插入既有組字 | `scripts/prepare-keys.ps1`、`platform/key_options_windows.rs` |
| `core/src/ffi.rs` | 追加 Windows 專用候選、輸入選項、標點及預測列查詢 ABI；不改上游既有函式簽名 | `platform/*_windows_ffi.rs`、`windows/src/engine.rs` |
| `core/Cargo.toml` | 讓產生的 core 可獨立建置，加入打包時使用的模型轉換工具 | `prepare-core.ps1`、`platform/map_model.rs` |

右側候選範圍是**核心行為變更**，不能把它描述為單純換 config。原始左側模式仍是核心的預設；Windows 前端依 profile 呼叫擴充 ABI 開啟右側模式。

## 哪些只在 Windows 前端

| 範圍 | 程式位置 | 是否改上游核心 |
| --- | --- | --- |
| Windows 方向鍵、Ctrl 標點對照及快捷鍵政策 | `windows/src/input.rs`、`tsf/keys.rs` | 按鍵政策在前端；標點插入與循環換字呼叫上述核心擴充 |
| 中英／全半形切換、工作列模式按鈕 | `windows/src/tsf/keys.rs`、`tsf/mode.rs`、`native.rs` | 否 |
| TSF 組字、提交、游標、sink、COM lifetime | `windows/src/tsf/` | 否 |
| 僅橫向候選、滑鼠點選與 DPI 定位 | `windows/src/candidate.rs`、`tsf/candidates.rs` | 否 |
| 共用 INI、詞庫／學習保存、WinUI 設定綁定 | `windows/src/profile.rs`、`bridge.rs`、`ui/` | 否 |
| 設定跨實例通知與重載 | `windows/src/native.rs`、`tsf/service.rs` | 否 |
| 註冊、套用清單、版本安裝／清理／解除安裝 | `windows/src/registration.rs`、`installer.rs`、`bin/shanjie-manager.rs` | 否 |

`windows/src/registration.rs` 的 category 包含 `GUID_TFCAT_TIPCAP_SYSTRAYSUPPORT`。這是 Windows 註冊差異，不是上游核心修改。產品不再編譯 C++，測試 ABI adapter 在 `tests/support/`；WinUI 的 C#／XAML 僅保留原生控制項與草稿綁定。

上游 `Chat`／`Formal` 是既有 LM 權重模式。Windows 設定窗只是呼叫上游 `shanjie_engine_set_profile`，沒有新增或修改其 scoring 權重。詞庫、beam decoder、bigram scorer 與詞頻資料不因操作 profile 改寫。

## 設定檔能改什麼

2026-10-07 新增 `platform/input_windows.rs`、`input_windows_ffi.rs`：Big5 候選篩選、個人詞庫 constructor、聯想詞查詢及注音原文提交，經 `prepare-candidates.ps1` 追加到建置副本。候選方向與這些功能都屬於核心擴充；上游 C header 及子模組未修改。覆蓋還核對兩處 `Cands::new`、個人詞庫合併與排序錨點。

Windows 拼音、符號／字碼與聯想操作在 `windows/src/input.rs`；`engine.rs` 協調學習與 epoch。`candidate.rs` 在兩架構 DLL 內直接建立單一 layered Win32 橫向膠囊，純白實色、0.5 DIP 淡灰邊線、藍色反白與系統高對比；alpha 只供外角抗鋸齒，沒有毛玻璃、第二背景窗或候選子程序。schema 5 範本、舊版遷移及開關索引見 `profiles/`、`MICROSOFT-IME-DIFFS.md`。命令列與 WinUI 共用 Rust parser／範本／保存；操作預設保留資料與外觀偏好。

設定存於 `%LOCALAPPDATA%/Shanjie/profile.ini`；兩份範本在 `profiles/`。

- `[profile] interaction=windows|shanjie`：方向鍵操作政策。
- `[candidates] anchor=right|left`：啟用已實作的核心候選方向。
- `[candidates] open=down|space|both`：完整組字後的選字開啟鍵；待輸入注音的空白聲調不受影響。
- `[candidates] layout=horizontal`：唯一排列。舊直向／格狀設定遷移；窄工作區以箭頭瀏覽同頁候選。
- `[shortcuts] shift_toggle`、`ctrl_space_toggle`、`shift_space_toggle`、`ctrl_punctuation`：1 開啟、0 關閉。
- `[engine] ranking=chat|formal`、`prediction`、`demote`：上游既有權重／即時預測／降權 API；Windows 範本停用即時預測與 `[input] associations`。
- `[input] escape`、`space_after_commit`、`pinyin_space_converts`，以及候選 Enter、↑、Backspace、翻頁開關：各自選擇已實作的政策。Tab／方向鍵不再展開格狀，完整對照以 `MICROSOFT-IME-DIFFS.md` 為準。

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

`prepare-lm.ps1`、`prepare-candidates.ps1` 與 `prepare-keys.ps1` 核對陣列／詞類型別、Fixed.key、初始化、`open_candidates`、兩個候選 span、固定範圍及按鍵分支的確切錨點。將來作者若提供相同正式 API，應遷移並刪除對應覆蓋，而非永久維護兩套實作。新原始碼差異不能只靠替換字串數量來認定等價。

## 現有驗證界線

核心與上游回歸、Windows 的最新模型／配表拒絕、兩種操作 profile、純 Rust TSF、候選／模式提示及 WinUI 離屏驗證詳見 `VERIFICATION.md`。TSF harness 的鍵盤 sink 註冊仍使用測試代理；Unix 權限、macOS 效能與 ignored 量測沒有變成已通過項目。安裝器清理 fixture 通過，不等同系統註冊／重開機移除實測。

目前沒有提交上游 PR。右側候選擴充與平台 backend 可在和作者討論 API 後另行整理；目前版本以本機可用為目標。
