# 善解 Windows 0.8.3

日期：2026-10-15。小改版，沿用 0.8.0 的上游與 model-v6＋classes-v4。

## 沙箱（WinUI／AppContainer）宿主

- **選字框**：Windows 搜尋（SearchHost）中選字框內容與位置都正確、視窗也可見，但落在 App 之下的視窗 band，畫面上看不到。依 Microsoft〈Input Method Editor (IME) requirements〉的 Owned window 要求，沙箱宿主改在 `CreateWindowEx` 時以 `ITfContextView::GetWnd`（失敗時 `GetFocus`）為擁有者，擁有者改變時重建視窗。建立後才改 `GWLP_HWNDPARENT` 經實測無效。一般桌面 App 行為不變。
- **定位備援**：`GetTextExt` 失敗或被裁切且沒有座標時，改用上一次游標位置或檢視範圍，不再直接隱藏。
- **設定**：沙箱宿主讀不到使用者資料夾的 `profile.ini`，改由已在執行的核心透過管道提供（新增 `Profile` 請求），讀不到時才用預設。
- **管道權限**：DACL 加入 ALL RESTRICTED APPLICATION PACKAGES（`S-1-15-2-2`）。
- **錯誤記錄**：沙箱宿主只在啟用、按鍵處理、編輯工作階段、`GetTextExt` 與選字框顯示失敗時，寫一行錯誤到該 App 暫存資料夾的 `shanjie-tsf.log`（上限 256 KB）；不記錄按鍵或輸入內容。

## 驗證

- 實機：Windows 搜尋與 Microsoft Store 出現選字框、可輸入，設定生效（使用者確認，診斷版建置）。
- `cargo test --release`：25 通過。

## 未驗證／已知限制

- 沙箱宿主仍無法自行啟動核心，須先在一般 App 打字或由登入預載啟動。
- 未實作搜尋整合（`ITfFnSearchCandidateProvider`、`ITfIntegratableCandidateListUIElement`）。
- 更新後，已在執行的程式（如 SearchHost）仍使用舊版 DLL，直到重新啟動。
