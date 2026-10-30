# 維護入口

- 開始功能更新前先讀 [TODO.md](TODO.md)，確認待處理需求與驗收條件。
- 更新或 merge 上游前先讀 [UPSTREAM-CHANGES.md](UPSTREAM-CHANGES.md) 與 `UPSTREAM.json`；架構與驗證指令見 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。
- 上游是固定 commit 的子模組，`upstream/shanjie` 的追蹤檔案保持乾淨；Windows 核心差異放在 `platform/` 與 prepare 腳本。
- `build/core` 是產生檔，不在其中手改。升版時同時更新子模組 pin 與 `UPSTREAM.json`，重新核對覆蓋契約。
- Windows 對核心的變更包含核心擴充，不要描述成「只改 config」。
- 新功能與上游差異分批 commit，完成並測試後直接 commit，不必先詢問；push 與發布仍先確認。文件只寫目前狀態，保持精簡；各版變更寫在 `Changelog.md` 與 `docs/releases/`，`TODO.md` 只列必須由使用者實機確認的項目，能用腳本測的先自己測。
- 建置、CI 與 Release 只產生單檔安裝程式 `Shanjie-Windows-Setup.exe`；要使用者測試時一併建好它。測試盡量平行用滿 CPU，以記憶體上限保護（`test-core.ps1 -MemoryLimit`）。
- 上游 PR 留待使用者與作者討論後另行整理。
- 使用 PowerShell 7，遵循使用者的繁體中文、模組化、必要測試與介面 skill 要求。

## GitHub 專案

- 正式 repo：https://github.com/tavricccc/shanjie-multiplatform （GitHub repository ID：1418506561）。此名稱已由使用者確認，協作與發佈以此 repo 為準。
- 主要分支為 main；本機 origin 與後續推送／Release 操作使用上述正式名稱。
