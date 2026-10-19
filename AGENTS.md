# 維護入口

- 開始功能更新前先讀 [TODO.md](TODO.md)，確認待處理需求與驗收條件。
- 更新或 merge 上游前先讀 [UPSTREAM-CHANGES.md](UPSTREAM-CHANGES.md) 與 `UPSTREAM.json`；架構與驗證指令見 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。
- 上游是固定 commit 的子模組，`upstream/shanjie` 的追蹤檔案保持乾淨；Windows 核心差異放在 `platform/` 與 prepare 腳本。
- `build/core` 是產生檔，不在其中手改。升版時同時更新子模組 pin 與 `UPSTREAM.json`，重新核對覆蓋契約。
- Windows 對核心的變更包含核心擴充，不要描述成「只改 config」。
- 新功能與上游差異分批 commit。文件只寫目前狀態；各版變更寫在 `Changelog.md` 與 `docs/releases/`，尚未實機驗收的項目寫在 `TODO.md`。
- 上游 PR 留待使用者與作者討論後另行整理。
- 使用 PowerShell 7，遵循使用者的繁體中文、模組化、必要測試與介面 skill 要求。
