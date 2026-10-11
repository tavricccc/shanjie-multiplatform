# 微軟注音對照與設定索引

核對日期：2026-10-08。依 [Microsoft Support 繁體中文 IME](https://support.microsoft.com/en-us/windows/hardware/input-devices/microsoft-traditional-chinese-ime)、[Microsoft Learn 繁體中文輸入指南](https://learn.microsoft.com/en-us/globalization/input/traditional-chinese-ime)，逐項對照目前 Windows adapter、固定上游核心及兩份 profile。

Windows 操作預設採微軟公開的常用鍵位；依使用者要求，**即時讀音預測與選字後聯想都關閉**，善解的額外標點／聲調操作亦關閉。微軟也有可開關的聯想功能，這裡關閉是使用者指定，沒有把它寫成微軟的出廠值。候選順序、詞句解碼與學習仍屬於善解；沒有宣稱重製微軟私有引擎。

同日後續依使用者要求，將目前已儲存的個別設定設為 Windows 預設；下表已同步更新 Caps Lock、學習、空白提交、拼音空白與反引號標點。這些是使用者選定的值，不把它們描述成微軟的出廠值。

## 模型與資料配對（2026-10-09 更新）

| 模型 | 配對詞類表 | 差異與選擇 |
| --- | --- | --- |
| [model-v5](https://github.com/Nanako0129/shanjie/releases/tag/model-v5) | [classes-v3](https://github.com/Nanako0129/shanjie/releases/tag/classes-v3) | 唯一模型；舊設定遷移，不提供選單 |

詞類表是模型的一部分，**不提供任意配表或單獨停用的開關**。檔頭綁定原始模型 SHA-256；映射保存來源雜湊、九個模型陣列、三個詞類陣列。缺表或配錯明確拒絕，沒有舊模型 fallback。

上游核心的「V3 即時預測」與模型版本是不同概念；是否使用仍由獨立開關決定。

## 按鍵與狀態

以下為本專案預設；未指定的微軟內部狀態不以猜測當成驗收。

| 狀態／按鍵 | Windows 操作預設 | 善解操作預設／可調整處 |
| --- | --- | --- |
| 未完成注音＋空白 | 完成第一聲，不開候選 | 相同；不受開候選鍵／提交後空白選項影響 |
| 完整組字＋↓ | 開轉換候選 | 同樣可開；`candidates.open` 可只選空白 |
| 完整組字＋空白 | 提交組字，不再額外輸入空白 | 預設開候選；`open` 與 `input.space_after_commit` 可分別調整 |
| 未開候選＋↑ | 在目前字詞的候選中循環，保持游標，不開窗 | 預設不換字；`up_cycles` 可獨立開啟 |
| 候選中 ↑／↓／←／→ | 橫向列前後選字 | 相同；不展開格狀 |
| 候選中 Tab | 下一候選 | 相同；預測列仍保留 Tab 進入／Shift＋Tab 退出 |
| 候選中空白 | 下一頁，末頁回第一頁 | `space_pages`、`wrap_pages` 分別控制換頁／循環 |
| 候選中 1–9 | 選目前頁／列的候選，保留組字 | 相同；不以普通數字選被動預測或聯想 |
| 候選中 Enter | 確認並提交整句、關窗 | 原有確認後繼續組字；`enter_commits` 可獨立調整 |
| 一般 Enter | 提交目前完整組字，不自動開聯想 | 相同；未完成音節沿用核心規則 |
| 候選中 Backspace | 只收回候選 | `backspace_deletes` 開啟時再刪游標前一個字／音節 |
| 未完成注音 Backspace | 刪畫面最後的注音，韻母→介音→聲母 | 保留上游 v0.2.1 修正，沒有再加一套順序選單 |
| Delete／←→／Home／End | 刪游標右側音節或移動組字游標 | 未開候選時共用核心；候選／預測依自身狀態處理 |
| 候選／已進入預測 Esc | 先取消候選或退出預測列 | 不因整句 Esc 設定一次刪掉整句 |
| 其他組字 Esc | 先清未完成讀音，再清完整句 | `input.escape` 可選逐階段、全部清除、保留組字 |
| Win 快捷鍵 | 交給 Windows，不把 Win 當上游 Command | 兩 profile 共用；Win＋Backspace 不會忘記學習 |
| App 的 Ctrl／Alt 快捷鍵 | 通過；只攔截已啟用的明確 IME 快捷鍵 | 字碼模式也不讓 Ctrl 標點破壞未完成的碼 |

Support 明確列出 ↓、↑、Space、Tab、→、數字、Enter 及聯想的 Shift＋數字；對 Esc、Backspace、整句提交及翻頁末端沒有完整狀態表。Learn 的 Esc 描述與本專案逐階段取消仍有差異，故提供具體選項，且保留使用者先前選定的 Enter 提交預設。這些未公開細節尚需新安裝的真實 App 對照，背景 harness 不構成完全相同的證明。

## 逐項開關

W／S 為全新 Windows／善解操作範本的值。UI 載入操作預設只變更操作、候選與預測；標記「保留」的資料／外觀偏好不會被覆寫。schema 5 的個別選項可另行調整；固定模型、橫向排列與移除格狀是本輪使用者指定。

| 設定分類與 INI key | 設定頁／作用 | W／S |
| --- | --- | --- |
| `profile.interaction` | 選字與外觀：↑↓ 或 ←→ 政策 | windows／shanjie |
| `engine.model` | 固定 model-v5＋classes-v3，舊值遷移 | 5／5 |
| `engine.ranking` | 一般：聊天／正式權重 | chat／chat；保留 |
| `engine.prediction` | 未完成讀音的即時預測 | 0／1 |
| `engine.demote` | 避免特定敏感詞排名過高，詞仍可選 | 1／1；保留 |
| `input.chinese` | 一般：預設中文／英文 | 1／1；保留 |
| `input.full_width` | 一般：預設全／半形 | 0／0；保留 |
| `input.keyboard` | 跟隨 TSF profile／標準／倚天／漢語／臺灣拼音 | 0／0；保留 |
| `input.caps_lock_english` | Caps Lock 暫時輸入英文；關閉時仍輸入注音 | 0／0 |
| `input.alphanumeric` | 無法轉換的序列輸出實際英數 | 1／0 |
| `input.escape` | 未開轉換候選時 staged／clear／ignore | staged／staged |
| `input.space_after_commit` | 提交完整組字後是否再交給 App 輸入空白 | 0／1 |
| `input.pinyin_space_converts` | 拼音按一次空白轉換；關閉為分隔／雙空白 | 0／0 |
| `input.standalone_tones` | 沒有組字時，聲調鍵輸入聲調符號 | 0／1 |
| `input.shift_punctuation` | 善解式 Shift＋符號直接中文標點 | 0／1 |
| `input.plain_punctuation` | 非注音的括號等鍵直接中文標點 | 0／1 |
| `input.context` | 使用少量游標前文作為本機選字及學習前文 | 1／1；保留 |
| `input.associations` | 數字／滑鼠選字後的後續詞聯想 | 0／0 |
| `input.learning` | 學習明確選字並在提交時記錄 | 1／0；保留 |
| `input.dictionary` | 設定頁編輯、原子保存的個人詞庫 | 保留內容／路徑 |
| `candidates.open` | 完整組字後 down／space／both | down／both |
| `candidates.anchor` | 游標右側／左側候選範圍 | right／left |
| `candidates.layout` | 僅橫向膠囊，舊值遷移 | horizontal／horizontal |
| `candidates.enter_commits` | 候選 Enter 是否同時提交整句 | 1／0 |
| `candidates.up_cycles` | 未開候選的 ↑ 循環換字 | 1／0 |
| `candidates.backspace_deletes` | 收回候選後再刪前一音節 | 0／0 |
| `candidates.space_pages` | 空白換候選／聯想下一頁 | 1／1 |
| `candidates.wrap_pages` | 末頁空白是否循環到首頁 | 1／1 |
| `candidates.big5` | 篩選候選、即時預測與聯想，不重寫既有組字 | 0／0；保留 |
| `shortcuts.shift_toggle` | 單按 Shift 中英切換 | 1／0 |
| `shortcuts.shift_side` | 左右／左／右 Shift | 0／0 |
| `shortcuts.ctrl_space_toggle` | 額外 Ctrl＋空白中英切換 | 0／0；保留 App 補全 |
| `shortcuts.shift_space_toggle` | Shift＋空白全半形 | 1／0 |
| `shortcuts.ctrl_punctuation` | Ctrl／Ctrl＋Shift 標點 | 1／0 |
| `shortcuts.backquote_symbols` | 反引號＋標點 | 0／0 |
| `shortcuts.code_input` | 反引號＋U／B 的 Unicode／Big5 字碼 | 1／0 |
| `shortcuts.symbol_keyboard` | Ctrl＋Alt＋逗號符號列表 | 1／0 |
| `shortcuts.ctrl_backslash` | 善解的 Ctrl＋反斜線頓號 | 0／1 |
| `shortcuts.forget_candidate` | Ctrl＋Shift＋Backspace 忘記目前反白項的學習 | 0／0；學習開啟時生效 |
| `appearance.theme` | 系統／淺／深色 | 0／0；保留 |
| `appearance.font_size` | 候選 14–32 DIP | 16／16；保留既有值，舊 schema 未指定時保留 18 |
| `appearance.mode_indicator` | 中英切換的游標旁提示 | 1／1；保留 |

Ctrl 標點不再靠改成上游 Shift 標點來模擬，避免 Ctrl＋[ 變成『、Ctrl＋反斜線變成｜等錯誤。標點插入原組字，不先提交整句；Ctrl＋1／9／0 沒有 Shift 時不攔截。反引號標點共用同一份字元對照。個別非標準符號以 `windows/src/input.rs` 的表為本專案定義。

## 預測、學習與生命週期

- 即時預測只補完輸入中的讀音。未進入時無數字／反白，數字仍是注音鍵；Tab 進入後才用數字選詞。Windows 的 ↑↓ 在已進入列中移動，Tab／Shift＋Tab 保留預測列本身的進退操作，不會誤展開格狀。滑鼠點選是明確選取。
- 聯想是選字後尚未輸入的後續詞，使用 Shift＋數字／滑鼠；Enter 提交並關閉。不是即時讀音預測，也不是一般 Enter／空白提交就要開的視窗。
- 關閉即時預測會清除目前列與反白；關閉聯想也會清除目前聯想。兩者保留既有組字。即時預測關閉時不建立預測索引。
- 開啟學習後，普通候選與新預測的明確選取沿用上游規則；Windows 以 mutex／重讀／epoch 協調保存與清除，不提供帳號／雲端同步。
- 變更鍵盤、排名、詞庫、學習、字元集、候選方向或前文設定時，先提交目前組字再換引擎；尚未轉換的拼音與字碼也保留其可見輸入。外觀、按鍵與預測開關透過通知套用。
- key／document／thread focus、layout destroy 都關閉候選；舊滑鼠 frame 不可選取新內容。這是生命週期與資料正確性，沒有提供「保留失焦候選」開關。

## 不做成虛假選項的功能

候選每頁最多九個，以及 40 音節自動提交仍是核心契約，不放沒有實作後端的頁數／無限組字選單。上游內部格狀程式保留原樣，但 Windows 導覽不會進入該狀態。Backspace 順序採既有上游修正；「可省略韻母」沒有正式 API，因此沒有開關。

微軟私有候選排序、無聲調拼音分段、聯想模型、舊版相容引擎、私有詞庫匯入、IME Pad 手寫／筆畫／部首辨識、雲端與搜尋都沒有被 INI 宣稱為已實作。注音原文的末尾候選／按鈕依使用者先前決定保持移除，相關 Windows 核心 API 留作驗證，不重新加回 UI。

## 遷移與驗證界線

Rust 共用載入流程先保存旧設定備份，補上當時隱含行為，再遷移 schema 5：模型固定 v5、排列固定 horizontal，移除 tab_expands／right_expands／grid_up_collapses；其他偏好、詞庫、學習保留，即時預測不自行開啟。命令列與 WinUI 共用 `profiles/` 與 Rust 邏輯。更新會清理退休 schema 備份；解除安裝在清理前詢問個人詞庫／學習保留或全刪。

儲存由程式主動通知已啟用的實例；沒有 INI 監控或逐鍵查檔。「重新啟動輸入法」先保存未儲存草稿，再提交組字並重載核心；沒有草稿時讀取直接改過的 INI，不覆寫手動修改。它不終止 App，也不替換 App 已載入的舊版 DLL。

驗證結果見 [VERIFICATION.md](VERIFICATION.md)。背景 TSF、原生窗與離屏 XAML 證據分別驗證 adapter、視窗及設定保存；新安裝的記事本、Chrome／Office、多螢幕、高對比與讀屏仍需實際工作階段驗收。
