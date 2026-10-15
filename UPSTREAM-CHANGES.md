# 上游差異與更新紀錄

這份文件是後續更新善解上游的維護入口。上游固定於 `feat/model-v6-switch` 的 `f8ea0bbed4e05ceda653cc73221660f3ee3f6873`（2026-10-15；基於 main `76dc7ac`，作者表示 model-v6 即將發佈，尚未合入 main），以 `UPSTREAM.json` 與 Git 子模組實際 commit 為準。

## 2026-10-15 Windows 0.8.0：同步上游並換 model-v6／classes-v4

上游 pin 由 `368eebc`（main）更新到 `feat/model-v6-switch` 的 `f8ea0bb`（約 1,040 個 commit，含 main 的 `76dc7ac`）。該分支尚未合入 main，待作者發佈後再改回 main 的對應 commit。內建模型換成 model-v6＋classes-v4（`MODEL-SOURCES.json`、安裝包驗證與 UI 文案同步）；model-v5／classes-v3 不再打包。

prepare 契約只需調整兩處：`prepare-candidates.ps1` 的 `Fixed` 新增 `pick_key` 欄位；`prepare-core.ps1` 的 Windows 學習儲存匯入新增 `is_full_context`、`NEXT_PREFIX`。`platform/resident_windows.rs` 改用完整路徑的 atomic 型別，避免與上游 engine.rs 已有的匯入重複。

**未接入**：kn-v1 Kneser–Ney 側檔、tinylm-v5 與重排強度設定、名單層只靠內建詞庫目錄一併複製，Windows 沒有新增對應設定或載入。上游測試期待 kn.sjkn，驗證腳本僅為測試下載，不打包。

## 2026-10-15 Windows 0.7.0：字面輸入、亂序辨識與注音候選

新增 Windows 組字政策 `platform/composition_windows.rs`，並擴充 smart-input／key-options／candidate 覆蓋與 prepare 契約。字面數字及標點留在核心組字；明確聲調先辨識完整亂序音節；原始按鍵及注音候選有獨立的選取種類。關閉空白注音開關時，只有注音文字候選的音節還原原始按鍵與空格，有效第一聲照常轉換。Windows snapshot 額外傳遞未完成讀音的狀態，避免把字面注音誤認為未完成讀音。

設定保存／分享、TSF 按鍵與候選操作及隔離驗證一起更新。所有核心變更位於 `platform/` 與 prepare 腳本，沒有手改 `build/core` 或上游追蹤檔；上游 pin、模型、內建詞庫及解碼評分不變。

## 2026-10-14 Windows 0.6.1：設定共存與提權等待

只修改 Windows profile 讀取／明確儲存、通知視窗、提權程序等待及相關驗證與文件。讀取僅在記憶體遷移；明確儲存才備份與寫回。v2 非同步通知不觸發舊 DLL，v1 接收端保持唯讀；未知新版 schema 不覆寫。提權等待改為訊息泵與 10 分鐘期限。上游 pin、platform 覆蓋、prepare 契約、詞庫、模型及解碼器不變。

## 2026-10-13 Windows 0.5.3：安裝清單、權限回復與診斷

只修改 Windows 安裝器、登錄快照、版本標示與發佈文件。新增 language_list／journal／permissions 模組：以 Windows 內建 International 介面同步現行使用者語言清單；保留原生權限繼承旗標；依舊 checkpoint 精確辨識並修復 0.5.2 的旗標遺失；先保存原始錯誤再嘗試回復。PowerShell 輔助腳本編入執行檔，資料以 JSON 傳遞。上游子模組、platform 覆蓋、prepare 契約、詞庫、model-v5／classes-v3、解碼器及輸入／broker 政策不變。隔離原生 API 測試及實際安裝的界線見 VERIFICATION.md。

## 2026-10-13 Windows 0.5.2：前端可靠性與單一鍵盤安裝

本輪只修改 Windows 前端、broker、個人資料保存、安裝器與 WinUI。上游子模組、platform 核心覆蓋、prepare 契約、詞庫、model-v5／classes-v3 及解碼器未更新。新增 request sequence／結果確認、可取消管線期限、焦點與世代復原，以及獨立的 installation／registry_snapshot／installer transaction 模組。原生安裝回復與前景 App 界線見 VERIFICATION.md。

