# 維護規則

這份是唯一的工作規則；其他文件只描述目前的程式與待驗收項目。使用 PowerShell 7、繁體中文，遵循使用者的模組化、必要測試與介面 skill 要求。

## 開始前

- 先看開啟中的 issue；動到核心或上游前讀 [UPSTREAM-CHANGES.md](UPSTREAM-CHANGES.md) 與 `UPSTREAM.json`。架構與驗證指令見 [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)。
- 查上游（Nanako0129/shanjie 已無法存取，改查鏡像 tavricccc/shanjie-upstream-backup）有沒有比 `UPSTREAM.json` 新的 release，有就提醒使用者。
- 定位是移植：功能、預設值與上游一致；與上游不同的預設或行為（新增或改變的設定）要明確告訴使用者，並記在 UPSTREAM-CHANGES.md。

## 上游與核心

- `upstream/shanjie` 是固定 commit 的子模組，追蹤檔案保持乾淨；不把上游整棵 merge 進來。
- **上游的詞庫、詞包與模型一律完整出貨，並照上游預設啟用。** 同步到的新版有新增或更新的檔案，就必須接上；不得因相容、體積或偏好而停用、略過或改預設值。真的有必要時先告訴使用者，並記在 UPSTREAM-CHANGES.md。`scripts/verify-upstream-data.ps1`（CTest 的 `fast.upstream-data` 與 `full.dist-data` 會呼叫）會比對，上游新增詞包或模型時它會失敗，要同步更新它的清單。
- Windows 對核心的變更放在 `platform/` 與 `scripts/prepare-*.ps1`。`build/core` 是產生檔，不手改。這些是核心擴充，不要描述成「只改 config」。
- 升版：
  1. 本 repo 與子模組都乾淨。
  2. 子模組 fetch 並選定 commit，比較 C header、候選範圍、學習序列化、LM parser 與授權。
  3. checkout 新 commit，同步更新 `UPSTREAM.json`（`build.ps1` 拒絕不在 pin 上的子模組）。
  4. 跑 `prepare-core.ps1`；錨點失敗代表覆蓋要重新審查。
  5. 跑 `pwsh -File scripts/test.ps1 -Level FULL`；Release 前再跑 `-Level RELEASE`。
  6. 分批 commit：pin 與覆蓋、Windows 功能、文件；更新 UPSTREAM-CHANGES.md。
- 不向上游提 PR；右側候選與平台 backend 等和作者討論 API 後再整理。

## 分支、Issue 與 Pull Request

- **不直接 push main。** 每個任務從最新的 `main` 開新分支，名稱 `type/short-name`（`feat/`、`fix/`、`docs/`、`test/`、`ci/`、`refactor/`、`chore/`），完成後開 PR 合入；不 force push 共用分支。
- 開工前先看有沒有對應的 issue 或別人的 PR；有就在 issue 留言認領，沒有就先開一個，避免兩個人做同一件事。
- PR 與 issue 的標題、內文、留言盡量用繁體中文；程式碼、檔名與專有名詞可保留原文，commit 訊息仍用英文。
- PR 標題格式 `type: 說明`（`type` 保留英文），內文用 [PR 模板](.github/pull_request_template.md)，關聯 issue 寫 `Closes #N`。一個 PR 只做一件事；功能與修正的 PR 不改版本號，發布另開升版 PR（見下方「版本號」）。
- PR 要先跟上最新 `main`（rebase 或 merge 都可以），並依下方「驗證」表跑過該跑的項目後再開；沒跑的在 PR 裡寫明。
- PR 的 `ci.yml` 與手動 `workflow_dispatch` 固定執行 `CI` 集合：上游資料、core overlay、shared／windows Rust 單元測試及低成本測試框架自檢，不依變更擴大、不建置完整產品或下載／轉換模型。維持 `ci`／`unit-tests` 名稱；一般分支 push 不觸發。完整 CTest 由本機 Windows 執行。既有 Release workflow 的版本發布條件與預設僅驗證模式保留；`RELEASE -CiSafe` 略過的互動項目須在本機補驗，不能宣稱通過。CI artifact 附 JUnit、日誌與 JSON 報告。合入時，已是每個邏輯變更各自一個 commit 的用 merge，其餘用 squash。
- Bug 與功能建議用 [issue 模板](.github/ISSUE_TEMPLATE/)；問題要附版本、Windows 版本與重現步驟。

## Commit、push 與發布

