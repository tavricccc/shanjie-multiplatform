# 微軟注音對照與設定索引

對照 [Microsoft Support 繁體中文 IME](https://support.microsoft.com/en-us/windows/hardware/input-devices/microsoft-traditional-chinese-ime) 與 [Microsoft Learn 繁體中文輸入指南](https://learn.microsoft.com/en-us/globalization/input/traditional-chinese-ime)（核對於 2026-11-06），列出兩份操作範本的鍵位與每個設定。

Windows 範本採微軟公開的常用鍵位，候選預設直式，候選中 Tab 與 → 預設展開表格。即時讀音預測、選字後聯想與善解額外的標點／聲調操作預設關閉；Caps Lock、學習、空白提交、拼音空白與反引號標點用專案自訂的值；候選中 Esc 只關候選不刪字；讀音候選預設不列出，開啟時排在第一項而非最後；語意校正、ACG／IT 詞包、依 App 排名、雙 Ctrl 加詞與混打（預設上游判斷）預設開啟，明顯改變體驗。這些是本專案的選擇，不是微軟的出廠值。候選排序、整句解碼與學習仍是善解的，沒有重製微軟的私有引擎。

## 模型

使用上游 Release 內建的模型；設定的一般頁可下載上游發布的其他相容模型。模型與詞類表的搭配沿用上游的規定。即時預測是獨立開關，和模型無關。

## 按鍵與狀態

| 狀態／按鍵 | Windows 操作預設 | 善解操作預設／可調整處 |
| --- | --- | --- |
| 未完成注音＋空白 | 完成第一聲，不開候選 | 相同；不受開候選鍵／提交後空白選項影響 |
| 完整組字＋↓ | 開轉換候選 | 同樣可開；`candidates.open` 可只選空白 |
| 完整組字＋空白 | 插入可編輯空格，組字繼續 | 相同；可開 `space_commits` 或 `space_operates` 改為提交／候選操作 |
| 未開候選＋↑ | 在目前字詞的候選中循環，保持游標，不開窗 | 預設不換字；`up_cycles` 可獨立開啟 |
| 候選中 ↑／↓／← | 逐項選字；展開時 ↑↓ 逐項移動，頂列 ↑ 收回 | 相同 |
| 候選中 → | 展開三欄候選；`right_action` 可改無（下一項）或下一頁 | 預設下一項；`right_action` 同樣可調 |
| 候選中 Tab | 與 → 同一個動作（`right_action`），預設展開三欄；已展開時下一候選 | 與上游相同：不跟 `right_action`，關閉候選，提交組字並把 Tab 交給 App；兩者的預測列都保留 Tab 進入／Shift＋Tab 退出 |
| 未開候選、組字中的 Tab | 提交組字並把 Tab 交給 App | 相同 |
| PageUp／PageDown | 不翻譯，交給 App | 相同 |
| 數字鍵盤 | 輸入字面字元；候選開啟時 1–9 仍選字 | 相同 |
| 雙按 Ctrl | 反白字詞後加詞（`add_word`） | 相同 |
| 候選中空白 | 下一頁，末頁回第一頁 | `space_pages`、`wrap_pages` 分別控制換頁／循環 |
| 候選中 1–9 | 選目前頁／列的候選，保留組字 | 相同；不以普通數字選被動預測或聯想 |
| 候選中 Enter | 確認候選再提交；範圍依 `enter_to_cursor` | 原有確認後繼續組字；`enter_commits` 可獨立調整 |
| 一般 Enter | 新設定提交游標前，固定多字詞不拆開 | 預設整句；`enter_to_cursor` 可另開。舊設定缺值保留整句 |
| 候選中 Backspace | 只收回候選 | `backspace_deletes` 開啟時再刪游標前一個字／音節 |
| 未完成注音 Backspace | 刪畫面最後的注音，韻母→介音→聲母 | 保留上游 v0.2.1 修正，沒有再加一套順序選單 |
| Delete／←→／Home／End | 刪游標右側音節或移動組字游標 | 未開候選時共用核心；候選／預測依自身狀態處理 |
| 候選／已進入預測 Esc | 先取消候選或退出預測列 | 不因整句 Esc 設定一次刪掉整句 |
| 其他組字 Esc | 先清未完成讀音，再清完整句 | `input.escape` 可選逐階段、全部清除、保留組字 |
| Win 快捷鍵 | 交給 Windows，不把 Win 當上游 Command | 兩 profile 共用；Win＋Backspace 不會忘記學習 |
| App 的 Ctrl／Alt 快捷鍵 | 通過；只攔截已啟用的明確 IME 快捷鍵 | 字碼模式也不讓 Ctrl 標點破壞未完成的碼 |

微軟文件的候選窗按鍵只列出 ↓、↑、Space、Tab、→、數字、Enter 與聯想的 Shift＋數字，沒有 Esc、Backspace、整句提交與翻頁末端的完整行為（Support 另有 Shift、Shift＋Space、Ctrl＋符號、反引號、Ctrl＋Alt＋逗號的快捷鍵表）；Learn 對 Esc 的描述也和這裡的逐階段取消不同，所以這些都做成可調整的選項。是否與微軟注音完全一致，仍需在真實 App 中對照。

## 逐項開關

W／S 為全新 Windows／善解範本的值。在設定中載入操作預設只改操作、候選與預測；標「保留」的項目不會被覆寫。目前 schema 9；升級舊 Windows 設定時一次開啟反引號標點，之後保留使用者關閉的值。

| 設定分類與 INI key | 設定頁／作用 | W／S |
| --- | --- | --- |
| `profile.interaction` | 選字與外觀：↑↓ 或 ←→ 政策 | windows／shanjie |
| `engine.model_id` | 一般：內建或已下載的模型 | 內建；保留 |
| `engine.ranking` | 一般：依 App 自動／聊天／正式權重 | auto／auto；保留 |
| `engine.chat_apps` | 自動時算聊天的程式；未設定用內建清單 | 未設定；保留 |
| `engine.acg_pack`／`engine.it_pack`／`engine.quote_pack` | 動漫遊戲／資訊 AI／經典台詞詞包 | 1、1、0；保留 |
| `engine.rerank`／`engine.rerank_level` | 語意校正與等級 | 1、standard；保留 |
| `engine.fuzzy_en_eng`、`fuzzy_zh_z`、`fuzzy_ch_c`、`fuzzy_sh_s`、`fuzzy_adjacent` | 模糊音與鄰鍵打錯 | 全 0；保留 |
| `engine.prediction` | 未完成讀音的即時預測 | 0／1 |
| `engine.demote` | 避免特定敏感詞排名過高，詞仍可選 | 1／1；保留 |
| `input.chinese` | 一般：預設中文／英文 | 1／1；保留 |
| `input.full_width` | 一般：預設全／半形 | 0／0；保留 |
| `input.keyboard` | 跟隨 TSF profile／標準／倚天／漢語／臺灣拼音 | 0／0；保留 |
| `input.caps_lock_english` | Caps Lock 暫時輸入英文；關閉時仍輸入注音 | 0／1 |
| `input.mixed_input`／`input.upstream_english` | 中英混打與切換：原鍵混打／上游英文／並用 | 兩個範本都是 0、1，預設上游英文 |
| `candidates.reading_panel` | 組字面板：顯示整句組字並標出游標所在的字，可依需要開啟；原鍵混打時固定開啟 | 0／0 |
| `input.english_overwrite` | 上游判斷：重打同一個位置算英文 | 1／1；限標準注音 |
| `input.english_key_order` | 上游判斷：聲調後音節不存在、注音鍵沒照順序算英文 | 1／1（舊設定檔保留原值）；限標準注音 |
| `input.english_reversal_guard` | 上游判斷：快打時鍵序顛倒不當英文 | 1／1；限標準注音 |
| `input.mixed_tolerance` | 原鍵混打片段的相鄰重複鍵容錯 | 0／0 |
| `input.alphanumeric` | 拼音鍵盤無法轉換的序列輸出實際英數；注音鍵盤只用在引擎失敗的後備，注音的英文判斷靠混打 | 1／0 |
| `input.cjk_spacing` | 中英文之間自動加空格 | 0／0；保留 |
| `input.punctuation_keeps_reading` | 標點保留未完成的注音 | 0／0 |
| `input.escape` | 未開轉換候選時 staged／clear／ignore | staged／staged |
| `input.space_after_commit` | 提交完整組字後是否再交給 App 輸入空白 | 0／1 |
| `input.space_commits` | 空白提交組字；關閉時插入可編輯空格 | 0／0 |
| `input.space_inputs_zhuyin` | 候選是否隱藏注音讀音項；關閉時讀音項排第一項，反白落在第二項 | 1／1 |
| `input.pinyin_space_converts` | 拼音按一次空白轉換；關閉為分隔／雙空白 | 0／0 |
| `input.standalone_tones` | 沒有組字時，聲調鍵輸入聲調符號 | 0／1 |
| `punctuation.shift_*`（13 個） | 善解式 Shift＋符號直接中文標點，逐鍵開關 | 全 0／全 1 |
| `punctuation.plain_*`（6 個） | 非注音的括號等鍵直接中文標點，逐鍵開關 | 全 0／全 1 |
| `input.context` | 使用少量游標前文作為本機選字及學習前文 | 1／1；保留 |
| `input.associations` | 數字／滑鼠選字後的後續詞聯想 | 0／0 |
| `input.learning` | 學習明確選字並在提交時記錄 | 1／1；保留 |
| `input.dictionary` | 設定頁編輯、原子保存的個人詞庫 | 保留內容／路徑 |
| `candidates.open` | 完整組字後 down／space／both | down／both |
| `candidates.anchor` | 游標右側／左側候選範圍 | right／left |
| `candidates.layout` | 橫向／直式；表格由 `right_action=expand` 進入 | vertical／horizontal |
| `candidates.enter_commits` | 候選 Enter 是否同時提交 | 1／0 |
| `candidates.enter_to_cursor` | Enter 只提交游標前；固定詞不拆開 | 1／0；舊設定缺值為 0 |
| `candidates.up_cycles` | 未開候選的 ↑ 循環換字 | 1／0 |
| `candidates.backspace_deletes` | 收回候選後再刪前一音節 | 0／0 |
| `candidates.space_pages` | 空白換候選／聯想下一頁 | 1／1 |
| `candidates.space_operates` | 空白是否操作候選 | 0／1 |
| `candidates.wrap_pages` | 末頁換頁（空白／→ 下一頁）是否循環到首頁 | 1／1 |
| `candidates.right_action` | 候選中 →（Windows 操作預設連 Tab）：none／page／expand（無／下一頁／展開） | expand／none；舊 right_pages=0/1 與沒有這個 key 的舊檔遷移為 none/page |
| `shortcuts.shift_toggle` | 單按 Shift 中英切換 | 1／1 |
| `shortcuts.shift_side` | 左右／左／右 Shift | 0／0 |
| `shortcuts.ctrl_space_toggle` | 額外 Ctrl＋空白中英切換 | 0／0；保留 App 補全 |
| `shortcuts.shift_space_toggle` | Shift＋空白全半形 | 1／0 |
| `shortcuts.ctrl_punctuation` | Ctrl／Ctrl＋Shift 標點 | 1／0 |
| `shortcuts.backquote_symbols` | 反引號＋標點 | 1／0 |
| `shortcuts.code_input` | 反引號＋U 的 Unicode 字碼，按 Space 或 Enter 結束；微軟另有反引號＋B 的內碼輸入，這裡不提供 | 1／0 |
| `shortcuts.symbol_keyboard` | Ctrl＋Alt＋逗號符號候選清單，選一個就離開；不是微軟的符號鍵盤 | 1／0 |
| `shortcuts.ctrl_backslash` | 善解的 Ctrl＋反斜線頓號 | 0／1 |
| `shortcuts.forget_candidate` | Ctrl＋Shift＋Backspace 忘記目前反白項的學習 | 0／1；學習開啟時生效 |
| `shortcuts.add_word`、`add_word_side`、`add_word_interval`、`add_word_shortcut` | 反白加詞的開關、哪一邊 Ctrl、雙按間隔、替代快捷鍵 | 1、0、450、0／相同 |
| `appearance.theme` | 系統／淺／深色 | 0／0；保留 |
| `appearance.font_size` | 候選 14–32 DIP | 16／16；保留既有值，舊 schema 未指定時保留 18 |
| `appearance.mode_indicator` | 中英切換的游標旁提示 | 1／1；保留 |
| `appearance.glass_tint` | 設定、安裝與更新視窗的玻璃濃度 | 42／42；保留 |

Ctrl 標點不再靠改成上游 Shift 標點來模擬，避免 Ctrl＋[ 變成『、Ctrl＋反斜線變成｜等錯誤。標點插入原組字，不先提交整句；Ctrl＋1／9／0 沒有 Shift 時不攔截；Ctrl＋Shift＋2～8 與 Ctrl＋減號（含 Shift）不支援，微軟寫的是通用規則。反引號標點共用同一份字元對照。個別非標準符號以 `windows/src/input.rs` 的表為本專案定義。

## 預測、學習與生命週期

- 即時預測只補完輸入中的讀音。未進入時無數字／反白，數字仍是注音鍵；Tab 進入後才用數字選詞。Windows 的 ↑↓ 在已進入列中移動，Tab／Shift＋Tab 保留預測列本身的進退操作，不會誤展開格狀。滑鼠點選是明確選取。
- 聯想是選字後尚未輸入的後續詞，使用 Shift＋數字／滑鼠；Enter 提交並關閉。不是即時讀音預測，也不是一般 Enter／空白提交就要開的視窗。
- 關閉即時預測會清除目前列與反白；關閉聯想也會清除目前聯想。兩者保留既有組字。即時預測關閉時不建立預測索引。
- 開啟學習後，普通候選與新預測的明確選取沿用上游規則；Windows 以 mutex／重讀／epoch 協調保存與清除，不提供帳號／雲端同步。
- 變更鍵盤、排名、聊天程式清單、模型、詞包、語意校正、模糊音、詞庫、學習、候選範圍、前文、混打方式與英文判斷，或 `space_inputs_zhuyin` 時，先提交目前組字再換引擎（清單在 `shared/src/profile.rs` 的 `REBUILD`）；尚未轉換的拼音與字碼也保留其可見輸入。外觀、按鍵與預測開關透過通知套用。
- key／document／thread focus、layout destroy 都關閉候選；舊滑鼠 frame 不可選取新內容。這是生命週期與資料正確性，沒有提供「保留失焦候選」開關。

## 不做成虛假選項的功能

候選每頁最多九個，以及 128 音節自動提交仍是核心契約，不放沒有實作後端的頁數／無限組字選單。表格由 `right_action=expand` 進入，顯示三欄；上游是九欄。Backspace 順序採既有上游修正；「可省略韻母」沒有正式 API，因此沒有開關。

微軟私有候選排序、聯想模型、舊版相容引擎、私有詞庫匯入、IME Pad 手寫／筆畫／部首辨識、IME 工具列、字元集選項、學習資料匯入匯出、組字顯示屬性、雲端與搜尋都沒有實作，設定中也不提供。無聲調拼音分段是本專案自己的做法，不是微軟的。模式圖示右鍵直接開設定，沒有微軟的選單，全半形也沒有滑鼠入口。注音原文的末尾候選／按鈕已移除，相關核心 API 只留給測試。

## 遷移與通知

設定讀取只在記憶體中遷移舊 schema；明確儲存時才備份舊檔並寫回，遇到未知的新版 schema 不覆寫。遷移保留個人偏好、詞庫與學習，即時預測不會自己打開；退役的舊 key（如免聲調／簡拼、`layout=grid`、`tab_expands`、`right_expands`）會移除或改寫；表格本身由 `right_action=expand` 提供。命令列與設定程式共用 `profiles/` 範本與同一套 Rust 邏輯。

儲存時以 v2 非同步通知新版實例，舊版 DLL 保留原本設定，避免共存時互相改寫。沒有 INI 監控或逐鍵查檔：直接改 INI 後，按「重新啟動輸入法」或重新啟用善解才會讀取。這個按鈕不終止 App，也換不掉 App 已載入的舊版 DLL。

## 可選的智慧組字

「中英混打與切換」提供關閉、原鍵混打、上游判斷及兩者並用。以下例子說明原鍵混打模式，適用標準／倚天注音，拼音鍵盤時停用。上游英文模式限標準注音，Shift＋字母可開始英文組字段，Caps Lock 與 Shift＋Caps Lock 保留 Windows 實際大小寫。兩者並用時，各自處理所負責的片段；容錯不改寫上游英文組字段。

- 從第一個按鍵顯示英數原文。例如標準鍵盤 `gj` 直接顯示在組字區，下方藍色小提示顯示 `ㄕㄨ`，沒有選字序號，也不能點選成字。
- 注音仍需完整讀音與聲調；`su3` 轉成「你」。未完成時按 Enter 保留英數原文，也可按 ↓／Tab 改回注音顯示後繼續操作。
- 本機常用英文辨識保留，`su3hello` 得到「你hello」，`callsu3` 得到「call你」。已辨識英文後空白保留空格；英文沒有自動補字。未知或短英文可按 Enter 保留原文。

「外觀」的 **介面與選字框主題** 同時控制設定窗與候選窗；跟隨 Windows 時讀取系統應用程式淺深色。深色候選為深灰底與淺色字，保持原有細邊線；高對比優先使用系統配色。
