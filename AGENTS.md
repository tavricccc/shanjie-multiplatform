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

## Commit、push 與發布

- 一個邏輯變更一個 commit（行為連同它的測試、UI 文字、README、TODO、Changelog 各自分開），以 `git add <檔案>` 分組，訊息用英文 `type: description`。完成並測試後直接 commit，不必先問。
- push、Release、安裝到實機、刪除使用者資料都先問。
- 版本號在 `windows/Cargo.toml`、`windows/Cargo.lock`、`ui/Shanjie.UI.csproj`、`RELEASE-CHANNEL.json`；push 到 main 且版本號變更時 Actions 建立 Release。發布前寫好 `docs/releases/RELEASE-NOTES-<版本>.md` 與 Changelog。既有 Release 不會被重跑改寫，要改說明或 pre-release 狀態用 `gh release edit`。
- 建置、CI 與 Release 只產生單檔安裝程式 `Shanjie-Windows-Setup.exe`；要使用者測試時一併建好它。

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
