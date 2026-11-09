## 變更內容

<!-- 改了什麼、為什麼。一個 PR 只做一件事。 -->

Closes #

## 驗證

<!-- 依 AGENTS.md「驗證：做多少」勾選實際跑過的；沒跑的寫明原因。 -->

- [ ] 只改文件，不需要建置或測試
- [ ] `scripts/test.ps1 -Level FAST`
- [ ] `scripts/test.ps1 -Level AFFECTED`
- [ ] `scripts/test.ps1 -Level FULL`（整合前）
- [ ] `scripts/test.ps1 -Level RELEASE`（發布前）
- [ ] 實機確認（寫在下方）
- [ ] 已註明未執行的測試及原因

結果與沒跑的項目：

## 檢查

- [ ] 已跟上最新 `main`
- [ ] 沒有改版本號（升版 PR 除外，內容見 AGENTS.md「版本號」）
- [ ] 行為變更已更新對應的測試、`Changelog.md` 或文件
- [ ] 與上游預設或行為不同之處已記在 `UPSTREAM-CHANGES.md`
- [ ] 沒有加入私人詞典或真實學習資料
