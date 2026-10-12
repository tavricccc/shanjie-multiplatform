# 中文中英混打 Caps Lock 大小寫問題確認

確認日期：2026-10-12。來源版本：`codex/winui3` 的 `11b0229fcff4ebce82f82c5905c347025a564faf`。

## 0.5.0 修復結果

此版使用 `ToUnicodeEx` 取得英文實際顯示字元，透過新的 `shanjie_windows_key_with_latin` 傳入核心；注音仍用原始按鍵判讀。混打保留顯示字元的大小寫，讀音與回播則正規化為小寫。不把「Caps Lock 切英文」與混打英文大小寫混為同一選項。

兩架構已通過原始 11 項大小寫案例，另確認 Caps Lock 下中文注音正常。原始八個錯誤案例均已修復，以下歷史表格保留舊版實際結果。新版 64 項整體原生回歸見 `build/update-checks-20261012-222719/results.json`。未更新系統目前使用的 DLL。

## 確認結果

問題已在目前 x64 與 x86 DLL 重現。中文模式開啟中英混打，且「Caps Lock 切英文」關閉時，Caps Lock 開啟但沒有按 Shift，輸入 `ENGLISH` 的按鍵序列仍顯示及提交為 `english`。

系統註冊的兩架構 DLL 與本次受測 `dist` 檔案 SHA-256 相同：

- x64：`a2d8144ca8f9c7a150a0364199b5fb18ebf248801285da7c0754fb6494f08436`
- x86：`697ac6d3aaacaf10c27dd3df777005124fc01d1b6db15aa906cfc48536f03c54`

實際使用者設定為 `chinese=1`、`mixed_input=1`、`caps_lock_english=0`、`keyboard=0`、`full_width=0`。診斷使用獨立設定副本，另將學習關閉。

## 隔離測試結果

每個架構執行 11 個案例，共 22 個；其中 8 個案例重現大小寫錯誤。兩架構結果一致，全部輸入按鍵均被正常接收。

下表的「Caps Lock 切英文」皆為關閉；按鍵序列為 `ENGLISH`：

| Caps Lock | Shift | 操作 | 期望 | 實際 |
| --- | --- | --- | --- | --- |
| 關 | 放開 | 完整單字 | `english` | `english` |
| 關 | 按下 | 完整單字 | `ENGLISH` | `ENGLISH` |
| 開 | 放開 | 完整單字 | `ENGLISH` | `english` |
| 開 | 按下 | 完整單字 | `english` | `english` |
| 先關後開 | 放開 | 前三個字母後開啟 | `engLISH` | `english` |
| 先開後關 | 放開 | 前三個字母後關閉 | `ENGlish` | `english` |
| 開 | 放開 | 先組出「你」，再輸入英文 | `你ENGLISH` | `你english` |

對照案例將「Caps Lock 切英文」開啟，Caps Lock 與 Shift 的四種組合皆產生正確字元。這項設定的模式切換語意應保留，修復不能強制改動使用者的選擇。

測試直接載入原始 DLL，呼叫其 `OnTestKeyDown` 與 `OnKeyDown`，使用真正的 Windows TSF context 和 edit session，以及專案既有的記憶體文字儲存與 ThreadProxy。ThreadProxy 只代理按鍵 sink 註冊。核心載入完成後才開始測試，以隔離首次載入問題。

鍵盤狀態透過 `SetKeyboardState` 設定於診斷執行緒，沒有使用 `SendInput`、前景視窗或全域鍵盤注入；使用者設定檔測試前後的 SHA-256 相同。[Microsoft 文件](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setkeyboardstate)說明此 API 的變更只影響呼叫執行緒。

診斷證據保存在 `build/diagnostics/caps-lock-20261012-165112/`，包括探針來源、兩架構編譯紀錄、各案例設定及日誌、`results.json` 與 `results.csv`。這是背景 TSF 重現，沒有操作記事本、Chrome 或 Office 的前景視窗。

## 根因

1. `windows/src/bridge.rs` 的 `sj_translate` 有讀到 Caps Lock，並放入 modifier 的 Caps Lock 旗標（遮罩 `0x10`）；但產生基礎字元時，傳入 `ToUnicodeEx` 的鍵盤表全部為零，所以按鍵 `E` 的基礎字元仍是 `e`。保留這種基礎字元對注音按鍵解析有用途。
2. `windows/src/tsf/keys.rs` 在 `wants` 與 `key_down` 中，當 `caps_lock_english=0` 時清除 Caps Lock 旗標。混打流程因此只拿到小寫基礎字元，無法知道 Caps Lock 已開啟。
3. `platform/smart_input_windows.rs` 的 `mixed_key` 只按 Shift 決定其英文原文大小寫，沒有獨立的 Caps Lock 或實際顯示字元資訊。

受測的 Caps Lock 開啟案例中，Windows 以實際鍵盤狀態轉譯的第一個字元是 `E`；DLL 的基礎轉譯是 `e`、modifier 是 16，但 TSF 最終輸出 `english`。這將問題定位到輸入法的狀態傳遞與大小寫處理。

不能只刪除兩處 `key.modifiers &= !16`。現有混打英文分支會對 Shift 以外的 modifier 放行；上游 `engine.rs` 也會對 `MOD_CAPSLOCK` 放行，會繞過既有組字處理。

## 修復方式

規劃將「英文大小寫」與「Caps Lock 切英文」分開處理：

- 在 Windows 輸入層保留實際 Caps Lock 與 Shift 狀態。英文顯示字元以目前鍵盤配置及完整鍵盤狀態呼叫 `ToUnicodeEx` 取得，沿用不改變鍵盤緩衝狀態的旗標 4。[Microsoft 文件](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-tounicodeex)描述了鍵盤狀態與此旗標的意義。
- 維持注音解析所需的標準按鍵，另經 Windows 專用輸入介面傳遞混打英文的實際顯示字元。不能把 Caps Lock modifier 原封不動送進上游，也不能將共用基礎按鍵轉譯直接全部改成大寫。
- 在 Windows 混打擴充中分別保存注音辨識用的原始按鍵與英文顯示內容，每次輸入保存當時的大小寫。Backspace、Esc、Enter、空白、中文尾段轉換及 reset 都要同步維護兩者，避免切換 Caps Lock 後改寫先前已輸入的字母。
- 保留「Caps Lock 切英文」目前的設定語意與非混打流程。正式修正放在 `windows/src/`、`platform/` 及必要的 prepare 腳本；上游子模組保持乾淨，`build/core` 只作為重新產生的建置輸出。

修復後重跑本次矩陣，補上 Caps Lock 開啟時仍可正常輸入中文注音、混打中文尾段、刪除及提交的案例。正式修補與修補後驗證留待下次更新；本輪只新增待辦、診斷及確認紀錄，產品 DLL 與 ZIP 未改動。
