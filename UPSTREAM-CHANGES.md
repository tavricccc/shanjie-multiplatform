# 上游差異

使用 [善解](https://github.com/Nanako0129/shanjie) v0.4.7 Release，實際 commit 以 `UPSTREAM.json` 為準。各版變更見 [Changelog](Changelog.md)。

`upstream/shanjie` 的追蹤檔案保持原樣。`scripts/prepare-core.ps1` 每次把上游 core 複製到 `build/core`，再套用下表的變更。**子模組沒改，不代表建出來的核心等於上游**：右側候選範圍、Enter 只提交游標前、注音順序混打等都是核心行為變更，不是改設定就能做到。

## 核心覆蓋

| 上游來源 | Windows 建置副本的變更 | 維護來源 |
| --- | --- | --- |
| `core/src/lib.rs` | 改用 Windows 學習儲存模組，加入唯讀映射模組 | `prepare-core.ps1`、`platform/learning_windows.rs` |
| `core/src/learn_store.rs` | 紀錄格式、欄位驗證沿用上游；檔案處理改用 Windows 原子替換與同步。忘記詞時 flush 失敗仍替換檔案，寫入失敗刪除暫存檔 | `platform/learning_windows.rs` |
| `core/src/learn.rs` | 日期改用 Win32 API；計分與衰減不變 | `platform/clock_windows.rs` |
| `core/src/lm.rs` | 9 個模型陣列與 3 個詞類陣列改為唯讀映射；`.winmap`（SJWM0002）專用讀取與來源雜湊核對 | `prepare-lm.ps1`、`platform/mapped_array.rs`、`platform/mapped_lm.rs` |
| `core/src/engine.rs` 候選 | 右側候選範圍；↑ 循環換字並保留游標；個人詞庫、Big5 篩選、聯想、預測列種類查詢；預測關閉時不建索引 | `prepare-candidates.ps1`、`platform/candidates_windows.rs`、`platform/input_windows.rs` |
| `core/src/engine.rs` 按鍵 | 逐鍵中文標點遮罩、可關閉的單獨聲調；標點插入既有組字 | `prepare-keys.ps1`、`platform/key_options_windows.rs` |
| `core/src/engine.rs` 組字 | `key` 改為 Windows 入口（上游原函式改名 `key_original`）：字面數字與標點、標點保留未完成讀音、注音順序混打與容錯、Enter 只提交游標前。上游英文段、英文選單、數字串進行中直接交給上游 | `prepare-smart-input.ps1`、`platform/smart_input_windows.rs`、`platform/composition_windows.rs` |
| `core/src/engine.rs` 資源 | 常駐快取共用不可變詞庫、詞包、數字表與模型；快取鍵含 `names-add.tsv`、`digits.tsv`、`trigram.sjt3` | `prepare-resident.ps1`、`platform/resident_windows.rs` |
| `core/src/trigram.rs` | Windows 的 `Mapped` 改用與模型陣列相同的唯讀映射；`.winmap` 旁有 `trigram.sjt3` 才載入，壞檔拒絕 | `prepare-trigram.ps1`、`platform/mapped_windows.rs` |
| `core/src/ffi.rs` | 追加 Windows 專用的候選、輸入選項、標點、預測列查詢 ABI；不改上游既有簽名 | `platform/*_windows_ffi.rs`、`windows/src/engine.rs` |
| `core/Cargo.toml` | 產生的 core 可獨立建置，加入模型轉換工具 | `prepare-core.ps1`、`platform/map_model.rs` |

prepare 腳本以確切錨點核對陣列型別、`Fixed` 欄位、初始化、`open_candidates`、候選 span 與按鍵分支；錨點對不上就失敗。不要為了通過而移除檢查，也不要只看替換次數就認定等價。上游日後提供相同的正式 API 時，應遷移並刪除對應覆蓋。

## 只在 Windows 前端

| 範圍 | 位置 |
| --- | --- |
| 方向鍵、Ctrl 標點對照、快捷鍵政策 | `windows/src/input.rs`、`tsf/keys.rs`（標點插入與循環換字呼叫上面的核心擴充） |
| 中英／全半形切換、工作列模式按鈕 | `windows/src/tsf/keys.rs`、`tsf/mode.rs`、`native.rs` |
| TSF 組字、提交、游標、sink、COM lifetime | `windows/src/tsf/` |
| 候選窗繪製、滑鼠點選、DPI 定位 | `windows/src/candidate.rs`、`tsf/candidates.rs` |
| 設定檔、詞庫／學習保存、WinUI 綁定、跨實例通知 | `shared/`、`windows/src/profile.rs`、`bridge.rs`、`native.rs`、`ui/` |
| 常駐核心、管道 broker、使用者模型管理 | `windows/src/broker.rs`、`broker/`、`bin/shanjie-core.rs`、`model_store.rs` |
| 註冊、安裝、更新、解除安裝 | `windows/src/registration.rs`、`installer.rs`、`bin/shanjie-manager.rs` |

上游的聊天／正式權重（`shanjie_engine_set_profile`）、詞庫、beam decoder 與 bigram scorer 都沒有改。設定只選擇已實作的功能，不會產生新的核心邏輯。

## 與上游行為仍不同之處

- 上游 App 預設開啟中英混打（含數字層）；Windows 範本 `upstream_english=0`，預設不開。改預設會改變現有使用者的打字結果，留待使用者決定。
- Windows 單獨聲調關閉（`standalone_tones=0`）時，閒置按聲調鍵仍輸入字面數字，不開上游數字串。
- 標點保留未完成讀音：上游 `engine_digits_off` 的 r01／r02／r36 因此在測試中排除，其餘逐行比對上游基準。
- 上游殼層的中英文自動加空格、各 App 個別設定、語意修正（tinylm）Windows 沒有；KN 側檔、`trigram.sjt3`、tinylm 不隨安裝包出貨。
- 預測列只在第一格標 Tab 符號（上游 header 註解），尚未核對 Windows 候選窗的畫法。

## 升版流程

不要手改 `build/core`，也不要把上游整棵 merge 進來。

1. 確認本 repo 與子模組都乾淨，讀過本頁。
2. 在子模組 fetch，選定新 commit，比較 C header、候選範圍、學習序列化、LM parser 與授權的差異。
3. checkout 新 commit，同步更新 `UPSTREAM.json`；`build.ps1` 會拒絕不在 pin 上的子模組。
4. 跑 prepare。錨點失敗代表覆蓋要重新審查。
5. 跑驗證並打包。編譯通過不等於真實 App 驗收。
6. 分批 commit：pin 與覆蓋調整、Windows 功能、文件；更新本頁的差異。

```powershell
git status --short
git -C upstream/shanjie status --short
git -C upstream/shanjie fetch origin
# checkout 選定的 commit，更新 UPSTREAM.json
pwsh -File scripts/prepare-core.ps1
pwsh -File scripts/verify.ps1
pwsh -File scripts/verify.ps1 -NativeOnly -Interaction shanjie
pwsh -File scripts/package.ps1
```

目前沒有向上游提交 PR；右側候選與平台 backend 要等和作者討論 API 後再整理。
