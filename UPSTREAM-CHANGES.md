# 上游差異

一般右錨點漢字候選沿用上游預測列的 reading／start／key、變體排序及模糊讀音索引，數字鍵解成漢字音節後也保留原數字候選。選取、忘記與原有 mixed／數字 token／倚天分支仍各按既有跨度處理。Windows 操作範本繼續採自己的 caller 政策，沒有切換到上游 scheme1；被動預測首格補上 Tab 提示。

上游 learning status getter 已接到 Windows Snapshot、Named Pipe、候選提示、語言列及設定頁。學習暫停在 TSF 讀前文之前判斷 Windows scope、原生密碼欄位與宿主名稱，並在 local／resident 核心清除舊前文及過期語意工作；不改使用者設定或學習檔格式。這是對應 Windows 邊界的移植，並非直接複製 macOS API 或 bundle ID。

自有 `shanjie-core.exe` 啟動先套用 HKCU 的 WER 排除，失敗則沿用上游拒絕啟動政策；不對載入 TIP 的第三方 App 設定 WER。排除不隨程序退出而刪除，以供同名舊版本繼續使用。完全移除安裝目錄時移除排除；仍有在用舊版或待清理檔案時保留，後續檔案清理不以 SYSTEM 的 HKCU 代替原使用者刪除。測試身分使用隔離 backend，不修改實際 WER 政策。

原始上游與 shipping overlay 的 CLI／rowstats 以相同 judge、模型、profile 及詞包逐 byte 比對；ACG＋IT 與本 fork 四詞包配置分開比較。模型效能工具分開固定核心換模型、前後版各自核心與配套，以及解碼預熱實驗；hash 驗證不在量測程序內執行，報告實際融合與三檔參數，不強制配對。首次開啟指新程序，OS 檔案快取未清除。

G05 改列特殊測試，只在明確指定 `full.core.rowstats` 時執行；不屬 CI、FAST、AFFECTED、FULL、Beta／正式 RELEASE 或手動 release verify 的預設集合，Release 收據也不要求它。本輪在固定點 `006c8f1d` 完整實跑 828.35 秒，保留獨立 raw／overlay 與負向控制。歷史模型比較與解碼預熱仍按變更範圍獨立安排。

Windows 設定程式以未封裝桌面身分執行。從帶有套件身分的宿主啟動時，入口會用 Windows 桌面程序啟動政策重新啟動自身，保留參數與結束碼，避免 WinUI 將宿主的 PRI 當成善解資源；只影響這次程序啟動。一般啟動失敗會將例外堆疊、HRESULT 與資源位置寫入使用者 `Shanjie/Logs`，對話框顯示日誌位置，不記錄詞典、學習或輸入內容。

模型清單與附件優先讀取官方倉庫，讀取失敗才切換鏡像；本機驗證同時接受原上游與鏡像的合法收據，保留以來源資訊計算的配套 ID，不改寫舊收據。原鍵混打直接插入標點時保留畫面上已有的純數字段，包括尚未形成小數的 `1%`。

設定的內建配套提示按實際元件版本判斷：同版、較新或版本無法比較的已下載配套不被建議改用內建。相容索引只補充已驗證配套與備註；索引落後或不可讀時，仍保留有效安裝收據，直接從原作者發布清單偵測新模型與同標籤雜湊更新，未確認的配套標示待驗。索引問題不會觸發鏡像切換。

原鍵混打的純數字暫存於左／右／Home／End 移動游標前，先轉成可編輯字面單元，保留畫面上的數字與後續插入、刪除及提交。上游判斷啟用 Windows 單數字顯示政策時，游標移動同樣保留所見數字；完成注音與明確開啟注音候選的路徑各自維持。

設定列、自訂開關與分段選擇器在首次載入時同步停用外觀；相依選項停用時保留其儲存值，恢復可用後沿用原選擇。

Windows 外觀新增 `[appearance] accent=default|windows|custom`、`accent_color=#RRGGBB`、`pure_black=0|1` 與 `animation=system|on|off`。既有預設藍與跟隨系統動畫保留；預設藍維持白色選取文字，其他強調色依明度選黑或白，提示文字另維持對比。深色純黑使用不透明黑底並停用玻璃混色，高對比優先採系統色。設定草稿只在設定程序預覽，儲存後沿用既有通知套用到選字框與加詞浮層；外觀屬個人設定，不由操作範本覆寫或匯出。強制動畫只改善解的繪圖與動畫政策，不改 Windows 選項。

加詞常駐輔助程式定期核對已放開的非 Ctrl 按鍵，避免漏收放開事件後永久將雙 Ctrl 判為組合鍵；Ctrl 的按放序列仍由手勢處理。有限日誌記錄手勢阻擋、狀態核對、取詞開始／逾時與設定 Shell 啟動失敗等階段，不記錄按鍵值、反白字詞或文件內容。此回復路徑已隔離回歸，尚不代表已確認使用者遇到的所有浮層失效都出於同一原因。

