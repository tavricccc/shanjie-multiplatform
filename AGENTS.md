# 維護規則

這份是唯一的工作規則；其他文件只描述目前的程式與待驗收項目。使用 PowerShell 7、繁體中文，遵循使用者的模組化、必要測試與介面 skill 要求。

## 開始前

- 先讀 [TODO.md](TODO.md)；動到核心或上游前讀 [UPSTREAM-CHANGES.md](UPSTREAM-CHANGES.md) 與 `UPSTREAM.json`。架構與驗證指令見 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。
- 查上游（Nanako0129/shanjie）有沒有比 `UPSTREAM.json` 新的 release，有就提醒使用者。
- 定位是移植：功能、預設值與上游一致；與上游不同的預設或行為（新增或改變的設定）要明確告訴使用者，並記在 UPSTREAM-CHANGES.md。

## 上游與核心

- `upstream/shanjie` 是固定 commit 的子模組，追蹤檔案保持乾淨；不把上游整棵 merge 進來。
- Windows 對核心的變更放在 `platform/` 與 `scripts/prepare-*.ps1`。`build/core` 是產生檔，不手改。這些是核心擴充，不要描述成「只改 config」。
- 升版：
  1. 本 repo 與子模組都乾淨。
  2. 子模組 fetch 並選定 commit，比較 C header、候選範圍、學習序列化、LM parser 與授權。
  3. checkout 新 commit，同步更新 `UPSTREAM.json`（`build.ps1` 拒絕不在 pin 上的子模組）。
  4. 跑 `prepare-core.ps1`；錨點失敗代表覆蓋要重新審查。
  5. 跑 `test-core.ps1` 與 `verify.ps1`，再打包。
  6. 分批 commit：pin 與覆蓋、Windows 功能、文件；更新 UPSTREAM-CHANGES.md。
- 不向上游提 PR；右側候選與平台 backend 等和作者討論 API 後再整理。

## 分支、Issue 與 Pull Request

- **不直接 push main。** 每個任務從最新的 `main` 開新分支，名稱 `type/short-name`（`feat/`、`fix/`、`docs/`、`chore/`），完成後開 PR 合入；不 force push 共用分支。
- 開工前先看有沒有對應的 issue 或別人的 PR；有就在 issue 留言認領，沒有就先開一個，避免兩個人做同一件事。
- PR 標題用英文 `type: description`，內文用 [PR 模板](.github/pull_request_template.md)，關聯 issue 寫 `Closes #N`。一個 PR 只做一件事；功能與修正的 PR 不改版本號，發布另開升版 PR（見下方「版本號」）。
- PR 要先跟上最新 `main`（rebase 或 merge 都可以），並依下方「驗證」表跑過該跑的項目後再開；沒跑的在 PR 裡寫明。
- 目前 PR 沒有自動檢查（CI 只在版本號變更合入 main 時跑），所以驗證結果以 PR 內文為準。合入時，已是每個邏輯變更各自一個 commit 的用 merge，其餘用 squash。
- Bug 與功能建議用 [issue 模板](.github/ISSUE_TEMPLATE/)；問題要附版本、Windows 版本與重現步驟。

## Commit、push 與發布

- 一個邏輯變更一個 commit（行為連同它的測試、UI 文字、README、TODO、Changelog 各自分開），以 `git add <檔案>` 分組，訊息用英文 `type: description`。完成並測試後直接 commit，不必先問。
- push、開 PR、Release、安裝到實機、刪除使用者資料都先問。
- 建置、CI 與 Release 只產生單檔安裝程式 `Shanjie-Windows-Setup.exe`；要使用者測試時一併建好它。

### 版本號

- 版本號在 `windows/Cargo.toml`、`windows/Cargo.lock`、`ui/Shanjie.UI.csproj`、`RELEASE-CHANNEL.json`，四處與 tag 必須一致。
- **發布走升版 PR**：功能 PR 都合入 main 後，另開一個只含版本號、`docs/releases/RELEASE-NOTES-<版本>.md` 與 `Changelog.md` 的 PR；它合入 main 時 Actions 建立 Release。其他 PR 不碰這些檔案。既有 Release 不會被重跑改寫，要改說明或 pre-release 狀態用 `gh release edit`。
- **預發布不遞增版本號。** 下一個要發布的版本固定為 `X.Y.Z`（目前正式版之後的那一版），預發布只加後綴：`X.Y.Z-beta.1`、`X.Y.Z-beta.2`……，`RELEASE-CHANNEL.json` 的 `prerelease` 設 `true`，tag 為 `vX.Y.Z-beta.N`。例：正式版是 0.15.3，預發布依序為 `0.15.4-beta.1`、`0.15.4-beta.2`，不是 0.15.4、0.15.5。
- 轉正式版時只拿掉後綴成 `X.Y.Z`，`prerelease` 設 `false`，`recommended_stable` 改成該版。beta 期間不更改 `X.Y.Z`。
- 使用後綴前，`ui/Setup/UpdateService.cs` 的 `Compare` 要能比較後綴；它目前只解析數字，遇到 `-beta.N` 會丟例外。

## 文件

- 文件只寫目前狀態並保持精簡。各版變更寫在 `Changelog.md` 與 `docs/releases/`；`TODO.md` 只列必須由使用者實機確認的項目，能用腳本測的先自己測。
- 每次回報列出這輪改了哪些文件及改了什麼。
- 背景 harness、離屏 render 與擷取不是前景 App、高對比、多螢幕、讀屏或實際安裝的驗收，不要這樣描述。

