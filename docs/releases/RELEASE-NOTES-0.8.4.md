# 善解 Windows 0.8.4

日期：2026-10-15。小改版，沿用 0.8.0 的上游與 model-v6＋classes-v4。

## 安裝後的執行環境

- **啟動新版核心**：安裝完成後，非提升權限的安裝工具立即啟動新版常駐核心。沙箱程式（Windows 搜尋、Store）無法自行啟動核心，不必再先在一般 App 打字。
- **停止舊版核心**：同一使用者、同一工作階段中，舊版本的 `shanjie-core.exe` 會收到停止要求；只處理映像名稱為 `shanjie-core.exe` 且不是目前版本的伺服器。
- **重新啟動持有舊版 DLL 的系統元件**：目前使用者工作階段中仍載入舊版 `Shanjie.dll` 的 SearchHost、StartMenuExperienceHost、TextInputHost、ShellExperienceHost 會被結束，由 Windows 自動重新啟動。
- **不在提升權限下啟動核心**：以系統管理員身分執行的程式（例如安裝對話框）不再啟動核心，避免核心以管理員權杖執行。
- `shanjie-manager refresh-runtime` 可手動執行上述三步。

## 驗證

- 實機：安裝後 SearchHost 改載入新版 DLL，只剩一個新版核心（使用者確認）。
- 實機：手動 `refresh-runtime` 停止 3 個舊版核心並保留目前版本；核心停止後可重新啟動。
- 實機發現：先前的核心曾以提升權限執行（由提升權限的安裝流程啟動），本版加入防護後重新啟動為一般權限。
- `cargo test --release`：25 通過。

## 未驗證

- 提升權限防護後的完整安裝流程未再實機跑一次。
