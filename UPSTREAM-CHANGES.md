# 上游差異

使用 [善解](https://github.com/Nanako0129/shanjie)（原作者 Nanako0129；原 repo 已無法存取，現由鏡像 [tavricccc/shanjie-upstream-backup](https://github.com/tavricccc/shanjie-upstream-backup) 提供子模組與模型 Release）v0.6.1 核心，實際 commit 以 `UPSTREAM.json` 為準；內建會意單獨更新為 SJ-SR-8-15M，其餘模型保留該版上游配套，見 `MODEL-SOURCES.json`。各版變更見 [Changelog](Changelog.md)，升版步驟見 [AGENTS.md](AGENTS.md)。

`upstream/shanjie` 的追蹤檔案保持原樣。`scripts/prepare-core.ps1` 每次把上游 core 複製到 `build/core`，再套用下表的變更。**子模組沒改，不代表建出來的核心等於上游**：右側候選範圍、Enter 只提交游標前、原鍵混打等都是核心行為變更，不是改設定就能做到。

## 核心覆蓋

| 上游來源 | Windows 建置副本的變更 | 維護來源 |
| --- | --- | --- |
| `core/src/lib.rs` | 改用 Windows 學習儲存模組，加入唯讀映射模組 | `prepare-core.ps1`、`platform/learning_windows.rs` |
| `core/src/learn_store.rs` | 紀錄格式、欄位驗證沿用上游；檔案處理改用 Windows 原子替換與同步。忘記詞時 flush 失敗仍替換檔案，寫入失敗刪除暫存檔 | `platform/learning_windows.rs` |
| `core/src/learn.rs` | 日期改用 Win32 API；選字預覽以異動日誌撤回更新、新增與淘汰的紀錄；計分與衰減不變 | `platform/clock_windows.rs`、`platform/selection_journal_windows.rs`、`prepare-selection-learning.ps1` |
| `core/src/lm.rs` | 9 個模型陣列與 3 個詞類陣列改為唯讀映射；`.winmap`（SJWM0002）專用讀取與來源雜湊核對 | `prepare-lm.ps1`、`platform/mapped_array.rs`、`platform/mapped_lm.rs` |
| `core/src/engine.rs` 候選 | 右側候選範圍；↑ 循環換字並保留游標；個人詞庫、聯想、預測列種類查詢；相關標點與全半形符號候選擴充；預測關閉時不因預測建索引（數字串載入模型時仍會建）。篩選或插入候選時，上游記錄容錯讀音的位置對照（`Cands::alt`）跟著項目移動；僅在 Windows 輸入選項啟用的標準配置下，數字單元在全形項目後補入對應注音或聲調符號，並以起始範圍修正右側數字轉國字候選的取代區間 | `prepare-candidates.ps1`、`platform/candidates_windows.rs`、`platform/input_windows.rs` |
| `core/src/engine.rs` 按鍵 | 逐鍵中文標點遮罩、可關閉的單獨聲調；標點插入既有組字 | `prepare-keys.ps1`、`platform/key_options_windows.rs` |
| `core/src/engine.rs` 選字學習 | 數字鍵、Enter、滑鼠及預測選字先建立可回復的學習；提交時保存一次，分段 Enter 保留前綴的讀音與選字標記，取消／刪改回復暫存學習 | `prepare-selection-learning.ps1`、`platform/selection_learning_windows.rs`、`platform/selection_journal_windows.rs` |
| `core/src/engine.rs`、`core/src/engine/digit_layer.rs` 組字 | `key` 改為 Windows 入口（上游原函式改名 `key_original`）：字面數字與標點、可選的「標點保留未完成讀音」（預設照上游丟掉）、原鍵混打與容錯、Enter 只提交游標前。上游英文段追蹤實際 `Shift` 按鍵邊界（`run_shifted`），`Caps Lock` 下 `Shift+英文` 後放開 `Shift` 仍可切分正／逆序中文音節；數字層在開啟原鍵混打或 Windows 上游判斷已啟用且「快打時鍵序顛倒不當英文」（`english_reversal_guard`）時將逆序音節判定傳入評分與還原候選（原生上游測試環境維持關閉）。原鍵混打用的英文詞表上游 0.6.1 已移除，改由 Windows 以上游工具產生、核對 v0.6.0 記錄的雜湊後編進核心，授權檔放在本 repo `LICENSES/` | `prepare-smart-input.ps1`、`platform/smart_input_windows.rs`、`platform/composition_windows.rs` |
| `core/src/engine.rs` 資源 | 常駐快取共用不可變詞庫、詞包、數字表、模型、會意與常談；詞庫快取鍵含所選詞包與 `names-add.tsv`、`digits.tsv`、`acg-add.tsv`、`quotes-add.tsv`、`it-add.tsv`，模型快取鍵含詞類表、KN 與兩個 trigram 表。命中快取時照上游 `load_lm`／`load_tinylm` 重設小模型與英文快取；上游每個引擎各建一份的數字表快取（合法讀音與字詞讀音）改由同一詞庫與模型的引擎共用 | `prepare-resident.ps1`、`platform/resident_windows.rs` |
| `core/src/trigram.rs` | Windows 的 `Mapped` 改用與模型陣列相同的唯讀映射；`.winmap` 旁的 `trigram.sjt3`（書面）與 `trigram-chat.sjt3`（聊天）各自存在才載入，壞檔拒絕 | `prepare-trigram.ps1`、`platform/mapped_windows.rs` |
| `core/src/predict.rs`、`core/src/lib.rs` | 預測索引與詞庫建好後收掉 `Vec` 多配的容量；內容不變 | `prepare-memory.ps1` |
| `core/src/digits.rs` | `Legal::new` 每個音節只展開一次，產生的集合不變；新增混打專用的欄位正規化查詢 `Legal::tail_units_mixed` 與 `Scorer::decide_mixed`（混打關閉時與上游一致） | `prepare-memory.ps1`、`prepare-smart-input.ps1` |
| `core/src/colloquial.rs` | `overlay_only` 由 `HashSet<String>` 改為單一排序字串；常談配對接受 SJ-SR-7-15M 與 SJ-SR-8-15M 的確切雜湊，保留既有聊天三檔係數 | `prepare-memory.ps1`、`platform/word_set_windows.rs`、`prepare-core.ps1` |
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
| 關於頁的 GitHub 貢獻者名單與原生預覽卡片、可換行與捲動的本機模型配置選單（不改模型內容或選取／儲存政策） | `ui/Controls/ContributorsView.cs`、`ModelPicker.xaml`、`ui/Assets/contributors.json`；資料維護見 `docs/CONTRIBUTORS.md` |
| 快速設定（第一次開啟設定時一個畫面一題的範本、細節與動畫示範；選範本時寫一份新的 profile 並備份舊檔；標記檔 `quick-setup.txt` 在 `profile.ini` 之外，上游沒有這個流程） | `ui/QuickSetupView.xaml`、`ui/Setup/`、`ui/Controls/TypingDemo.cs`、`ChoiceCards.cs`、`StepProgress.cs`、`FinishMark.cs` |
| 中英文之間自動加空格（上游 cjk-spacing，殼層） | `shared/src/spacing.rs`、`tsf/service.rs`、`tsf/keys.rs` |
| 依 App 自動選聊天／書面（上游 profile-choice） | `shared/src/profile.rs` 的 `ranking`、`tsf/resident.rs` |
| 語意校正在背景執行（上游 s7c2b 2.1 的 runner） | `windows/src/engine.rs`、`broker/`、`tsf/rerank.rs` |
| 常駐核心、管道 broker、使用者模型管理 | `windows/src/broker.rs`、`broker/`、`bin/shanjie-core.rs`、`model_store.rs` |
| 註冊、安裝、更新、解除安裝 | `windows/src/registration.rs`、`installer.rs`、`bin/shanjie-manager.rs` |

上游的聊天／正式權重（`shanjie_engine_set_profile`）、詞庫、beam decoder 與 bigram scorer 都沒有改。設定只選擇已實作的功能，不會產生新的核心邏輯。

工作列模式按鈕與 TSF 服務採用 apartment marshaling，讓跨 apartment 的 COM 呼叫回到建立物件的執行緒，避免跨執行緒存取 `Rc`、`RefCell` 與 TSF 介面。

個人詞典按「讀音＋字詞」替換相同的內建列，其他讀音保留；保留被替換列原有的 `manual` 保護與詞包標記，不把合法重複詞條當成整個詞庫載入失敗。出貨的詞庫／詞包與測試用 `data`、`eval` 由固定 Git commit 匯出，強制停用該次匯出的換行轉換，避免 Windows checkout 的 CRLF 使數字表失效或參考結果比對失敗；不修改上游子模組。

上游數字串開啟時，Windows 的待處理查詢會包含它，讓 Space 仍交給核心決定數字或第一聲。開啟原鍵混打或開啟上游判斷且「快打時鍵序顛倒不當英文」（`english_reversal_guard`）時，數字層以欄位正規化（`Legal::tail_units_mixed`）與介音橋接評估所有含數字／符號之逆序音節（包含全在數字表內的 `823` = `ㄉㄚˇ`、`813` = `ㄅㄚˇ`，以及跨字母的 `8d ` = `ㄎㄚ`、`8w ` = `ㄊㄚ`、`9x6` = `ㄌㄞˊ`、`9w4` = `ㄊㄞˋ`、`u0r3` = `ㄐㄧㄢˇ`）。原鍵混打的第一聲與二至輕聲整段逆序音節（含與英文詞表重疊之無 `Shift` 三鍵第一聲，如 `jig ` = `ㄕㄨㄛ`、`flu ` = `ㄑㄧㄠ`）、未完成字面片段或 `Shift` 明確英文前綴後的尾綴正／逆序音節皆依鍵盤配置欄位順序還原，並在兩側候選保留原始按鍵字面（`Shift+字母` 明確英文與非完整音節之英文詞表單字在空白鍵維持字面英文且不被拆字尾，也不跨過最後一個 `Shift` 字元吞食英文前綴）。數字串結束並留下未完成注音時，以上游記錄的完整按鍵順序恢復原鍵混打與原始讀音紀錄；當次顯示字元只套用到同一英文字母的大小寫，不覆蓋回放的數字。新數字串接手後重建輸出，避免同一按鍵同時顯示在原始字串與數字緩衝區。不關閉上游數字判斷，也不改變混打預設；混打關閉時不因已儲存的快打還原開關而啟用數字倒序。

Windows 直接字面、標點與空白插入前先收整未決數字串，維持數字與後續符號的輸入順序。

Windows 直接字面、標點與空白插入先結束較早的 Shift 英文暫存，維持 `ABC+` 的輸入順序。

學習預覽只記錄本次更新、新增與淘汰紀錄的撤回資訊，不複製整份學習庫或重建其索引。分段 Enter 最多保留 128 音節的已提交前綴與必要左文脈；累計超過時先結算已提交部分，再開始下一段，取消仍只撤回尚未結算的學習。

## 與上游行為仍不同之處

- 內建會意採用上游另行發布的 SJ-SR-8-15M，權重與檔頭係數原樣保留，核心仍固定 v0.6.1。未修改的上游公開附件從使用者提供的模型備份取回，納入 `models/sj-sr-8-15m/`；`MODEL-SOURCES.json` 的 `source` 指向本機來源、`sha256` 核對實際附件、`pinned_sha256` 核對核心 pin 原有的第 7 版記錄。常談對第 7、8 版皆生效，沿用原有聊天係數；未宣稱重現上游第 8 版的完整核心或準確率評測。

- 啟用「記憶選字」時，Windows 候選確認立即建立暫存記憶；改選、刪字或取消會撤回尚未確認的記憶，提交時寫入一次。Enter 只提交游標前時，已送出前綴的讀音與選字標記保留到剩餘組字確認，讓「楓之谷」可整詞記憶，不需先按 End。失焦提交只在立即取得學習鎖時保存，其他會話正在寫入時先送出文字、略過該次記憶，避免切換視窗等待。學習關閉時不記憶，未套用 Windows 選項的核心維持上游只在提交學習的政策。
- 右側候選沿用上游的自學新詞與模糊音清單政策：同長度的新詞排在詞庫詞條前，低於啟用權重的新詞仍可列出並忘記；選取模糊音項目時改用其詞庫讀音。私人詞典載入與共用詞庫快取都保留詞包的 OOV 前綴範圍，供後續模型載入使用。
- Windows 範本預設開啟「反引號＋標點」，善解範本仍關閉；缺省判斷依互動模式。schema < 9 的 Windows 設定一次改為開啟，包含舊版手動關閉的值（INI 無法分辨設定來源）；schema 9 後保留關閉。支援 Shift 數字列符號、減號／底線與波浪號；一般 Ctrl 標點的可用按鍵不變。「反引號＋標點」對應出的符號會送入核心可編輯組字，可開候選替換或按 Esc 取消；Unicode 內碼與符號鍵盤仍直接提交。
- Windows 輸入選項啟用相關符號候選：括號、引號、斜線、直線／箭頭、尖括號／不等號、破折號、底線與其他鍵盤標點可選同類及全／半形符號，左右候選位置一致，原符號優先且去重。反引號＋Shift＋反斜線與 Ctrl＋Shift＋反斜線輸入「｜」，可再選 ↑↓ 等箭頭。未套用 Windows 輸入選項的核心仍保留上游候選表；明確呼叫 `set_punctuation` 會改用傳入的完整表。
- 反引號特殊輸入優先於拼音處理；進入時先提交原有拼音組字，再清理拼音與原始讀音暫存。Unicode 內碼不受拼音攔截，宿主在特殊模式內的選取通知不重寫組字顯示。
- Windows 的全／半形切換立即更新組字顯示，提交與失焦送出採相同轉換；ASCII `!`–`~` 對應 `U+FF01`–`U+FF5E`，空白對應 `U+3000`，中文標點不變。核心保留原字元，切回半形可還原組字中的 ASCII。

- 善解範本（`profiles/shanjie.ini`）與上游 App 一樣開啟上游英文詞表；Windows 範本（微軟注音風格）不開。
- 上游判斷單按數字鍵：上游顯示未完成注音（`1` 是 ㄅ，Enter 與 ↓ 送出／展開 ㄅ，digits.md A.12）；Windows 顯示並送出數字本身，按鍵仍保留，後續可組成注音（`184` 是 爸）。套用 Windows 輸入選項的標準（大千）配置下，單按 `0`–`9` 按 ↓ 與各數字／符號單元（`0`–`9`、`.`、`/`、`-`）的左右候選與 ↑ 循環，皆會在全形數字後附上對應的大千注音或聲調符號；聲韻母若詞庫與模型有條目即轉成可再改選漢字的真實讀音單元，聲調或缺乏條目者則以符號單元插入，原有的漢字轉換候選（`DigitAlt::Syllable`）與左右數字候選的預測列維持可用（選取或忘記預測項時保留原數字與候選錨點），同時修正右側候選使用漢字轉換項的取代區間。未套用 Windows 選項的核心與倚天配置維持上游行為。沒有設定開關。
- Windows 單獨聲調關閉（`standalone_tones=0`）時，閒置按聲調鍵仍輸入字面數字，不開上游數字串。Windows 互動模式開啟時，空組字按聲調鍵一律輸入聲調符號，所以 `3`、`4`、`6`、`7` 不能作為數字串開頭；善解互動模式照上游，開啟英文時由數字串取代。
- Windows 的「→ 右方向鍵」為 `[candidates] right_action=none|page|expand`：預設「無」保留原方向鍵，「下一頁」沿用舊開關，「展開」開啟三欄多列候選。舊 `right_pages=0/1` 分別遷移為 `none/page`；已有新選項時優先保留。兩種互動模式收合時 ↑↓ 逐項移動；展開時沿同一直排 ±3 移動，最上列 ↑ 收回，末列向下停在原位。核心未套用 Windows 選項時保留上游九欄；輸出視窗與數字仍依反白所在的九項群組選字。預測與聯想不展開。
- 數字鍵盤的 0–9 與 `+ - * / .` 走字面插入，不當注音；組字仍可編輯，候選開啟時 1–9 保留選字。有未完成讀音時先提交讀音再輸入字面字元；內碼輸入中的九宮格數字視為內碼。全形模式下字面字元一併轉為全形。
- 「標點保留未完成讀音」是 Windows 選項（`[input] punctuation_keeps_reading`），新設定檔預設關、與上游相同；從舊版升級的設定檔設為開，維持原本打字結果。
- 語意校正：上游在背景佇列跑完後回主執行緒套用；Windows 由常駐核心在每個會話的背景執行緒跑，TSF 每 15 ms 以不帶序號的請求查詢是否完成（最多 2 秒），完成後才送出有序號的 `Op::Rerank` 套用。這個請求記在重建組字的紀錄裡，核心重啟後照樣重放，候選順序與重啟前相同。
- 依 App 自動：上游用 macOS bundle ID 與選單裡的「這個 App 用聊天設定」；Windows 用執行檔名稱，清單在設定頁直接編輯（`[engine] chat_apps`），沒有輸入法選單項目。
- 中英文自動加空格：上游只在可讀游標前文且學習未暫停時取開頭字元；Windows 在未組字時讀游標前文（「使用游標前文」關閉時，只有這個開關開著才讀）。
- 上游的 App 內「學習暫停（安全輸入／此 App）」與「不要備份選字記憶」是 macOS 殼層功能，Windows 沒有對應。
- 上游 0.6.1 把「依按鍵順序判斷英文」預設改為開；新設定檔照上游，既有設定檔保留原本的值（多半是關）。
- 上游 0.6.1 的「單按 Shift 切換中英」（`shanjie_engine_set_ascii`，每個 App 各自記住）沒有接上；Windows 沿用自己的 Shift 切換，兩個範本預設都開。
- 組字面板（`[candidates] reading_panel`）是 Windows 專屬，**原鍵混打需要它**，兩個範本預設關閉。關閉時與上游相同：組字只在 App 內，沒有候選也沒有提示就不顯示面板。開啟後在候選窗位置顯示整句組字，好處是游標移進句子時會標出游標所在的字，使用者可依需要開啟；游標在句尾時不標，上游沒有這個面板。
- 原鍵混打（`[input] mixed_input`，鍵名不變）是 Windows 專屬的混打方式：按鍵排不成合法注音就當英文，原始按鍵留在組字區，面板提示注音，聲調完成後轉中文，Enter 保留原文。選用時組字面板固定開啟，不改寫 `reading_panel` 的值，離開這個模式就回到原值。
- 候選中 Tab：上游是關閉候選並繼續處理這個鍵（`core` 的兜底分支）。Windows 操作預設的 Tab 與 → 共用 `[candidates] right_action`，範本預設 `expand`，對齊微軟注音；善解操作預設的 Tab 與上游相同。
- 候選窗方向：上游預設橫排，可在設定改直排（`shanjie_engine_set_candidate_vertical`），Linux 前端也可選；Windows 所有範本一律直式，沒有設定也沒有橫向繪製，候選的直式畫法是 Windows 自己的，沒有接上游這個函式。舊設定的 `[candidates] layout` 讀入時丟棄（任何值都無害），下次儲存就消失。`profiles/*.ini` 保留這一行並加註解，因為 Linux 前端會讀。
- 範本預設：兩個範本的混打方式都是上游判斷（`upstream_english=1`、`mixed_input=0`），上游判斷不支援的配置（倚天、拼音）沒有作用。善解範本改成與上游相同：`space_operates=1`（空白開候選、候選中換頁）、`shift_toggle=1`、`learning=1`、`caps_lock_english=1`、`forget_candidate=1`（上游 ⌘⌫，Windows 是 Ctrl＋Shift＋Backspace）。只影響新設定檔與缺少的鍵，已存的值不改。混打方式是單選（關閉、上游判斷、原鍵混打，`mixed_input` 與 `upstream_english` 最多一個為 1）：0.9.0 到 0.15.x 的「兩者並用」已移除，原鍵混打開著的設定檔讀入時一律把 `upstream_english` 設 0（兩者並用變成原鍵混打，它原本就先判斷；缺少 `upstream_english` 時也不會從範本補成 1）。
- 善解範本仍與上游不同：`add_word=1`（雙 Ctrl 加詞，上游沒有）；`glass_tint=42`（上游 0，Windows 玻璃視窗有最低濃度，候選窗不讀它）；上游「瀏覽器第一個字不顯示預測」與英文段提示「空白：英文斷開」Windows 沒有。
- 「自動檢查更新」是 Windows 設定頁「關於」的選項，預設開，存在 `%LOCALAPPDATA%\Shanjie\update-auto.txt`，不在 `profile.ini`，上游沒有對應。開啟時由隱藏排程 `ShanjieUpdateCheck` 在登入與之後每 24 小時向 GitHub 查詢，排程同一版一天最多通知一次，開啟設定程式時只要有新版就通知；都不自動下載。
- Windows 的玻璃視窗（設定、安裝、更新卡片）有最低濃度，任何背景最多透出 20%，「玻璃濃度」從這個下限調到不透明，避免深色背景蓋過文字；次要文字也加深。
- 「更新通道」是 Windows 設定頁「關於」的選項（正式版／測試版），存在 `%LOCALAPPDATA%\Shanjie\update-channel.txt`，不在 `profile.ini`，上游沒有對應。預設是正式版；正在執行測試版（版本號帶 `-beta.N`）且尚未選擇時預設測試版。
- 預測列只在第一格標 Tab 符號（上游 header 註解），尚未核對 Windows 候選窗的畫法。

## 測試

`scripts/test-core.ps1` 在獨立的 `build/test` 樹跑上游 core 的全部單元與整合測試，使用核心 pin 的原始模型組（會意第 7 版），保留固定候選結果的斷言；內建第 8 版由模型準備時的載入／常談配對檢查與 `shanjie-model-check builtin` 的原生輸入／重排檢查驗證。Windows 只做以下調整，其餘斷言照上游：

- 符號連結改為複製；不存在的來源以資料夾代替（同樣是「存在但讀不到」）。
- 唯讀映射中的檔案不能刪除或改寫：`tri_tiny` 先釋放模型再刪檔；`load_lm_refuses_a_bad_trigram_table` 跳過。
- 上游 unix 學習檔（權限、FIFO、硬連結）由 Windows 儲存取代：`learn_store` 不跑，`engine_learn` 的三個 unix 測試與 `engine_fuzzy_rerank` 的權限設定不用；`english_codes_through_the_c_abi` 需要 `mkfifo`，跳過。
- Windows 自己的組字政策（標點保留讀音、原鍵混打等）預設關閉時與上游相同，上游斷言不改寫；開啟後的行為由 `platform/` 內的單元測試涵蓋。
