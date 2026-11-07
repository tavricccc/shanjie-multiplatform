# 上游差異

使用 [善解](https://github.com/Nanako0129/shanjie) v0.6.1 Release，實際 commit 以 `UPSTREAM.json` 為準；內建模型組與上游 App 相同，見 `MODEL-SOURCES.json`。各版變更見 [Changelog](Changelog.md)，升版步驟見 [AGENTS.md](AGENTS.md)。

`upstream/shanjie` 的追蹤檔案保持原樣。`scripts/prepare-core.ps1` 每次把上游 core 複製到 `build/core`，再套用下表的變更。**子模組沒改，不代表建出來的核心等於上游**：右側候選範圍、Enter 只提交游標前、注音順序混打等都是核心行為變更，不是改設定就能做到。

## 核心覆蓋

| 上游來源 | Windows 建置副本的變更 | 維護來源 |
| --- | --- | --- |
| `core/src/lib.rs` | 改用 Windows 學習儲存模組，加入唯讀映射模組 | `prepare-core.ps1`、`platform/learning_windows.rs` |
| `core/src/learn_store.rs` | 紀錄格式、欄位驗證沿用上游；檔案處理改用 Windows 原子替換與同步。忘記詞時 flush 失敗仍替換檔案，寫入失敗刪除暫存檔 | `platform/learning_windows.rs` |
| `core/src/learn.rs` | 日期改用 Win32 API；計分與衰減不變 | `platform/clock_windows.rs` |
| `core/src/lm.rs` | 9 個模型陣列與 3 個詞類陣列改為唯讀映射；`.winmap`（SJWM0002）專用讀取與來源雜湊核對 | `prepare-lm.ps1`、`platform/mapped_array.rs`、`platform/mapped_lm.rs` |
| `core/src/engine.rs` 候選 | 右側候選範圍；↑ 循環換字並保留游標；個人詞庫、Big5 篩選、聯想、預測列種類查詢；預測關閉時不因預測建索引（數字串載入模型時仍會建）。篩選或插入候選時，上游記錄容錯讀音的位置對照（`Cands::alt`）跟著項目移動 | `prepare-candidates.ps1`、`platform/candidates_windows.rs`、`platform/input_windows.rs` |
| `core/src/engine.rs` 按鍵 | 逐鍵中文標點遮罩、可關閉的單獨聲調；標點插入既有組字 | `prepare-keys.ps1`、`platform/key_options_windows.rs` |
| `core/src/engine.rs` 組字 | `key` 改為 Windows 入口（上游原函式改名 `key_original`）：字面數字與標點、可選的「標點保留未完成讀音」（預設照上游丟掉）、注音順序混打與容錯、Enter 只提交游標前。上游英文段、英文字的寫法選單、數字串進行中直接交給上游。注音順序混打用的英文詞表上游 0.6.1 已移除，改由 Windows 以上游工具產生、核對 v0.6.0 記錄的雜湊後編進核心，授權檔放在本 repo `LICENSES/` | `prepare-smart-input.ps1`、`platform/smart_input_windows.rs`、`platform/composition_windows.rs` |
| `core/src/engine.rs` 資源 | 常駐快取共用不可變詞庫、詞包、數字表、模型、會意與常談；詞庫快取鍵含所選詞包與 `names-add.tsv`、`digits.tsv`、`acg-add.tsv`、`quotes-add.tsv`、`it-add.tsv`，模型快取鍵含詞類表、KN 與兩個 trigram 表。命中快取時照上游 `load_lm`／`load_tinylm` 重設小模型與英文快取；上游每個引擎各建一份的數字表快取（合法讀音與字詞讀音）改由同一詞庫與模型的引擎共用 | `prepare-resident.ps1`、`platform/resident_windows.rs` |
| `core/src/trigram.rs` | Windows 的 `Mapped` 改用與模型陣列相同的唯讀映射；`.winmap` 旁的 `trigram.sjt3`（書面）與 `trigram-chat.sjt3`（聊天）各自存在才載入，壞檔拒絕 | `prepare-trigram.ps1`、`platform/mapped_windows.rs` |
| `core/src/predict.rs`、`core/src/lib.rs` | 預測索引與詞庫建好後收掉 `Vec` 多配的容量；內容不變 | `prepare-memory.ps1` |
| `core/src/digits.rs` | `Legal::new` 每個音節只展開一次；產生的集合不變 | `prepare-memory.ps1` |
| `core/src/colloquial.rs` | `overlay_only` 由 `HashSet<String>` 改為單一排序字串；查詢結果不變 | `prepare-memory.ps1`、`platform/word_set_windows.rs` |
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
| 中英文之間自動加空格（上游 cjk-spacing，殼層） | `shared/src/spacing.rs`、`tsf/service.rs`、`tsf/keys.rs` |
| 依 App 自動選聊天／書面（上游 profile-choice） | `shared/src/profile.rs` 的 `ranking`、`tsf/resident.rs` |
| 語意校正在背景執行（上游 s7c2b 2.1 的 runner） | `windows/src/engine.rs`、`broker/`、`tsf/rerank.rs` |
| 常駐核心、管道 broker、使用者模型管理 | `windows/src/broker.rs`、`broker/`、`bin/shanjie-core.rs`、`model_store.rs` |
| 註冊、安裝、更新、解除安裝 | `windows/src/registration.rs`、`installer.rs`、`bin/shanjie-manager.rs` |

上游的聊天／正式權重（`shanjie_engine_set_profile`）、詞庫、beam decoder 與 bigram scorer 都沒有改。設定只選擇已實作的功能，不會產生新的核心邏輯。

個人詞典按「讀音＋字詞」替換相同的內建列，其他讀音保留；保留被替換列原有的 `manual` 保護與詞包標記，不把合法重複詞條當成整個詞庫載入失敗。出貨的詞庫／詞包與測試用 `data`、`eval` 由固定 Git commit 匯出，強制停用該次匯出的換行轉換，避免 Windows checkout 的 CRLF 使數字表失效或參考結果比對失敗；不修改上游子模組。

上游數字串開啟時，Windows 的待處理查詢會包含它，讓 Space 仍交給核心決定數字或第一聲；Windows 直接字面、標點與空白插入入口在寫入前先依上游邊界收整未決數字串，維持先輸入數字在前、後輸入符號在後的順序。數字串結束並留下未完成注音時，以上游記錄的完整按鍵順序恢復 Windows 混打與原始讀音紀錄；當次顯示字元只套用到同一英文字母的大小寫，不覆蓋回放的數字。新數字串接手後重建輸出，避免同一按鍵同時顯示在原始字串與數字緩衝區。不關閉上游數字判斷，也不改變混打預設。

## 與上游行為仍不同之處

Windows 直接字面、標點與空白插入會先結束較早的 Shift 英文暫存，避免 `ABC+` 顯示或提交為 `+ABC`；上游英文判斷、Windows 注音順序與兩者並用皆保留原有中英混打功能。

- Windows 範本預設開啟「反引號＋標點」，善解範本仍關閉；缺省判斷依互動模式。schema < 9 的 Windows 設定一次改為開啟，包含舊版手動關閉的值（INI 無法分辨設定來源）；schema 9 後保留關閉。支援 Shift 數字列符號、減號／底線與波浪號；一般 Ctrl 標點的可用按鍵不變。
- 反引號特殊輸入優先於拼音處理；進入時先提交原有拼音組字，再清理拼音與原始讀音暫存。Unicode／Big5 內碼不受拼音攔截，宿主在特殊模式內的選取通知不重寫組字顯示。
- Windows 的全／半形切換立即更新組字顯示，提交與失焦送出採相同轉換；ASCII `!`–`~` 對應 `U+FF01`–`U+FF5E`，空白對應 `U+3000`，中文標點不變。核心保留原字元，切回半形可還原組字中的 ASCII。

- 善解範本（`profiles/shanjie.ini`）與上游 App 一樣開啟上游英文詞表；Windows 範本（微軟注音風格）不開。
- 上游判斷（含兩者並用）單按數字鍵：上游顯示未完成注音（`1` 是 ㄅ，Enter 送出 ㄅ，digits.md A.12）；Windows 顯示並送出數字本身，按鍵仍保留，後續可組成注音（`184` 是 爸）。↓ 與上游相同，開該注音的清單。沒有設定開關。
- Windows 單獨聲調關閉（`standalone_tones=0`）時，閒置按聲調鍵仍輸入字面數字，不開上游數字串。Windows 互動模式開啟時，空組字按聲調鍵一律輸入聲調符號，所以 `3`、`4`、`6`、`7` 不能作為數字串開頭；善解互動模式照上游，開啟英文時由數字串取代。
- Windows 的「→ 右方向鍵」為 `[candidates] right_action=none|page|expand`：預設「無」保留原方向鍵，「下一頁」沿用舊開關，「展開」開啟三欄多列候選。舊 `right_pages=0/1` 分別遷移為 `none/page`；已有新選項時優先保留。兩種互動模式的 ↑↓ 都逐項移動，展開後頂列 ↑ 收回，數字仍依核心反白所在的九項群組選字；預測與聯想不展開。
- 數字鍵盤的 0–9 與 `+ - * / .` 走字面插入，不當注音；組字仍可編輯，候選開啟時 1–9 保留選字。有未完成讀音時先提交讀音再輸入字面字元；內碼輸入中的九宮格數字視為內碼。全形模式下字面字元一併轉為全形。
- 「標點保留未完成讀音」是 Windows 選項（`[input] punctuation_keeps_reading`），新設定檔預設關、與上游相同；從舊版升級的設定檔設為開，維持原本打字結果。
- 語意校正：上游在背景佇列跑完後回主執行緒套用；Windows 由常駐核心在每個會話的背景執行緒跑，TSF 每 15 ms 以不帶序號的請求查詢是否完成（最多 2 秒），完成後才送出有序號的 `Op::Rerank` 套用。這個請求記在重建組字的紀錄裡，核心重啟後照樣重放，候選順序與重啟前相同。
- 依 App 自動：上游用 macOS bundle ID 與選單裡的「這個 App 用聊天設定」；Windows 用執行檔名稱，清單在設定頁直接編輯（`[engine] chat_apps`），沒有輸入法選單項目。
- 中英文自動加空格：上游只在可讀游標前文且學習未暫停時取開頭字元；Windows 在未組字時讀游標前文（「使用游標前文」關閉時，只有這個開關開著才讀）。
- 上游的 App 內「學習暫停（安全輸入／此 App）」與「不要備份選字記憶」是 macOS 殼層功能，Windows 沒有對應。
- 上游 0.6.1 把「依按鍵順序判斷英文」預設改為開；新設定檔照上游，既有設定檔保留原本的值（多半是關）。
- 上游 0.6.1 的「單按 Shift 切換中英」（`shanjie_engine_set_ascii`，每個 App 各自記住）沒有接上；Windows 沿用自己的 Shift 切換。
- 「自動檢查更新」是 Windows 設定頁「關於」的選項，預設開，存在 `%LOCALAPPDATA%\Shanjie\update-auto.txt`，不在 `profile.ini`，上游沒有對應。開啟時由隱藏排程 `ShanjieUpdateCheck` 在登入與之後每 24 小時向 GitHub 查詢，排程同一版一天最多通知一次，開啟設定程式時只要有新版就通知；都不自動下載。
- Windows 的玻璃視窗（設定、安裝、更新卡片）有最低濃度，任何背景最多透出 20%，「玻璃濃度」從這個下限調到不透明，避免深色背景蓋過文字；次要文字也加深。
- 「更新通道」是 Windows 設定頁「關於」的選項（正式版／測試版），存在 `%LOCALAPPDATA%\Shanjie\update-channel.txt`，不在 `profile.ini`，上游沒有對應。預設是正式版；正在執行測試版（版本號帶 `-beta.N`）且尚未選擇時預設測試版。
- 預測列只在第一格標 Tab 符號（上游 header 註解），尚未核對 Windows 候選窗的畫法。

## 測試

`scripts/test-core.ps1` 在獨立的 `build/test` 樹跑上游 core 的全部單元與整合測試，使用內建模型組。Windows 只做以下調整，其餘斷言照上游：

- 符號連結改為複製；不存在的來源以資料夾代替（同樣是「存在但讀不到」）。
- 唯讀映射中的檔案不能刪除或改寫：`tri_tiny` 先釋放模型再刪檔；`load_lm_refuses_a_bad_trigram_table` 跳過。
- 上游 unix 學習檔（權限、FIFO、硬連結）由 Windows 儲存取代：`learn_store` 不跑，`engine_learn` 的三個 unix 測試與 `engine_fuzzy_rerank` 的權限設定不用；`english_codes_through_the_c_abi` 需要 `mkfifo`，跳過。
- Windows 自己的組字政策（標點保留讀音、注音順序混打等）預設關閉時與上游相同，上游斷言不改寫；開啟後的行為由 `platform/` 內的單元測試涵蓋。