- 一個邏輯變更一個 commit（行為連同它的測試、UI 文字、README、Changelog 各自分開），以 `git add <檔案>` 分組，訊息用英文 `type: description`。完成並測試後直接 commit，不必先問。
- push、開 PR、Release、安裝到實機、刪除使用者資料都先問。
- 完整建置與 Release 只產生單檔安裝程式 `Shanjie-Windows-Setup.exe`；要使用者測試時一併建好它。輕量 CI 不建置安裝包。
- **個人編譯與正式驗收分離：** 使用者僅要求編譯、確認產物或建立開發用候選包時，不得擅自啟動完整 FULL／RELEASE 驗證。涉及程式修改時，依變更範圍執行必要的 FAST／AFFECTED 測試；已完成且有效的相同驗證不得因 Markdown 舊指令而重複執行。只有準備 PR 整合、正式發布，或使用者明確要求時，才執行相應完整驗收。若 AFFECTED 計畫擴大為 FULL，而本次只授權編譯，先保留待驗項目，不因編譯需求直接啟動；正式產物仍須遵守下列發布防護。

### 本機正式發布防護

- 「打包新版本」的本機入口是 `pwsh -File scripts/release.ps1`。它只重用同 commit、同四處版本、乾淨工作樹、完整 x64／x86 RELEASE 清冊與 JUnit 通過、所有子案例已執行，且建置檔／候選安裝包／JUnit 雜湊仍一致的紀錄；無效就重新建置並跑 RELEASE，有效則不重跑。
- `package.ps1` 預設只產生 `build/release-candidates/<id>/` 的候選包。RELEASE 在這裡驗證候選包，不呼叫正式發布入口；整套通過後才可由 `package.ps1 -Promote -ReportPath <report.json>` 的程式防護提升至 `dist/Shanjie-Windows-Setup.exe`，並寫 `dist/release.json`。
- `status=passed`、不完整清冊、跳過／未執行、CI-safe、舊 commit／版本或變更過的產物均不能授權本機正式發布。正式動作不得直接繞過此入口；不得把候選包當成正式發布完成。
- GitHub Actions 的 RELEASE -CiSafe 只匯出標示為 CI artifact 的產物，不產生本機完整驗收標記；日常 PR 仍用固定輕量 CI。遠端 push／tag／Release 仍須使用者授權。

### 版本號

- 版本號在 `windows/Cargo.toml`、`windows/Cargo.lock`、`ui/Shanjie.UI.csproj`、`RELEASE-CHANNEL.json`，四處與 tag 必須一致。
- **發布走升版 PR**：功能 PR 都合入 main 後，另開一個只含版本號、`docs/releases/RELEASE-NOTES-<版本>.md` 與 `Changelog.md` 的 PR；它合入 main 時 Actions 建立 Release。其他 PR 不碰這些檔案。既有 Release 不會被重跑改寫，要改說明或 pre-release 狀態用 `gh release edit`。
- **預發布不遞增版本號。** 下一個要發布的版本固定為 `X.Y.Z`（目前正式版之後的那一版），預發布只加後綴：`X.Y.Z-beta.1`、`X.Y.Z-beta.2`……，`RELEASE-CHANNEL.json` 的 `prerelease` 設 `true`，tag 為 `vX.Y.Z-beta.N`。例：正式版是 0.15.3，預發布依序為 `0.15.4-beta.1`、`0.15.4-beta.2`，不是 0.15.4、0.15.5。
- 轉正式版時只拿掉後綴成 `X.Y.Z`，`prerelease` 設 `false`，`recommended_stable` 改成該版。beta 期間不更改 `X.Y.Z`。
## 文件

- 文件只寫目前狀態並保持精簡。各版變更寫在 `Changelog.md` 與 `docs/releases/`；待辦與需要實機確認的項目開 issue，能用腳本測的先自己測。
- 每次回報列出這輪改了哪些文件及改了什麼。
- 背景 harness、離屏 render 與擷取不是前景 App、高對比、多螢幕、讀屏或實際安裝的驗收，不要這樣描述。

## 資料與實機

- 私人詞典不進內建詞庫、模型、安裝包或 Git。
- 會動到實機的測試先說明並取得同意。不控制目前使用者的核心、不廣播測試設定、不清除真實學習資料；測試用獨立的 `LOCALAPPDATA` 與 `SHANJIE_TEST_PROFILE`。
- 測試啟動的核心與加詞輔助程式（路徑在 `build/` 或 `dist/` 下）跑完要停掉，不碰 `Program Files` 裡的。
- WinUI 驗證須在建立子程序前設定獨立 `LOCALAPPDATA`、`SHANJIE_TEST_PROFILE` 與行程內核心，不能等 XAML 啟動後才隔離；不得掃描工作副本外的核心作為清理對象。

