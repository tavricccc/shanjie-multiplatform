# Windows 移植驗證紀錄

最新日期：2026-10-07。上游 pin：`a96f5c553f6ffcf9ac23670363451c49c9faf138`。

## 2026-10-07 WinUI 3 版本

後續密度調整：依使用者照片移除注音按鈕與頁次 footer、取消 item container 預設留白、依實際文字測量候選大小、收緊五頁設定並把開關排成同列。驗證使用畫面外的真實 WinUI capture，包含 14／18／32 DIP 字級、長詞、聯想與三種排列；沒有改候選核心與方向鍵政策。

密度複查發現 GridView 預設 item 尺寸仍沿用短詞配置，14 DIP 四字詞出現省略；已讓 container 與 ItemsWrapGrid 使用同一計算寬高並完成 layout 後更新 panel。候選仍保留九欄的上游格狀數字與方向鍵契約。

另外以明確 panel 寬度隔離 ScrollViewer viewport 的像素取整，淺／深色最小字級圖均完整顯示同列 1–9。獨立複查 verdict 為 accept／ship。x64／x86 host harness、profile 保存與離屏 capture 通過；新版 ZIP 的 UI SHA-256 與最後建置一致。

- x64／x86 TSF DLL 與 self-contained x64 WinUI 3 host 編譯成功；此次沒有更新系統安裝。
- 核心測試共 112 個通過（lib 33、engine 32、learning 45、replay 2），標準／倚天各回放 302 句。Unix／macOS 排除與 global_eps_table 界線沿用下文。
- 背景 TSF harness 通過。此回測量 Private Bytes 74.8 MiB、Working Set 164.3 MiB，僅代表該測試程序，未計入 WinUI host 或真實多 App 總量。
- `verify-interaction.ps1`：排列與按鍵政策分離、Unicode／Big5／符號、個人詞庫、聯想詞、漢語／臺灣拼音、交錯引擎學習、注音提交與 epoch 清除通過。測試使用隔離的個人資料夾。
- `verify-ui.ps1`：x64／x86 實際啟動 WinUI host、繼承 handle、NOACTIVATE、隱藏／重開、退出通過；真實 XAML 離屏 capture 包含淺／深色五頁、三種候選排列、儲存成功／錯誤、善解操作說明。
- 獨立 impeccable 審查找到固定儲存列的回饋可見性、profile 說明及維護文件三處問題，已統一修正。Capture 位於 `build/ui-review-final/`。
- 複查修正後的 preset severity 殘留也已解決，獨立 verdict 為 accept／ship。兩張 error-then-preset 跟隨系統淺色，不算深色證據。
- 新 ZIP 103.4 MiB，包內所有 SHA-256 核對通過，最終 UI／腳本／文件與來源一致，所有 PowerShell 腳本解析通過。命令列預設保留個人詞庫指向的隔離驗證通過。

這些是背景整合與離屏視覺驗證，不代表新版本已在記事本、Chrome、Office、讀屏、高對比、多螢幕 DPI、App 容器通過日常驗收。下文的安裝／記事本記錄只適用於 10 月 6 日的舊 Win32 介面版本。

## 2026-10-06 舊版本紀錄

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
- 新增右側候選核心測試兩個，上游原有 109 個測試仍通過；原有左侧模式沒有被替換。
- TSF harness：外部選取通知後游標位置同步、句中右側選字、左右 Shift 切換中英通過。
- 原生設定窗已在本機開啟、讀取控制項、保存截圖並按「儲存」，核對個人 INI 的 `anchor=right`、`open=down` 與切換開關。
- 新的中英切換不再為空組字要求同步寫入鎖；有組字時以非同步 edit session 提交。系統模式按鈕已實作，尚待新安裝版本的實際系統匣驗收。
- 右鍵管理員安裝／移除入口已納入安裝包；PowerShell 腳本語法與包內 SHA-256 校驗通過。已透過 UAC 完成系統安裝註冊，未實際驗證右鍵入口互動與重新啟動後刪除。

背景 harness 在引擎啟用後量到整個程序 Private Bytes 41.3 MiB、Working Set 95.4 MiB。Working Set 包含模型映射頁面；此測量不能當作多 App 總耗用、閒置 CPU、輸入延遲或真實 App RAM 的實測。

## 驗證範圍

TSF harness 的文字儲存是記憶體中的 ITextStoreACP，TSF manager 和 context 是 Windows 實作。Windows 要求 active TIP 才可註冊 foreground keyboard sink，所以 harness 以 ThreadProxy 代替該單一步驟；未替代 edit session 或 composition。

上游的 Unix 權限／FIFO／連結拒絕測試與 macOS getrusage 效能測試在 Windows 建置副本中使用 cfg 排除。
上游 global_eps_table 是學習參數掃描，移植驗證不重跑它。這些項目不能計入 Windows 已通過。

## 尚待驗證

管理員解除安裝與重新啟動後刪除、數位簽章、Chrome/Edge/Office/Terminal 相容性、密碼與受保護欄位、焦點／滑鼠游標變更、實際多螢幕 DPI、觸控鍵盤遮擋、讀屏與 App 容器。

已在本機完成管理員安裝；核對 64 位元與 32 位元 COM DLL 路徑、兩個 TSF profile 註冊與全部安裝檔案 SHA-256。Microsoft Store 版 PowerShell 7.6.6 x64 已確認可用。使用者輸入法清單包含善解的兩個 profile，保留原本順序與既有項目；真實 App 驗證限於上述記事本操作。

上述舊版本保持學習關閉。2026-10-07 已加入 Windows 多程序協調與可選學習功能，驗證範圍見本頁最新紀錄。

本次完成可建置、可打包、已做背景整合驗證的開發版。日常可用與正式發佈仍需上述實際系統驗收。

新增原生設定窗與右側候選版已建置／打包。首次更新 UAC 回傳取消；2026-10-06 使用者要求重新觸發後，安裝退出碼為 0，兩架構 COM 註冊指向新版本目錄，全部安裝檔案 SHA-256 與設定窗執行檔均已核對。已開啟的程式仍可能載有舊 DLL，需重新開啟後驗收新行為。
