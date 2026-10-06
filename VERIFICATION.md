# Windows 移植驗證紀錄

日期：2026-10-06。上游 pin：`a96f5c553f6ffcf9ac23670363451c49c9faf138`。

## 已驗證

- Rust core、原生 TSF DLL：x64 與 x86 MSVC release 編譯成功。
- 上游 model-v1：SHA-256 `9879fd8b264b1c1f4c083ccedf84dc5625cd8d2595bd2d13150eed5a0520a923` 符合上游檔案。
- 映射模型：所有 9 個資料陣列、標量與代表性詞頻／分數核對一致。
- 109 個上游 core、engine、learning、replay 測試通過。標準與倚天各回放 302 句。
- x64 背景 TSF harness：真正 Windows context、edit session、composition、caret、commit、Esc、候選 element 與 COM lifetime 通過；不開視窗、不註冊系統輸入法。
- impeccable 對候選原生介面原始碼的機械檢查未回報問題。未做螢幕視覺驗收。
- 右鍵管理員安裝／移除入口已納入安裝包；PowerShell 腳本語法與包內 SHA-256 校驗通過。未實際觸發 UAC、系統註冊或重新啟動後刪除。

背景 harness 在引擎啟用後量到整個程序 Private Bytes 41.3 MiB、Working Set 95.4 MiB。Working Set 包含模型映射頁面；此測量不能當作多 App 總耗用、閒置 CPU、輸入延遲或真實 App RAM 的實測。

## 驗證範圍

TSF harness 的文字儲存是記憶體中的 ITextStoreACP，TSF manager 和 context 是 Windows 實作。Windows 要求 active TIP 才可註冊 foreground keyboard sink，所以 harness 以 ThreadProxy 代替該單一步驟；未替代 edit session 或 composition。

上游的 Unix 權限／FIFO／連結拒絕測試與 macOS getrusage 效能測試在 Windows 建置副本中使用 cfg 排除。
上游 global_eps_table 是學習參數掃描，移植驗證不重跑它。這些項目不能計入 Windows 已通過。

## 尚待驗證

管理員安裝／移除與兩架構系統註冊、數位簽章、Chrome/Edge/Office/Terminal 相容性、密碼與受保護欄位、焦點／滑鼠游標變更、實際多螢幕 DPI、觸控鍵盤遮擋、讀屏與 App 容器。

學習 backend 測試通過不代表多程序學習可以啟用；frontend 仍維持學習關閉。

本次完成可建置、可打包、已做背景整合驗證的開發版。日常可用與正式發佈仍需上述實際系統驗收。
