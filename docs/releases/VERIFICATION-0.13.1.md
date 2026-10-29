# 0.13.1 驗證紀錄

本版修正 2026-10-29 已定位的浮層重複縮放／遮蔽，以及 Store 的 DOS 路徑解析拒絕。版本仍為 pre-release，一般使用者建議先用 0.12.2。

## 已知基準

- 0.13.0 實機浮層：來源 192 DPI、helper 96 DPI unaware，實體尺寸 1760×1176，且位於來源下方。使用者更正 Win+R／AyuGram／Codex／記事本皆可叫出，故本次修顯示，不改手勢取詞規則。
- Store 實際權限下：Rust DOS canonicalize 回 Win32 5；NT 路徑及核心檔案可讀，既知端點 Status 正常。因此修正端點建立的直接阻斷，Status 成功不當作中文組字驗收。
- 使用者確認 0.13.0 除這些問題以外的測試正常。Excel 反白取詞不再列需求。

## 本版驗證

執行證據保存在本機 `build/release-0.13.1/`。測試僅使用自建視窗、公開詞例與隔離資料，不安裝或改動真實輸入法。

| 範圍 | 已完成結果與界線 |
| --- | --- |
| 建置 | Windows release x64／x86、WinUI x64 0.13.1 成功；未啟用 input-diagnostics |
| 混打與輸入 | x64／x86 各 135/135，保留數字、r3／r4／r6／r7、有效注音與標點等既有案例 |
| TSF／取消 | 兩架構各 18/18 相容性、15/15 浮層生命週期；不切換的私有桌面，覆蓋 Shift、交接、cmd 單次提交、取消／重入／來源存活 |
| 單元與 helper | Windows 45 passed、4 ignored；其中三項指定 helper 後另測通過，僅隔離 HKCU registry fixture 未執行。跨程序 UIA 可編輯／唯讀取詞、保護欄位、逾時回收及浮層測試通過 |
| 浮層 DPI／層級 | 本機實際 192 DPI：跨程序三種來源 awareness 各測滑鼠取消／Esc，尺寸 880×588，native owner 為 helper、實際 z-order 在來源上方、未改前景、來源仍存活。單元另測來源重新置頂後浮層回到其上方。100／150% 與混合螢幕尚未實機覆蓋 |
| 使用者資料 | 真正原生介面在一般、封裝 LocalAppData、systemprofile 環境傳入下均回同一使用者位置；不讀寫真實個人資料。WinUI 隔離字典 LF／CRLF／CR 各兩列往返通過 |
| 受限檔案操作 | 既有 ShellExperienceHost AppContainer 權杖下，舊 DOS canonicalize 仍回 Win32 5；本版 NT 檔案身分、內容雜湊、明確權杖的已知資料夾均與一般使用者一致。只作檔案／API 驗證，不是 Store 的完整 TSF 實機驗收 |
| 核心恢復 | x64／x86 客戶端各 8 項，使用隔離的 64 位元測試核心：期限、遺失回覆、忙碌、重啟、模糊結果及容量回復通過 |
| 失敗紀錄 | 20 次失敗開啟只寫三個階段，保留 Win32 3／HRESULT，測試識別字與資料路徑未寫入 |
| 模型故障契約 | Python 18/18 通過；保留退步可選、完整性、競態／網路故障與非空發布保護 |
| 共享模組 | 12/12，使用 Windows 鎖定相依，字典與設定契約通過 |
| 本機封裝 | 原生 645 個列入 manifest 的檔案通過；加 manifest 共 646 個檔案，ZIP 與單檔內嵌內容逐一雜湊一致，不含私人 profile／字典／學習、診斷或 TODO。未啟動安裝程式 |

首次一般元件測試在受限執行環境碰到排程／pipe 查詢限制；相同測試在獨立資料與正常桌面權限下重跑通過。舊相容性 harness 和已安裝 0.13.0 的 helper 共用桌面，會被正常手勢接管，因此 0.13.0 與新版都不會開啟舊 TSF 面板；harness 已補獨立桌面，回歸通過，未為測試停用真正 helper。

建置仍有既有 COM export／extern 宣告與 WinUI x:Bind analyzer 警告；本機 NuGet vulnerability feed 無法連線，不宣稱完成線上弱點稽核。真正受限程序、不同 DPI／長時間仍依下方界線驗收。

## GitHub 工作流程

[Node.js 24 release 乾跑 40869327960](https://github.com/tavricccc/shanjie-multiplatform/actions/runs/40869327960) 成功，source `71f82a3412db2a5c256ede368507be7e96f0b9de`；不建立 Release、不覆寫既有資產，建置 artifact 上傳成功。檢查註記沒有 Node.js 20 警告，保留既有 WinUI analyzer 警告及 Ubuntu runner 系統更新通知。

## 實機界線

本輪不執行 UAC 安裝、不更動登入排程或系統 DPI，也不重啟 Explorer。修正後的 Store 中文完整輸入、真正宿主顯示、不同螢幕縮放與登入單例仍需安裝後驗收。不可把自建跨程序視窗或 API 測試寫成這些已通過。

沿用 [延伸驗收清單](../VALIDATION-BACKLOG.md) 的長時間、多工作階段與安裝矩陣。歷史 x86 check 286 的間歇條件未定位；本次通過不得宣稱其根因已修。