## 資料與實機

- 私人詞典不進內建詞庫、模型、安裝包或 Git。
- 會動到實機的測試先說明並取得同意。不控制目前使用者的核心、不廣播測試設定、不清除真實學習資料；測試用獨立的 `LOCALAPPDATA` 與 `SHANJIE_TEST_PROFILE`。
- 測試啟動的核心與加詞輔助程式（路徑在 `build/` 或 `dist/` 下）跑完要停掉，不碰 `Program Files` 裡的。

## 驗證：做多少

依改到的地方挑，不必每次全跑：

| 改到 | 跑 |
| --- | --- |
| 只有文件、TODO、Changelog、註解 | 不建置、不測試 |
| `platform/`、prepare 腳本 | `test-core.ps1`；只動某個測試的 adapter 時用 `-Tests` 只跑那個檔 |
| `shared/`、`windows/src` 的邏輯 | 對應 crate 的 `cargo test --lib`，加上相關的 `verify-*.ps1` |
| 按鍵路徑或核心請求 | 再跑 `measure-engine.ps1` |
| `ui/` | `verify-ui.ps1` |
| 一輪變更完成、準備 commit 或發布 | x64 `verify.ps1` 一次；發布前或改到 x86 相關時加 `verify.ps1 -NativeOnly -Architecture x86` |

### 建置與 Rust

- 平常只用 `cargo check`、`cargo test --lib`（加 `--target x86_64-pc-windows-msvc`），不跑 release build。
- 完整建置（`build.ps1`）只在使用者要求或要交給使用者測試時做，而且一定連單檔安裝程式一起建，不留只建一半的 `dist/`。
- 全域 `~/.cargo/config.toml` 已設定：registry 快取每日自動 GC、dev/test 只留行號除錯資訊、相依套件 `opt-level = 1`。不要在 repo 裡設 `target-dir`（腳本寫死 `windows/target`）。
- 清掉 30 天沒用的 target：`pwsh -File ~/.cargo/clean-stale-targets.ps1 -WhatIf` 先預覽。

## 多餘的驗證（不要做）

以下都是過去 commit 做過、後來刪掉或證明沒有用的：

- **驗證紀錄與證據文件**：`VERIFICATION.md`、`docs/verification.md`、`docs/releases/VERIFICATION-*.md`，以及「record … verification／evidence／boundary」這類 commit（歷史上 26 個；0.13.x 有四個 commit 只為在證據檔加一兩行）。結果在回報裡說，未驗的寫進 TODO，不另寫紀錄。
- **以版本號命名的一次性測試**：`features_090`、`features_0100`、`compat_0122` 與對應的 `verify-features-*.ps1` 後來都改名併入 `verify.ps1`。新回歸加進以功能命名的既有 harness，並由 `verify.ps1` 呼叫，不另開只跑一次的腳本。
- **沒人讀的 fixture 與參數**：`tests/fixtures/classes-v5` 沒有任何測試引用、harness 的模型參數沒有用到，都已刪除。新增 fixture 必須有測試讀它。
- **同一行為測兩次**：加詞浮層的唯讀與密碼欄在 `add_word_popup` 與 `tsf_compat` 各測一次，前者依賴真實桌面而時好時壞，已刪。一個行為只放在最適合的 harness。
- **重跑已通過的套件求心安**：已通過的不再跑第二次；偶發失敗重跑一次，通過就查原因或記下，不連跑多次。
- **替上游寫 Windows 測試**：上游的核心測試已在 Windows 上整套執行（`test-core.ps1`），只為 Windows 擴充寫測試，不改寫上游斷言。
- **與改動無關的量測與擷取**：沒改到按鍵路徑不量延遲；沒改到介面不做截圖或離屏 render。
- **只改文件也建置**：純文件 commit 不建置、不跑測試、不重新打包。

## GitHub 專案

- 正式 repo：https://github.com/tavricccc/shanjie-multiplatform （GitHub repository ID：1418506561），協作與發佈以此為準。
- 主要分支為 main。

## CodeGraph

- 結構理解、符號搜尋、靜態函式呼叫關係與依賴／變更影響分析，優先使用 CodeGraph（`codegraph_explore`；單一符號或檔案用 `codegraph_node`）。結果不足或語言不支援時，再用原始碼搜尋等工具補足；靜態關係不代表執行期行為。
- 使用前確認目標是本 Git 根目錄 `D:\Software\shanjie-windows-sourcecode\shanjie-windows`，必要時明確傳入 `projectPath`；外層工作區不是索引根目錄。
- 後續聊天重用 `.codegraph/`，先以 `codegraph_status` 檢查索引與 watcher 狀態；自動更新落後時執行增量 `sync`，不要每次 `init` 或完整 `index`。本機 CLI：`& '..\.build-tools\codegraph\1.6.2\codegraph-win32-x64\bin\codegraph.cmd' status .`（將 `status` 換為 `sync` 可增量更新）。
- MCP 連線存續時 watcher 自動更新；沒有連線時不常駐，重開後先檢查更新狀態。從 CLI 執行時同樣設定 `$env:CODEGRAPH_TELEMETRY='0'`、`$env:CODEGRAPH_NO_UPDATE_CHECK='1'`，避免遙測與使用者層級的版本檢查快取。
- 遵守既有專案規則、權限與核准限制；MCP 不擴大寫入、部署或系統變更權限。PowerShell、XAML 等未受支援內容仍需直接查原始檔。