## 2026-10-12 Windows 0.5.0：共用資源與大小寫顯示

上游 pin、模型格式、評分器與上游檔案不變。`prepare-resident.ps1` 在產生核心時，將個人詞庫建構及模型載入分成原始路徑與快取入口；`platform/resident_windows.rs` 以 Arc 分享不可變 Lexicon／LmState，最多保留四份詞庫及四組模型，依資料路徑、檔案版本與詞庫實例區分。組字與學習政策仍屬於各 Engine。

`prepare-smart-input.ps1` 注入英文顯示字元欄位；`smart_input_windows.rs` 新增 `key_with_latin`，將英文原文大小寫與注音解析分開。`smart_input_windows_ffi.rs` 新增同名 Windows FFI。TSF 輸入路徑使用 Windows 實際字元狀態，並讓混打 Shift 字母走組字路徑；原始 `key` 與上游 ABI 仍可使用。

Windows 外層新增本機管道 broker 與 x64 核心程序；x86／x64 前端共用相同核心。權限、會話隔離、重啟及重播屬 Windows 平台功能。登入啟動由安裝程式註冊，未改上游 macOS/Linux 啟動流程。

## 2026-10-11 後續：移除簡拼、原文組字與深色候選

依最新要求移除 Windows `smart_zhuyin` 欄位、免聲調／簡拼按鍵政策、設定控制項與強制 prediction。profile 遷移直接刪除舊設定，不保留兼容開關。上游既有 abbreviation API 未改動，也未在 Windows 啟用。

中英混打在未完成讀音時，以原始英數按鍵作為 preedit，另以不可選取的單項注音提示顯示 reading。聲調仍走上游完整音節轉換，Enter 可提交短／未知英文字串。Core 的 view 與 key 都覆蓋提示輸出，Windows bridge 以 prediction flags 的 bit 1 標記 reading hint；普通 prediction 保持 bit 0。TSF 與原生 renderer 拒絕提示點選，數字聲調不被當成選字。

候選 theme 不再被忽略；明確淺／深或系統 AppsUseLightTheme 決定實色配色，高對比仍優先。注音提示採選取藍與白字，沒有數字、毛玻璃或額外 HWND。介面文件以此次後續結果為準，下一段為被取代的初版紀錄。

## 2026-10-11 可選中英混打與簡拼（初版，簡拼已移除）

`platform/smart_input_windows.rs` 與 `prepare-smart-input.ps1` 新增 Windows 組字政策：本機常用英文詞辨識、英文／帶聲調中文分界、英文逐字刪除，以及免聲調／部分注音候選的空白、Enter、↓ 操作。混打的英文／注音判斷：以英文字母開頭的序列仍須照聲母→介音→韻母；以數字或符號開頭的序列（如 `0d4`）可以亂序，但同一欄不可重複（0.6.0）。簡拼沿用上游 prediction、abbreviation 與原模型排名，補上由較後注音欄回到較前欄時的新音節分界。這是核心擴充，不是只改設定，也不是華碩的私有模型。

`[input] mixed_input`、`smart_zhuyin` 預設均為 0，適用標準／倚天注音。簡拼開啟時，Windows 讓有效 prediction 為開啟，不改使用者保存的 prediction 偏好；切換任一新選項先保留組字再重建核心。舊 schema 5 由既有缺省值遷移補入兩個關閉值，不改個人資料。拼音保留設定值但不啟用這兩項政策。

英文詞表來源固定於 Rime Easy English commit `54a4a07289412efc54134092c0d945f895a71ed3`，僅抽取常用、至少三個字母的英文項目，來源與 LGPL 授權見 `LICENSES/english-data.md`。沒有網路辨識；英文候選不是另一個雲端或大型模型。完整合法注音優先，小寫短詞、詞表外單字、無分界的英文／簡拼混合仍可能需要手動選詞或切換。英文後按 ↓／Tab 可要求回到注音；這不承諾華碩所有整句辨識行為。

## 2026-10-11 同步 main 與 model-v5