使用 [善解](https://github.com/Nanako0129/shanjie)（原作者 Nanako0129）主線提交 `de7d69c5`（v0.9.1 之後 38 個提交）的核心與完整詞庫。詞庫新增「為啥」（ㄨㄟˋ ㄕㄚˊ）、「不足為外人道」（ㄅㄨˋ ㄗㄨˊ ㄨㄟˋ ㄨㄞˋ ㄖㄣˊ ㄉㄠˋ）及「部署」（ㄅㄨˋ ㄕㄨˇ）的讀音，並含口語變調與疊字陰平 `colloq` 讀音及教育部兩部辭典使用說明。沿用 v0.9.0 的完整模型配套，包括會意 `SJ-SR-10-31M`（格式 `SJTL0006`、`L=62`、`K=4` 與 `[SEP]` 邊界標記）及 `bigram`、`classes`、`kn`、`trigram` 和 `colloquial`。固定提交以 `UPSTREAM.json` 為準。模型清單與 Windows UI 同步支援 `SJTL0001`–`SJTL0006` 及 KN 配對雜湊解析。子模組初始化、模型清單與附件均優先官方，無法讀取才改用 [鏡像](https://github.com/tavricccc/shanjie-upstream-backup)；附件大小與 SHA-256 照常核對，既有收據的來源與 ID 不重寫。同名模型以實際讀取的模型名稱識別，跨來源／配套 ID 仍顯示已下載；附件或 release 有新版時保留舊版並提供更新，不因同名略過更新。各版變更見 [Changelog](Changelog.md)。

Windows 詞庫匯出先套用 #123，恢復「系上」（ㄒㄧˋ ㄕㄤˋ）與「挨到」（ㄞ ㄉㄠˋ），再套用 #125 從疊加層移除 533 個會擠掉常見讀音的詞，共 549 筆讀音。出貨的 `overlay-add.tsv` 與原作者提交 `5ab7307ad11c` 逐位元相同，測試匯出同步採用相符的數字、解碼與預測基準。詞庫由 Windows 直接載入，生成工具與裁剪清單由上游維護。 [上游 PR #123](https://github.com/Nanako0129/shanjie/pull/123) · [上游 PR #125](https://github.com/Nanako0129/shanjie/pull/125)

Windows 採用原作者提交 `dfbeb954bb63` 的 8,441 詞專名層與來源紀錄，包含新增的「浮洲站」、「善解輸入法」及「小檜溪」讀音。出貨授權檔與來源標示帶入 47 個政府開放資料集的資料集編號與機關，詞庫測試採用相符的數字、預測及解碼基準。專名層與原有疊加詞庫一起載入。 [上游 PR #131](https://github.com/Nanako0129/shanjie/pull/131)

[上游 PR #102](https://github.com/Nanako0129/shanjie/pull/102) 的混合詞解碼、英文補全、`nopredict` 詞表欄位及 `^en:` 學習鍵隨固定核心與詞庫取得。Windows 上游判斷路徑保留實際 Shift 邊界與英文大小寫，已進入英文預測列時數字鍵用於選字。原鍵混打及倚天沿用既有路徑。兩套操作範本保留原預測預設。英文預測列出現時顯示 Tab，沒有候選時回到英文提示。右錨點的混合詞及還原英文字候選沿用完整 token 與後接音節的範圍。

`^en:` 共用既有學習 TSV、Windows 原子替換與跨行程協調。舊版略過此鍵，完整改寫時會移除新鍵紀錄，其他紀錄沿用原格式。原上游 [#103](https://github.com/Nanako0129/shanjie/pull/103)、[#104](https://github.com/Nanako0129/shanjie/pull/104)、[#105](https://github.com/Nanako0129/shanjie/pull/105) 的 Swift timing、macOS 重排優先度及 Orca 前文只保留在子模組。

`upstream/shanjie` 的追蹤檔案保持原樣。`scripts/prepare-core.ps1` 每次把上游 core 複製到 `build/core`，再套用下表的變更。**子模組沒改，不代表建出來的核心等於上游**：右側候選範圍、Enter 只提交游標前、原鍵混打等都是核心行為變更，不是改設定就能做到。

新版核心保留上下文重現排序（游標前最後 64 個 Unicode scalar 的漢字相鄰對）；長度至少 32 時，語意校正候選池另依上游係數 2 加權。TSF 在開始組字時讀取最多 256 個 UTF-16 單元，從最後一個換行或 NUL 之後取最後 64 個 scalar；沿用「使用游標前文」開關，不儲存這段文字。macOS 殼層先裁 64 個 grapheme，Windows 直接採核心的 scalar 上限，保留既有 App 政策，未新增 macOS 終端機 bundle ID 規則。Windows 按鍵入口只消耗一次前文的新鮮度，包含純注音與原鍵混打的直接處理路徑，避免內部回放提早清掉前文。數字逆序的 RHO=7／符號 RHO=4 評分、待歸鍵與聲調提早按下的還原，以及 `share`／`new_shared`／`shared_free` 資源共用 API 均保留。

讀音解碼沿用上游的唯一讀音豁免規則、完整 `std-exempt.tsv`（含「嗨」等新增項目）與音節歸零（`with_syllable_norm`，`+rn`），並完整整合變調與口語讀音（`tone3`、`shanjie_engine_set_tone3`，含三聲變調、口語輕聲與疊字陰平 `colloq` 列，右側候選同步查詢 `entries_with`）與「這／那／哪」接量詞合音（`sandhi-add.tsv`）；語意工作使用上游新版 `SJTL0006` 長上下文鍵（`key_for_len`）、候選權重、排序與結果對照，不在 Windows 重寫評分或序列化格式。

Windows 詞包切換沿用原作者 #59 的共享引擎資源及 #61 的「建置完成等組字結束」策略。本地模式由 TSF 執行緒取得共享資源；Named Pipe 模式由常駐核心中原工作階段的執行緒取得。兩種模式的背景工作都經 Windows 載入流程重新合併所選詞包與個人詞典。相同版本的 `.winmap`、TinyLM 與常談可共用，數字、讀音及詞庫相關快取按新詞庫建立或命中。`learning.tsv` 沿用 Windows 既有 mutex 與重載流程。這條 Windows 路徑保留個人詞典與單音節詞包標記，沒有直接用不帶個人詞典參數的 `shanjie_engine_new_shared`。模型版本或來源改變時走完整建置。

Windows 兩套範本、缺少設定鍵的舊 profile 及 WinUI 都以 `[input] english_overwrite=0` 為預設，明確儲存的值保留。Shift 英文段後放開 Shift 可切回正序或倒序注音，含單符號或第一聲；英文段內 Shift＋空白保留空白，組字或候選介面仍標示英文緩衝。聲調提早的復原也涵蓋原鍵混打及進過數字層的按鍵。RHO 保持 7：句首 `9y4`、`20u4` 仍可能被當成「數字＋注音」。第一聲空白保持送達順序，`1u 0 ` 可輸入「逼安」，所以不將它回溯為 `1u0 `。

上游已提供的 `Legal::new` 音節去重改用原版，移除 Windows 重複覆蓋。評測 judge v2 與新增驗收表隨固定上游測試來源匯出；其教育部資料僅用於評測，不加入產品詞庫。

評測的 `eval/accept/*.doubt.tsv` 只記錄盲判無法確認正解的回報，包含錯字回報清單；不改動正解或分數，也不加入產品詞庫。

語意模型沿用上游的可攜矩陣運算：x64 依 CPU 能力選 AVX-512F、AVX2 或純量核心，x86 使用純量核心。三條路徑採固定運算順序，結果逐位元一致。模型升級為 `SJ-SR-10-31M`（`SJTL0006`），聊天與書面三檔融合參數及 `PAIRED_TINYLM_SHA256` 均沿用上游。Windows 背景語意校正沿用每工作階段獨立引擎與既有 Named Pipe／TSF 架構，未移植上游單一引擎的 `Poll`/`Polled`/`Owner` 協定與 MSI 安裝程式；長前文的校正耗時可超過 Enter 的 100 ms 等待，此時直接提交校正前的文字；僅補齊不重新解碼按鍵保留既有重排工作、專用 `Shanjie rerank` 背景執行緒維持 `THREAD_PRIORITY_NORMAL`（對應上游 macOS #104 的 `userInitiated` QoS 調整，但 Windows 僅調整該專用執行緒並維持 `NORMAL_PRIORITY_CLASS`、其餘執行緒與 Enter 100 ms 上限不變，避免高於前景輸入執行緒搶佔打字）、候選窗開啟時不提前消費結果、非同步 Edit Session 遭拒時維持計時器重試，以及 `RerankReady` 非阻塞查詢與 200 ms 期限。macOS 的 Option 全形標點（rule 1o）於核心隨上游保留測試，Windows 前端依現有修飾鍵規則不改寫 Alt 行為。

## 核心覆蓋

| 上游來源 | Windows 建置副本的變更 | 維護來源 |
| --- | --- | --- |
| `core/src/lib.rs` | 改用 Windows 學習儲存模組，加入唯讀映射模組 | `prepare-core.ps1`、`platform/learning_windows.rs` |
| `core/src/learn_store.rs` | 紀錄格式、欄位驗證沿用上游；檔案處理改用 Windows 原子替換與同步。忘記詞時 flush 失敗仍替換檔案，寫入失敗刪除暫存檔 | `platform/learning_windows.rs` |
| `core/src/learn.rs` | 選字預覽以異動日誌撤回更新、新增與淘汰的紀錄；計分與衰減不變 | `platform/selection_journal_windows.rs`、`prepare-selection-learning.ps1` |
| `core/src/lm.rs` | 9 個模型陣列與 3 個詞類陣列改為唯讀映射；`.winmap`（SJWM0002）專用讀取與來源雜湊核對 | `prepare-lm.ps1`、`platform/mapped_array.rs`、`platform/mapped_lm.rs` |
| `core/src/engine.rs` 候選 | 右側候選範圍（同步傳入 `t3` 位置表並改用 `entries_with` 支援變調、口語輕聲與疊字 `colloq` 候選與讀音還原）；↑ 循環換字並保留游標；個人詞庫、聯想、預測列種類查詢；相關標點與全半形符號候選擴充；預測關閉時不因預測建索引（數字串載入模型時仍會建）。篩選或插入候選時，上游記錄容錯讀音的位置對照（`Cands::alt`）跟著項目移動；僅在 Windows 輸入選項啟用的標準配置下，數字單元在全形項目後補入對應注音或聲調符號，並以起始範圍修正右側數字轉國字候選的取代區間 | `prepare-candidates.ps1`、`platform/candidates_windows.rs`、`platform/input_windows.rs` |
| `core/src/engine.rs` 按鍵 | 逐鍵中文標點遮罩、可關閉的單獨聲調；標點插入既有組字。純注音輸出模式在英文／數字判斷前將按鍵映射為不可轉換的注音字面單元，沿用編輯與提交路徑，不建立預測或選字學習 | `prepare-keys.ps1`、`prepare-smart-input.ps1`、`platform/key_options_windows.rs`、`platform/smart_input_windows.rs` |
| `core/src/engine.rs` 選字學習 | 數字鍵、Enter、滑鼠及預測選字先建立可回復的學習；提交時保存一次，分段 Enter 保留前綴的讀音與選字標記，取消／刪改回復暫存學習 | `prepare-selection-learning.ps1`、`platform/selection_learning_windows.rs`、`platform/selection_journal_windows.rs` |
| `core/src/engine.rs`、`core/src/engine/digit_layer.rs` 組字 | `key` 改為 Windows 入口（上游原函式改名 `key_original`）：字面數字與標點、可選的「標點保留未完成讀音」（預設照上游丟掉）、原鍵混打與容錯、Enter 只提交游標前。`shadow_push` 同步將 Windows 顯示字元寫入 `shadow_key` 與按鍵紀錄 `typed`，上游判斷模式完整保留按鍵紀錄、Esc 注音回放與收尾鍵英文／注音自動比較（`auto_english`）；插入字面／標點時一併清除前一個自動英文復原標記。上游英文段追蹤實際 `Shift` 按鍵邊界（`run_shifted`），`Caps Lock` 下 `Shift+英文` 後放開 `Shift` 仍可切分正／逆序中文音節；數字層在原鍵混打時保留 Windows 的欄位正規化判定且維持不接受變調音節；原鍵混打支援非純數字之三聲變調未收錄音節。上游判斷模式沿用上游逆序評分、待歸鍵與聲調提早還原，受 `english_reversal_guard` 控制。原鍵混打用的英文詞表上游 0.6.1 已移除，改由 Windows 以上游工具產生、核對 v0.6.0 記錄的雜湊後編進核心，授權檔放在本 repo `LICENSES/` | `prepare-smart-input.ps1`、`platform/smart_input_windows.rs`、`platform/composition_windows.rs` |
| `core/src/engine.rs` 資源 | 常駐快取共用不可變詞庫、詞包、數字表、收尾鍵英文比較表（`english.tsv`）、模型、會意與常談；詞庫快取鍵含所選詞包與 `names-add.tsv`、`digits.tsv`、`english.tsv`、`acg-add.tsv`、`quotes-add.tsv`、`it-add.tsv`、`beauty-add.tsv`、`meme-add.tsv`、`mixed-words.tsv` 與新版異體字、保護詞及讀音豁免表，模型快取鍵含詞類表、KN 與兩個 trigram 表。命中快取時照上游 `load_lm`／`load_tinylm` 重設小模型與英文快取；上游每個引擎各建一份的數字表快取（合法讀音與字詞讀音）改由同一詞庫與模型的引擎共用 | `prepare-resident.ps1`、`platform/resident_windows.rs` |
| `core/src/predict.rs`、`core/src/lib.rs` | 預測索引與詞庫建好後收掉 `Vec` 多配的容量；內容不變 | `prepare-memory.ps1` |
| `core/src/digits.rs` | 新增混打專用的欄位正規化查詢 `Legal::tail_units_mixed` 與 `Scorer::decide_mixed`（混打關閉時與上游一致） | `prepare-memory.ps1`、`prepare-smart-input.ps1` |
| `core/src/colloquial.rs` | `overlay_only` 由 `HashSet<String>` 改為單一排序字串；沿用上游 SJ-SR-10-31M 的配對、聊天三檔係數與句首詞頻估計 | `prepare-memory.ps1`、`platform/word_set_windows.rs` |
| `core/src/ffi.rs` | 追加 Windows 專用的候選、輸入選項、標點、預測列查詢 ABI；不改上游既有簽名 | `platform/*_windows_ffi.rs`、`windows/src/engine.rs` |
| `core/Cargo.toml` | 產生的 core 可獨立建置，加入模型轉換工具 | `prepare-core.ps1`、`platform/map_model.rs` |

prepare 腳本以確切錨點核對陣列型別、`Fixed` 欄位、初始化、`open_candidates`、候選 span 與按鍵分支；錨點對不上就失敗。不要為了通過而移除檢查，也不要只看替換次數就認定等價。上游日後提供相同的正式 API 時，應遷移並刪除對應覆蓋。

中英文之間自動加空格沿用既有開關，預設關閉。在半形英數模式，游標前是漢字時也會在首個 ASCII 英文字母或數字前補一個空格。這個按鍵由前端直接寫入，後續英數鍵交給宿主。終端機、支援 Vim 指令的常見編輯器、私密欄位及修飾鍵組合保留直通，無法取得同步編輯鎖時亦同。 [原作者 #110](https://github.com/Nanako0129/shanjie/pull/110)

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

- 「純注音輸出」（`[input] zhuyin_only`）為 Windows 專屬模式，兩個範本預設關閉，既有設定缺省亦關閉；不改核心 pin 或模型。啟用後，標準／倚天配置的 37 個注音符號與 4 個聲調逐鍵以字面輸出，空白固定輸入空格，不開候選、預測或聯想，也不記憶為選字。原有混打、聲調與空白鍵設定保留，關閉後恢復；拼音配置下保留設定值但不啟用。切換模式與鍵盤配置一樣，在背景建立新引擎，等現有組字結束再切換。

- 完整沿用上游主線的逐鍵讀音解碼（含音節歸零 `+rn`）、變調與口語讀音（三聲變調、口語輕聲與疊字陰平）、合音詞、收尾鍵英文／注音自動比較（`english.tsv`、`shanjie_engine_set_english_auto` 照上游維持預設開啟且不另設 UI 開關）、多音字詞頻、句首單字學習、異體字排序、語意保護詞與模型配套參數；不再使用 v0.6.1 的聊天係數。「變調與口語讀音」開關（`[engine] tone3_sandhi`）與上游一致置於「模糊輸入」區塊且預設開啟，屬個人設定不隨操作預設覆寫或匯出。美妝與保養詞包（539 詞）、迷因與直播詞包（139 詞）與上游一樣預設開啟，Windows 設定保存、快速設定、個人詞典與常駐快取均包含兩者；完整出貨五組詞包。
- 收尾鍵英文／注音自動比較（上游 PR #80）在開啟「依按鍵順序判斷英文」與「顛倒保護」時，對無前文且倒序輸入的高頻表列英文字串（例如 `o1 `、`l s `）於收尾鍵判定為英文 token，緊接按 `Esc` 可還原為中文音節；句中有中文前文時仍依語言模型解碼為中文（例如「是否需要來杯咖啡提神」中的 `o1 `）。原鍵混打模式不經此比較，維持既有鍵序與空白規則。
- 同步主線的英文正式拼寫與中英混合詞表（例如 ChatGPT、CP值、SIM卡）。上游判斷英文模式的 Shift 起頭英文沿用正式拼寫，包含簡寫與品牌大小寫；原鍵混打預設維持既有鍵序與大小寫政策，另提供獨立開關 `[input] mixed_english_writing`（兩個範本預設 `0`）可於原鍵混打下套用同一張上游正式拼寫表，保留原鍵混打判斷、重複鍵容錯與刻意輸入的大小寫，不影響純英文模式。右側候選遇到中英混合詞時沿用上游的完整英文單元加中文後綴範圍，普通候選仍照使用者的錨點選項。
- 上游新增 `shanjie_engine_set_key_scheme` ABI 一併提供；Windows 前端繼續使用現有標準方案 0 加 TSF／profile 的可設定按鍵政策，避免重複套用方案 1 的固定微軟規則而蓋掉既有 Windows／善解互動模式。上游兩方案的原始核心測試均保留。
- 上游現有的 Windows 日期與 trigram 唯讀映射取代舊覆蓋；`.winmap` 模型陣列、Windows 學習儲存與選字暫存日誌仍保留。Windows 儲存沿用上游新版序列化驗證，保存句首單字的粗／中／細讀音鍵。
- 移植原作者 #122 的單按 Shift 切換。標準注音關閉中英混打時，切到英數會丟棄未完成音節，保留已組出的文字。英數照實際鍵盤字元接入同一組字，切回中文可續打，Enter 一次送出。倚天關閉混打時先送出已完成部分並丟掉未完成音節。兩種混打模式、拼音、純注音與一般模式切換沿用各自的既有規則。 [原作者 #122](https://github.com/Nanako0129/shanjie/pull/122)
- 標點設定增加麥注音排列。兩套範本與沒有此鍵的既有設定檔預設採用 `[punctuation] style=apple`。選擇 `mcbopomofo` 後，Shift＋2～8 輸入 `＠＃＄％︿＆＊`。Shift＋單引號改為全形分號，單按單引號與反斜線改為頓號與全形反斜線，這三鍵仍依原有的逐鍵開關。倚天的單按單引號仍是注音鍵。 [原作者 #122](https://github.com/Nanako0129/shanjie/pull/122)
- 英文覆寫預設關閉，保留明確儲存的值；沒有 macOS 的一次性重設。Windows 的 Enter 最多等目前語意校正 100 毫秒，診斷頁與瀏覽器修正仍由各平台處理。

- 啟用「記憶選字」時，Windows 候選確認立即建立暫存記憶；改選、刪字或取消會撤回尚未確認的記憶，提交時寫入一次。Enter 只提交游標前時，已送出前綴的讀音與選字標記保留到剩餘組字確認，讓「楓之谷」可整詞記憶，不需先按 End。失焦提交只在立即取得學習鎖時保存，其他會話正在寫入時先送出文字、略過該次記憶，避免切換視窗等待。學習關閉時不記憶，未套用 Windows 選項的核心維持上游只在提交學習的政策。
- 右側候選沿用上游的自學新詞與模糊音清單政策：同長度的新詞排在詞庫詞條前，低於啟用權重的新詞仍可列出並忘記；選取模糊音項目時改用其詞庫讀音。私人詞典載入與共用詞庫快取都保留詞包的 OOV 前綴範圍，供後續模型載入使用。
- Windows 範本預設開啟「反引號＋標點」，善解範本仍關閉；缺省判斷依互動模式。schema < 9 的 Windows 設定一次改為開啟，包含舊版手動關閉的值（INI 無法分辨設定來源）；schema 9 後保留關閉。支援 Shift 數字列符號、減號／底線與波浪號；一般 Ctrl 標點的可用按鍵不變。「反引號＋標點」對應出的符號會送入核心可編輯組字，可開候選替換或按 Esc 取消；Unicode 內碼與符號鍵盤仍直接提交。單獨輸入反引號後按 Enter 會提交字面反引號並結束特殊輸入及 TSF 組字。
- Windows 輸入選項啟用相關符號候選：括號、引號、斜線、直線／箭頭、尖括號／不等號、破折號、底線與其他鍵盤標點可選同類及全／半形符號，左右候選位置一致，原符號優先且去重。全／半形減號可選 `→`、`←`、`…`、`⋯`，正斜線可選 `↗`、`↙`，反斜線可選 `↖`、`↘`，星號可選 `※`。數字串中的減號與正斜線也可改選對應符號。反引號＋Shift＋反斜線與 Ctrl＋Shift＋反斜線輸入「｜」，可再選 ↑↓ 等箭頭。未套用 Windows 輸入選項的核心仍保留上游候選表；明確呼叫 `set_punctuation` 會改用傳入的完整表。
- 反引號特殊輸入優先於拼音處理；進入時先提交原有拼音組字，再清理拼音與原始讀音暫存。Unicode 內碼不受拼音攔截，宿主在特殊模式內的選取通知不重寫組字顯示。
- Windows 的全／半形切換立即更新組字顯示，提交與失焦送出採相同轉換；ASCII `!`–`~` 對應 `U+FF01`–`U+FF5E`，空白對應 `U+3000`，中文標點不變。核心保留原字元，切回半形可還原組字中的 ASCII。

- 善解範本（`profiles/shanjie.ini`）與上游 App 一樣開啟上游英文詞表；Windows 範本（微軟注音風格）不開。
- 上游判斷單按數字鍵：上游顯示未完成注音（`1` 是 ㄅ，Enter 與 ↓ 送出／展開 ㄅ，digits.md A.12）；Windows 顯示並送出數字本身，按鍵仍保留，後續可組成注音（`184` 是 爸）。套用 Windows 輸入選項的標準（大千）配置下，單按 `0`–`9` 按 ↓ 與各數字／符號單元（`0`–`9`、`.`、`/`、`-`）的左右候選與 ↑ 循環，皆會在全形數字後附上對應的大千注音或聲調符號；聲韻母若詞庫與模型有條目即轉成可再改選漢字的真實讀音單元，聲調或缺乏條目者則以符號單元插入，原有的漢字轉換候選（`DigitAlt::Syllable`）與左右數字候選的預測列維持可用（選取或忘記預測項時保留原數字與候選錨點），同時修正右側候選使用漢字轉換項的取代區間。未套用 Windows 選項的核心與倚天配置維持上游行為。沒有設定開關。
- Windows 單獨聲調關閉（`standalone_tones=0`）時，閒置按聲調鍵仍輸入字面數字，不開上游數字串。Windows 互動模式開啟時，空組字按聲調鍵一律輸入聲調符號，所以 `3`、`4`、`6`、`7` 不能作為數字串開頭；善解互動模式照上游，開啟英文時由數字串取代。
- Windows 的「→ 右方向鍵」為 `[candidates] right_action=none|page|expand`：預設「無」保留原方向鍵，「下一頁」沿用舊開關，「展開」開啟三欄多列候選。舊 `right_pages=0/1` 分別遷移為 `none/page`；已有新選項時優先保留。兩種互動模式收合時 ↑↓ 逐項移動；展開時沿同一直排 ±3 移動，最上列 ↑ 收回，末列向下停在原位。核心未套用 Windows 選項時保留上游九欄；輸出視窗與數字仍依反白所在的九項群組選字。預測與聯想不展開。
- 數字鍵盤的 0–9 與 `+ - * / .` 走字面插入，不當注音；組字仍可編輯，候選開啟時 1–9 保留選字。有未完成讀音時先提交讀音再輸入字面字元；內碼輸入中的九宮格數字視為內碼。全形模式下字面字元一併轉為全形。
- 「部分符號使用半形」預設關閉，可在中文模式選擇 27 個符號鍵，啟用時預設採用上游的 14 鍵配置。勾選鍵優先以半形進入組字，沿用 Windows 的全／半形候選與待完成讀音設定。倚天保留單按 `'`／`=` 的注音，反引號快捷鍵仍有優先權。關閉後保留已儲存的勾選。 [原作者 #132](https://github.com/Nanako0129/shanjie/pull/132)
- 「標點保留未完成讀音」是 Windows 選項（`[input] punctuation_keeps_reading`），新設定檔預設關、與上游相同；從舊版升級的設定檔設為開，維持原本打字結果。
- 語意校正：上游在背景佇列跑完後回主執行緒套用；Windows 由常駐核心在每個會話的專用背景執行緒（`Shanjie rerank`，`THREAD_PRIORITY_NORMAL`）跑，與上游 macOS #104 採 `userInitiated` QoS 目標一致但維持與前景／Pipe 執行緒同級且不改程序優先序類別；不重新解碼的按鍵保留既有工作，TSF 每 15 ms 以不帶序號的非阻塞請求（200 ms 期限）查詢是否完成（最多 2 秒，候選窗開著時不提前消費），完成後才送出有序號的 `Op::Rerank` 套用。這個請求記在重建組字的紀錄裡，核心重啟後照樣重放，候選順序與重啟前相同。另保留預設關閉的 `rerank-timing` Cargo feature 供離線純數值時間線量測與隔離優先序實驗。
- 依 App 自動：上游用 macOS bundle ID 與選單裡的「這個 App 用聊天設定」；Windows 用執行檔名稱，清單在設定頁直接編輯（`[engine] chat_apps`），沒有輸入法選單項目。
- 中英文自動加空格：上游只在可讀游標前文且學習未暫停時取開頭字元；Windows 在未組字時讀游標前文（「使用游標前文」關閉時，只有這個開關開著才讀）。
- App 內學習暫停已以 Windows scope 與宿主執行檔身分接通；macOS 的備份排除選項不直接移植。
- 上游 0.6.1 把「依按鍵順序判斷英文」預設改為開；新設定檔照上游，既有設定檔保留原本的值（多半是關）。
- 上游 0.6.1 的「單按 Shift 切換中英」（`shanjie_engine_set_ascii`，每個 App 各自記住）沒有接上；Windows 沿用自己的 Shift 切換，兩個範本預設都開。
- 組字面板（`[candidates] reading_panel`）是 Windows 專屬，**原鍵混打需要它**，兩個範本預設關閉。關閉時與上游相同：組字只在 App 內，沒有候選也沒有提示就不顯示面板。開啟後在候選窗位置顯示整句組字，好處是游標移進句子時會標出游標所在的字，使用者可依需要開啟；游標在句尾時不標，上游沒有這個面板。
- 英文段提示（`[candidates] english_hint`，預設開，與上游一致）：英文段開著時，面板在組字列後接「空白：英文斷開」，上游是獨立一行；Windows 可在設定關閉，關閉後只剩沒有組字列時面板不顯示。
- 原鍵混打（`[input] mixed_input`，鍵名不變）是 Windows 專屬的混打方式：按鍵排不成合法注音就當英文，原始按鍵留在組字區，面板提示注音，聲調完成後轉中文，Enter 保留原文。選用時組字面板固定開啟，不改寫 `reading_panel` 的值，離開這個模式就回到原值。
- 候選中 Tab：上游是關閉候選並繼續處理這個鍵（`core` 的兜底分支）。Windows 操作預設的 Tab 與 → 共用 `[candidates] right_action`，範本預設 `expand`，對齊微軟注音；善解操作預設的 Tab 與上游相同。
- 候選窗方向：上游預設橫排，可在設定改直排（`shanjie_engine_set_candidate_vertical`），Linux 前端也可選；Windows 所有範本一律直式，沒有設定也沒有橫向繪製，候選的直式畫法是 Windows 自己的，沒有接上游這個函式。舊設定的 `[candidates] layout` 讀入時丟棄（任何值都無害），下次儲存就消失。`profiles/*.ini` 保留這一行並加註解，因為 Linux 前端會讀。
- 範本預設：兩個範本的混打方式都是上游判斷（`upstream_english=1`、`mixed_input=0`），上游判斷不支援的配置（倚天、拼音）沒有作用。善解範本改成與上游相同：`space_operates=1`（空白開候選、候選中換頁）、`shift_toggle=1`、`learning=1`、`caps_lock_english=1`、`forget_candidate=1`（上游 ⌘⌫，Windows 是 Ctrl＋Shift＋Backspace）。只影響新設定檔與缺少的鍵，已存的值不改。混打方式是單選（關閉、上游判斷、原鍵混打，`mixed_input` 與 `upstream_english` 最多一個為 1）：0.9.0 到 0.15.x 的「兩者並用」已移除，原鍵混打開著的設定檔讀入時一律把 `upstream_english` 設 0（兩者並用變成原鍵混打，它原本就先判斷；缺少 `upstream_english` 時也不會從範本補成 1）。
- 善解範本仍與上游不同：`add_word=1`（雙 Ctrl 加詞，上游沒有）；`glass_tint=42`（上游 0，Windows 玻璃視窗有最低濃度，候選窗不讀它）；上游「瀏覽器第一個字不顯示預測」Windows 沒有。
- 「自動檢查更新」是 Windows 設定頁「關於」的選項，預設開，存在 `%LOCALAPPDATA%\Shanjie\update-auto.txt`，不在 `profile.ini`，上游沒有對應。開啟時由隱藏排程 `ShanjieUpdateCheck` 在登入與之後每 24 小時向 GitHub 查詢，排程同一版一天最多通知一次，開啟設定程式時只要有新版就通知；都不自動下載。
- Windows 的玻璃視窗（設定、安裝、更新卡片）有最低濃度，任何背景最多透出 20%，「玻璃濃度」從這個下限調到不透明，避免深色背景蓋過文字；次要文字也加深。
- 「更新通道」是 Windows 設定頁「關於」的選項（正式版／測試版），存在 `%LOCALAPPDATA%\Shanjie\update-channel.txt`，不在 `profile.ini`，上游沒有對應。預設是正式版；正在執行測試版（版本號帶 `-beta.N`）且尚未選擇時預設測試版。
- 預測列只在第一格標 Tab 符號（上游 header 註解），尚未核對 Windows 候選窗的畫法。

## 測試

`scripts/test-core.ps1` 在獨立的 `build/test` 樹跑上游 core 的單元與整合測試，使用 v0.9.0 的完整原始模型組（含會意 `SJ-SR-10-31M`），保留固定候選結果的斷言。Windows 只做以下調整，其餘斷言照上游：

- 符號連結改為複製；不存在的來源以資料夾代替（同樣是「存在但讀不到」）。
- 唯讀映射中的檔案不能刪除或改寫：沿用上游 `tri_tiny` 的 Windows 釋放流程；`load_lm_refuses_a_bad_trigram_table` 不在 Windows 執行。
- 上游 unix 學習檔（權限、FIFO、硬連結）由 Windows 儲存取代：`learn_store` 不跑，`engine_learn` 的三個 Unix 專屬測試與 `engine_fuzzy_rerank` 的權限設定不用；其餘一般儲存與句首學習測試在 Windows 儲存後端照常執行；`english_codes_through_the_c_abi` 需要 `mkfifo`，跳過。
- 上游 `learning_open_is_3_on_windows_and_picking_still_works` 驗的是尚未實作儲存的 Windows stub，這個移植已有 Windows 儲存，故不執行該項；實際開啟、保存、重開與句首單字鍵由儲存回歸覆蓋。
- Windows 自己的組字政策（標點保留讀音、原鍵混打等）預設關閉時與上游相同，上游斷言不改寫；開啟後的行為由 `platform/` 內的單元測試涵蓋。
