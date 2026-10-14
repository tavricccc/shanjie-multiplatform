# 維護入口

- 開始下次功能更新前，先讀 [TODO.md](TODO.md)，確認待處理需求與驗收條件。
- 更新／merge 善解上游之前，先讀 `UPSTREAM-CHANGES.md`、`UPSTREAM.json` 與 `VERIFICATION.md`。
- 上游是固定 commit 的子模組。保持 `upstream/shanjie` 的追蹤檔案乾淨；Windows 核心差異放在 `platform/` 與 prepare 腳本。
- `build/core` 是產生檔，不在其中手改或保存修正。升版要同時更新子模組 pin 和 `UPSTREAM.json`，重新核對覆蓋契約。
- 不把 Windows 核心覆蓋描述成「只改 config」。目前候選方向包含核心擴充；具體差異見維護文件。
- 新功能與來源差異分批 commit，文件記錄已驗證與未驗證的界線。上游 PR 留待使用者和作者討論後另行整理。
- 使用 PowerShell 7，遵循使用者的繁體中文、模組化、必要測試與介面 skill 要求。