包含正式 v0.4.0 及其後已合入 main 的 Kneser–Ney side file、Rust tiny-LM／SJTL0001 研究核心。新增詞包、縮寫組字、預測起點及 Enter 提交未完成注音等上游行為。Windows 仍僅提供橫向候選；此階段接入完整核心，縮寫設定由上方後續功能接入，詞包／研究模型設定與下載仍未新增。

C ABI 的 ShanjieOutput 尾端新增 candidate_vertical；候選 constructor 新增方向參數。平台 Output literals 和 Cands 擴充同步更新。映射模型保持 SJWM0002，補足來源 sha 與 KN 狀態，沿用上游 side-file 拒絕／配對規則；沒有 KN 檔時保持上游預設關閉。tiny-LM 核心與既有研究 API 一同編譯，沒有自動啟用 rerank。

包只提供 model-v5＋classes-v3，schema 5 遷移舊模型並保留已存詞庫、學習與外觀。官方來源 SHA-256 與大小記於 MODEL-SOURCES.json。prepare 覆蓋契約與映射等價核對已通過，完整回歸／打包結果見本輪 VERIFICATION.md。

Windows TSF 啟用接入 sinks、設定與原生通道，並透過 `windows/src/tsf/resident.rs` 在背景預載詞庫／模型；不在啟用執行緒同步載入。每個程序保留一個備用核心或尚未完成的載入工作，停用／重新建立服務時重用；核心移交前清除組字和拼音狀態，各服務不共用可變組字。DLL 隨程序常駐，避免背景工作或備用核心被卸載；COM 物件仍正常釋放。第一鍵只接收預載結果，過早輸入會等待同一份工作。OnTestKeyDown 不等待核心，Win 組合鍵在取 context／compartment 前放行。閒置失焦不重置核心或等待學習 mutex。按鍵不查 INI；設定儲存主動通知仍保留，直接改檔需明確重啟或重新啟用。明確重啟清除備用核心並背景重載，不替換已載入 DLL。這是 Windows 宿主生命週期變更，上游 pin 與核心覆蓋不變。

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
| `core/src/engine.rs` 按鍵處理 | Shift／直接中文標點逐鍵遮罩（0.6.0 起每個符號獨立，取代整列開關）與可關閉的單獨聲調；中文標點插入既有組字 | `scripts/prepare-keys.ps1`、`platform/key_options_windows.rs` |
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

Windows 拼音、符號／字碼與聯想操作在 `windows/src/input.rs`；`engine.rs` 協調學習與 epoch。`candidate.rs` 在兩架構 DLL 內直接建立單一 layered Win32 橫向膠囊，淺／深實色、0.5 DIP 細邊線、藍色反白與系統高對比；alpha 只供外角抗鋸齒，沒有毛玻璃、第二背景窗或候選子程序。schema 5 範本、舊版遷移及開關索引見 `profiles/`、`docs/MICROSOFT-IME-DIFFS.md`。命令列與 WinUI 共用 Rust parser／範本／保存；操作預設保留資料與外觀偏好。

設定存於 `%LOCALAPPDATA%/Shanjie/profile.ini`；兩份範本在 `profiles/`。

- `[profile] interaction=windows|shanjie`：方向鍵操作政策。
- `[candidates] anchor=right|left`：啟用已實作的核心候選方向。
- `[candidates] open=down|space|both`：完整組字後的選字開啟鍵；待輸入注音的空白聲調不受影響。
- `[candidates] layout=horizontal`：唯一排列。舊直向／格狀設定遷移；窄工作區以箭頭瀏覽同頁候選。
- `[shortcuts] shift_toggle`、`ctrl_space_toggle`、`shift_space_toggle`、`ctrl_punctuation`：1 開啟、0 關閉。
- `[engine] ranking=chat|formal`、`prediction`、`demote`：上游既有權重／即時預測／降權 API；Windows 範本停用即時預測與 `[input] associations`。
- `[input] escape`、`space_after_commit`、`pinyin_space_converts`，以及候選 Enter、↑、Backspace、翻頁開關：各自選擇已實作的政策。Tab／方向鍵不再展開格狀，完整對照以 `docs/MICROSOFT-IME-DIFFS.md` 為準。

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