## 驗證：做多少

所有一般驗收統一由 `scripts/test.ps1` 和 CTest 管理。本機 AFFECTED 依 `scripts/test-plan.psm1` 的影響規則選測；規則無法明確判斷時擴大到 FULL。GitHub Actions 使用固定 CI 集合，沒有這項擴大機制。變更影響表與 CTest 案例清冊見 [docs/TESTING.md](docs/TESTING.md)。

日常未提交修改使用 `-ChangeScope Worktree`，單次提交使用 `-ChangeScope Commit`；兩者都包含 staged／unstaged／untracked 修改，但不代表整條分支已驗證。預設 `Branch -BaseRef origin/main` 保留上游累積比較語意，PR／整合用它或 FULL；不能靜默忽略分支中尚未驗證的舊變更。已含 CMake 重構的分支若因此擴大，先看 PlanOnly 的範圍與理由，再按本次目的明確選擇範圍。

上游的 `verify-changed.ps1` 保留為 AFFECTED 相容入口，`-DryRun` 對應 `test.ps1 -PlanOnly`，只輸出計畫、狀態為 `planned`，不能當成測試成功。明確要求 x86 時對應雙架構 RELEASE。`verify-selection.ps1` 匯入同一份影響規則，不再維護第二套 Steps／Suites 排程。

| 時機／變更 | 跑 | 用途 |
| --- | --- | --- |
| 自動 PR／手動輕量 CI | `pwsh -File scripts/test.ps1 -Level CI -CiSafe` | 固定必要基線，不分析分支差異、不擴大至 FULL、不建置產品 |
| 日常程式修改 | `pwsh -File scripts/test.ps1 -Level FAST` | 靜態資料清冊、核心 overlay 與共用／Windows Rust 單元測試；純文件不建置、不測試；程式碼檔不以行首符號判定純註解 |
| 功能完成 | `pwsh -File scripts/test.ps1 -Level AFFECTED` | FAST 加上依變更模組選出的完整相關回歸；未知影響自動擴大至 FULL |
| PR 或整合前 | `pwsh -File scripts/test.ps1 -Level FULL` | 完整 x64 驗收，保留上游核心、模型、TSF 與每項既有回歸斷言 |
| Release 前 | `pwsh -File scripts/test.ps1 -Level RELEASE` | FULL、x86 回歸、離屏互動 harness、WinUI、候選 host 與安裝包檢查 |

### 條件式驗證與去重

