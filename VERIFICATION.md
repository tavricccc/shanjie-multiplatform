# Windows 移植驗證紀錄

日期：2026-10-06。上游 pin：`a96f5c553f6ffcf9ac23670363451c49c9faf138`。

## 已驗證

- Rust core、原生 TSF DLL：x64 與 x86 MSVC release 編譯成功。
- 上游 model-v1：SHA-256 `9879fd8b264b1c1f4c083ccedf84dc5625cd8d2595bd2d13150eed5a0520a923` 符合上游檔案。
- 映射模型：所有 9 個資料陣列、標量與代表性詞頻／分數核對一致。
- 109 個上游 core、engine、learning、replay 測試通過。標準與倚天各回放 302 句。
- x64 背景 TSF harness：真正 Windows context、edit session、composition、caret、commit、Esc、候選 element 與 COM lifetime 通過；不開視窗、不註冊系統輸入法。
- impeccable 對候選原生介面原始碼的機械檢查未回報問題；已實際擷取原生候選窗，確認文字與系統反白。多 DPI、讀屏尚未驗收。
- Windows／Shanjie 兩份 INI 均實際載入並通過背景 TSF 測試。Windows：↓ 開啟／移動、數字與 Enter 選字、Shift 中英切換；Shanjie：原本左右候選與 Shift 行為。
- 可見 TSF 測試重現首次 GetTextExt 無排版資料，收到 OnLayoutChange 後原生候選窗可見。
- 新版註冊 DLL 已在重新開啟的 Win32 記事本驗證：↓ 開啟直列候選、↓ 移至「妳」、Enter 選字／提交、Shift 切換英文、Shift＋空白後輸出全形 `ｂ`。
- 打包的 CMD profile 套用入口已實際寫入個人 INI，不要求管理員權限。
- 右鍵管理員安裝／移除入口已納入安裝包；PowerShell 腳本語法與包內 SHA-256 校驗通過。已透過 UAC 完成系統安裝註冊，未實際驗證右鍵入口互動與重新啟動後刪除。

背景 harness 在引擎啟用後量到整個程序 Private Bytes 41.3 MiB、Working Set 95.4 MiB。Working Set 包含模型映射頁面；此測量不能當作多 App 總耗用、閒置 CPU、輸入延遲或真實 App RAM 的實測。

## 驗證範圍

TSF harness 的文字儲存是記憶體中的 ITextStoreACP，TSF manager 和 context 是 Windows 實作。Windows 要求 active TIP 才可註冊 foreground keyboard sink，所以 harness 以 ThreadProxy 代替該單一步驟；未替代 edit session 或 composition。

上游的 Unix 權限／FIFO／連結拒絕測試與 macOS getrusage 效能測試在 Windows 建置副本中使用 cfg 排除。
上游 global_eps_table 是學習參數掃描，移植驗證不重跑它。這些項目不能計入 Windows 已通過。

## 尚待驗證

管理員解除安裝與重新啟動後刪除、數位簽章、Chrome/Edge/Office/Terminal 相容性、密碼與受保護欄位、焦點／滑鼠游標變更、實際多螢幕 DPI、觸控鍵盤遮擋、讀屏與 App 容器。

已在本機完成管理員安裝；核對 64 位元與 32 位元 COM DLL 路徑、兩個 TSF profile 註冊與全部安裝檔案 SHA-256。Microsoft Store 版 PowerShell 7.6.6 x64 已確認可用。使用者輸入法清單包含善解的兩個 profile，保留原本順序與既有項目；真實 App 驗證限於上述記事本操作。

學習 backend 測試通過不代表多程序學習可以啟用；frontend 仍維持學習關閉。

本次完成可建置、可打包、已做背景整合驗證的開發版。日常可用與正式發佈仍需上述實際系統驗收。