- **修改按鍵路徑或核心請求時，仍須額外執行 `pwsh -File scripts/measure-engine.ps1`。** 一般功能測試或核心內部延遲斷言不等於其 IPC／每鍵量測；未改這些路徑不需量測。
- `ui/` 變更由 AFFECTED 的 `settings-ui`＋`candidate-host` 負責；原生候選變更依上游選 `candidate-host`＋`tsf-compat`，不擴成整套 WinUI。`platform/`／`prepare-core.ps1` 保留 Rust、core、混打、英文／提交與額外延遲量測，不一律擴 FULL。FULL 單獨不含全部 `release.ui.*`；缺少必要項目時，只透過 CTest 補缺項，不重跑舊 wrapper。
- 去重依實際斷言、架構、參數、fixture、產物／來源與環境。以仍有效的 JSON／JUnit／子案例結果認列已通過項目；只有登記或選中、PlanOnly、跳過與失敗都不算完成。同一驗證不因不同腳本名稱再跑一次；部分涵蓋時，只補尚未驗證且確實必要的配置。無法拆分的工具先說明重複範圍，不直接啟動整套。
- 其他專用 fixture、故障注入、舊版共存、視覺／實機與選用量測的目的、前置條件及選測參數見 [docs/TESTING.md](docs/TESTING.md#專用驗收與實際差集)。上游未訂逐檔強制規則者需依實際變更判斷，不能一律加跑。
- 自動提示規則只在 `scripts/test-plan.psm1` 維護。新增或修改 Markdown 驗證要求時，須同步程式規則與 `tests/verify_selection.ps1`；PlanOnly 只能提示規則文件變動，**不會理解任意自然語言或自動把新條文轉為測試**。額外提示不是 PASS，也不改變 CI 固定集合。

### CTest 操作

```powershell
cmake --preset windows-x64
cmake --build --preset windows-x64 --target shanjie_tsf_smoke
ctest --preset windows-x64 --show-only
ctest --preset windows-x64 -L tsf --output-on-failure
pwsh -File scripts/test.ps1 -Level AFFECTED -RetryFailures
```

`-RetryFailures` 只對失敗 CTest 項目最多重跑一次；必要 fixture 由 CTest 重新加入。兩次 JUnit／日誌各自保留，JSON 的 `attempts` 與 `flaky_tests` 保留首次失敗。英文鍵與提交測試的全部模式／鍵盤／按鍵組合由同一份清冊產生執行計畫與 `subcases` 結果；未執行不算通過。每次分級驗收都在 `build/reports/<run>/report.json` 記錄層級、選中與未選測試的原因、結果、耗時、快取資訊與 JUnit 位置；CTest 原始日誌和 JUnit XML 也保存在同一個 ignored build 目錄。回報必須列出實際跑過與未跑的測試及原因。不得把未執行的 Windows／TSF 測試說成通過。

`scripts/verify.ps1` 與 `scripts/test-core.ps1` 保留作為舊入口、核心除錯與 benchmark 對照；日常與 PR 驗收使用上列分級入口。上游核心測試原有的記憶體使用限制排程仍保留，混打測試的快取身分斷言使用獨立 Cargo 程序，避免其他案例淘汰它正在比對的快取；`engine_lm` 與 `engine_replay` 延遲斷言仍各自序列執行。CTest 對模型使用、核心 fixture 與 UI 工作加鎖；私人桌面案例各用獨立 profile，不註冊輸入法、不切換桌面、不讀寫真實使用者資料。

保留上游 #100 的核心通過結果快取與只同步變更內容：AFFECTED／直接呼叫 `test-core.ps1` 可重用完全一致的 executable、參數、資料、模型及日誌；報告須明列重用、沒有重跑斷言。FULL／RELEASE 及 benchmark 必須實際執行核心斷言，不能將結果快取命中當成同覆蓋率加速。直接使用 `test-core.ps1 -Rerun` 可強制執行。保留 #96 的工具私人副本與 WinUI 啟動前隔離；截圖只在 `verify-ui.ps1 -Visual` 要求時產生。

### 建置與 Rust

- FAST／AFFECTED 由 CTest 呼叫 Cargo 原生測試；FAST 是快速基線，AFFECTED 依變更再選相關回歸；Rust 測試框架仍由 Cargo 管理，不另造一套。
- `build.ps1`、Cargo、NuGet／dotnet、模型與資料準備都有內容或工具鏈驗證過的增量快取；不要只因產物存在就跳過驗證。
- Release 單檔安裝包由 RELEASE 驗收建立並校驗；一般修改不強制打包。
- 全域 `~/.cargo/config.toml` 已設定：registry 快取每日自動 GC、dev/test 只留行號除錯資訊、相依套件 `opt-level = 1`。不要在 repo 裡設 `target-dir`（腳本寫死 `windows/target`）。
- 清掉 30 天沒用的 target：`pwsh -File ~/.cargo/clean-stale-targets.ps1 -WhatIf` 先預覽。

## 多餘的驗證（不要做）

以下都是過去 commit 做過、後來刪掉或證明沒有用的：

- **驗證紀錄與證據文件**：`VERIFICATION.md`、`docs/verification.md`、`docs/releases/VERIFICATION-*.md`，以及「record … verification／evidence／boundary」這類 commit（歷史上 26 個；0.13.x 有四個 commit 只為在證據檔加一兩行）。結果在回報裡說，未驗的開 issue，不另寫紀錄。
- **以版本號命名的一次性測試**：`features_090`、`features_0100`、`compat_0122` 與對應的 `verify-features-*.ps1` 已整併至功能命名的 harness。新回歸加進適合的既有 harness，並在 CMake／CTest 登記穩定案例，不另開一次性腳本。
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

- 有 `.codegraph/` 索引時，結構、符號與靜態呼叫關係優先使用 CodeGraph；明確傳入 `git rev-parse --show-toplevel` 得到的專案根目錄，不使用其他開發者的工作目錄。
- 先檢查索引／watcher 狀態，必要時增量 sync，不反覆 init／完整 index。沒有索引時直接讀原始碼，不為一般驗收自動建索引。
- CLI 從 PATH 尋找，執行時設定 `CODEGRAPH_TELEMETRY=0` 與 `CODEGRAPH_NO_UPDATE_CHECK=1`。PowerShell、XAML 等不受支援內容直接檢查原始碼；靜態關係不等於執行期行為。
- 工具鏈以 PATH、vswhere、CMake 偵測或明確環境設定取得。個人驗收腳本可留在 ignored build/，正式腳本、CMake 與 Actions 不得引用個人路徑。
