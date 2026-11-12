# CTest 交接（2026-11-10）

目前整合分支：`refactor/ctest-upstream-integration`，遠端為 `HolyBearTW/shanjie-multiplatform`。從原 CTest 分支的 `80c019dd2d900299b8dc2a905eccb2f04d49f97d` 建立，合併上游 `tavricccc/shanjie-multiplatform/main` 的 `4794ab3c54f9edb018885112381ad9c8341c07bc`（0.16.0-beta.2）。原 `refactor/ctest-verification` 停留在 80c019d，保持不變；main 未修改。整合 SHA 以包含本文件的 `git rev-parse HEAD` 為準，機器摘要見 JSON 的 `latest_integration`。

**整合版尚未完成任何 Windows 原生驗收，也未執行 Benchmark。** 不得把下方舊 Windows 成功紀錄或 Cloud 框架檢查套到目前 HEAD。固定的 `upstream/shanjie` 子模組與產品原始碼沒有加入本次測試整合的行為修改；產品目錄與此次上游提交一致。

## 本輪上游整合

- 解決 CI、Release、模型、核心及各驗證腳本的衝突；CMake／CTest 保持 Windows／MSVC 條件，不為 Linux 放寬。
- #86：保留上游 `release.py`、13 個 FakeGitHub 交易測試及發布 workflow 的 plan／publish、同來源草稿續作、SHA-256／來源 manifest、run-attempt artifact 防護。僅 Windows 驗收步驟接入 CTest；既有 Linux job 原樣保留。
- #96：保留工具私人副本、不同架構、WinUI 啟動三情境與三種鍵盤 initial／reopen 的停用控制項斷言；使用 CTest 原有的啟動前 profile／行程內核心隔離，不掃描或停止工作樹外的核心。
- #100：保留 legacy harness 快取、核心測試來源只同步變更內容、通過結果快取、預設不截圖。AFFECTED 可明示重用一致的核心結果，FULL／RELEASE 強制執行斷言。上游 `verify-changed`／`verify-selection` 共用 CTest 規則，避免兩套選測排程。
- 補登 `digit-navigation` 兩種混打模式及六個 WinUI CTest 項目。預期 FULL 87；RELEASE x64 99＋x86 84＝183，參數子案例 227。此為靜態清冊計數，須 Windows configure／實跑確認。
- Cloud 已通過 PowerShell 7.5.4 的 63 支語法解析、34 項整合檔案／快取檢查、19 項選測案例、34 項 mock Cargo／真實 Git 搬移檢查及上游 13 個 Python 發布交易測試。未執行 MSVC、WinUI、TSF、x64／x86 或 RELEASE。

## 已完成內容

- CMake／CTest 保留本機 FAST、AFFECTED、FULL、RELEASE、增量建置、模型／資料／UI 內容快取、資源鎖與平行排程；修正 PowerShell 語法、模組匯出、x86 staticlib 路徑、參數陣列與乾淨 WinUI 輸出的空陣列問題。
- AFFECTED 的未知影響保守擴大到 FULL；FAST 維持快速基線。報告記錄 fixtures、全部英文／提交組合、首次失敗與重試、JUnit、原始 CTest 日誌、包含與自身耗時。
- 日常 `ci`／`unit-tests` 固定 `test.ps1 -Level CI -CiSafe` 五項：上游資料、core overlay、shared／windows Rust 單元測試及框架自檢。PR 到 main／PR 更新與手動 dispatch 執行；一般分支 push 不執行。沒有 FULL 擴大、完整產品／模型／WinUI／TSF／安裝包建置、大型 target 快取或完整歷史下載。
- 本機正式產物走 `scripts/release.ps1`；`package.ps1` 預設只做候選包。提升正式產物前核對乾淨來源、commit、四處版本、x64／x86 必要清冊、實際 JUnit／子案例及產物雜湊；CI-safe、缺測、舊紀錄不能授權。有效紀錄直接重用，避免 package 與 RELEASE 循環。
- 測試使用獨立 profile／私人桌面及自有核心，WinUI 在建立子程序前隔離。修正 Windows 單元測試共享 sink 的平行衝突、混打快取身分測試的程序隔離，以及加詞浮層隱藏早於詞典存檔的實際競態；最後一項僅以既有有界等待保留原內容斷言，沒有修改產品行為。

## 原 CTest 分支 P2 修正（80c019d，保留）

- `build.ps1` 的產品 Cargo build 明確指定絕對 `--target-dir <工作樹>/windows/target`，覆蓋外部 `CARGO_TARGET_DIR`；沿用既有增量目錄與兩個 `--target` 子目錄，和 DLL／工具複製及 CMake staticlib 位置一致。
- `test.ps1` 的 committed、staged、unstaged diff 使用 `--no-renames`，將搬移視為刪除加新增；原、新路徑都參與 FAST／AFFECTED 判斷。純文件修改與文件重新命名仍可略過建置／測試。
- `tests/verification_paths.ps1` 以真實 Git fixture 與 mock Cargo 產物驗證上述路徑，已接入既有 `fast.infrastructure`，CI 仍為固定五項。Linux PowerShell 7.5.4：34 項檢查通過，59 支 `.ps1`／`.psm1` 語法解析零錯誤；分別換回原版 build／test 腳本時，回歸測試都能拒絕原問題。可單獨執行 `pwsh -File tests/verification_paths.ps1`。
- 本輪未編譯 Rust／MSVC，未執行 Windows TSF／WinUI／x86 或任何 Benchmark。四輪完成數據、案例集合雜湊及原始證據路徑完整保留；P3 計時與其他最佳化暫緩。

## Windows 驗證紀錄

| 項目 | 結果與提交 |
| --- | --- |
| 最新完整、乾淨的 RELEASE | `bed03c39ffe06418e7f7ac06de90ccd3a0c9c88e`，175／175 CTest、214／214 子案例通過，x64／x86，非 CI-safe，1527.076 秒（25:27.076）；包含完整 x64 FULL。報告：`build/reports/release-20261109-223421-6b6f5fed/report.json`。**對最新 HEAD 已失效。** |
| RELEASE 提升／重用 | 在 bed03c3 實測重用完整紀錄並提升同一候選包；沒有再跑 RELEASE。舊／不完整收據、改 commit 與改產物的拒絕檢查已測。新 HEAD 不得沿用當時的 `dist/release.json`。 |
| 加詞存檔修正 | `07c43a4ddd01be09225a720ac0798f34d833b045`，x64／x86 原生加詞 harness 均通過；完整最新 FULL／RELEASE 尚未完成。 |
| 輕量 CI，本機乾淨 worktree | `9125d266ef34f967662774dd085f6c9d81882767` 冷快取 51.533 秒，bed03c3 暖快取 5.735 秒，均為固定 5／5。不是 GitHub Runner 實測。舊 GitHub job 為 151／137／154 秒；新版遠端尚未量測。 |
| 靜態檢查 | 58 支 PowerShell 檔案 Parser 零錯誤，3 份 workflow YAML／觸發條件／固定 CI 邊界檢查通過；零完成報告但 exit 0 的程序會記為失敗。續跑識別前四輪已實測。 |

早期 AFFECTED→FULL 曾 86／86 通過，但當時工作樹有未提交修改；同步後另一次為 85／86、加詞失敗。這些原始失敗紀錄均保留，不能冒充乾淨提交的成功驗收。真正可對應乾淨 commit 的完整紀錄以上述 bed03c3 RELEASE 為準。

## 六輪 Benchmark 與保留資料

原始 baseline 是未修改的 `64c22ef` verify，加原 CI 的兩套單元測試，當時用來對照原 CTest FULL。四個完成結果各有 1,003 個實際產品案例／組態，識別碼集合相同；數值、集合雜湊與證據位置原封不動保留在 [精簡機器報告](ctest-handoff-results.json)。**合併最新上游後產品來源及案例已變，四輪現在只能作歷史參考，不是目前整合版的公平比較基準。**

| 輪次 | 狀態 | 實測時間 | 對應提交／原始資料 |
| --- | --- | --- | --- |
| baseline_cold | 完成，保留不重跑 | 1404.563 秒（23:24.563） | `64c22ef`；`75e8e4557bf041599d07e12a55934e55` |
| baseline_warm | 完成，保留不重跑 | 1253.556 秒（20:53.556） | 同上；純原始 verify 分別為 1388.292／1243.177 秒，差額包含補充 CI 與執行器成本 |
| legacy_cold | 完成的歷史參考，保留不重跑 | 1302.611 秒（21:42.611） | `1ca8c8d4c5a02b6b48d05e1f1d7641c62164667d`；`913d804fd8bd490fb77f032cdefe2fee` |
| legacy_warm | 完成的歷史參考，保留不重跑 | 1004.494 秒（16:44.494） | 同上；不是目前測試來源的驗收，禁止當作同 commit 的加速比較 |
| ctest_cold | 尚未完成 | 無有效完整時間 | d63dc62 曾於 336.277 秒失敗，加詞檢查 747；`6fec982b39f3473fb9da59eed16958f4`。07c43a4 已修正並通過局部 x64／x86，但完整冷快取仍待執行 |
| ctest_warm | 未執行 | 未量測 | 必須先完成目前 CTest 冷快取；不得宣稱已加速 |

另一次 07c43a4 的 legacy_cold 為交接主動中斷，部分經過時間 964.562 秒，**不是完成的效能數據**；位於 `ee01a3ce6efb40eaac656f7cc9e4b459`，不能覆寫上表已完成的歷史結果。

以上 GUID 目錄均相對於本機 `SHANJIE_BENCHMARK_ROOT`；內有 `comparison.json`、`artifacts/<輪次>/worktree/build/` 的 report、JUnit／CTest 日誌，以及 `evidence/`。主工作樹的路徑指標、詳細狀態在 `build/reports/ctest-review-preflight/`。

原始資料還另外備份至本機驗收目錄下 `handoff/ctest-windows-evidence-20261110-014621.zip`：10,603 個文字／JSON／XML／日誌檔，19,825,459 bytes，SHA256 `8fb2d34ea9cb0a839517bf9ac84920d2bcba5340d604e43d78eff887201446c4`。實際完整位置由 ignored `build/reports/ctest-review-preflight/handoff-backup.json` 記錄。**Push 不包含原始日誌或這份備份**；Cloud 可讀本文件與精簡 JSON，原始資料須回原 Windows 主機取得。原始報告有本機路徑，不直接加入版控。

## Windows 原機接續

先確認乾淨工作樹及工具鏈。舊版量測曾因防毒自動恢復而提前中斷，原因未完全確定；沒有完成報告就保留失敗。正式腳本沒有個人路徑相依，ignored 的本機工具及原始證據不刪除。

```powershell
git switch refactor/ctest-upstream-integration
pwsh -File scripts/test.ps1 -Level AFFECTED -BaseRef 4794ab3c54f9edb018885112381ad9c8341c07bc -PlanOnly
pwsh -File scripts/test.ps1 -Level FULL
pwsh -File scripts/test.ps1 -Level RELEASE
```

Windows 待驗收：

1. MSVC harness 增量建置與 x64／x86 分離、Rust／模型／核心全套、TSF／私人桌面回歸、新增 digit-navigation。既有產物的 tools junction 應只封存連結本身，改為私人副本。
2. WinUI clean／reopen／不同工作目錄，三種鍵盤初次／重開的停用設定與非標準鍵盤斷言；預設不截圖仍必須執行全部斷言。需要圖片時才使用 `verify-ui.ps1 -Visual`。
3. AFFECTED 對一致輸入的核心結果快取應清楚回報重用；FULL／RELEASE 必須重新執行。修改 executable／資料／模型／參數／日誌要失效，失敗重跑不得留下有效成功收據。
4. 最終乾淨提交的完整 RELEASE 與候選包。可在同一輪設定隔離 `CARGO_TARGET_DIR`，確認真實 DLL／工具／staticlib 仍在 `windows/target`，再還原環境；不為此多跑整套。舊 bed03c3 收據及 CI-safe 均不能提升本機正式產物。受控的本機 `release.ps1` 重用／拒絕測試留待另行授權，這次不發布或安裝。
5. GitHub Windows Runner 的固定五項 CI 與 Release `-CiSafe`。Cloud 沒有執行遠端工作；CI-safe 省略的 WinUI／candidate-host 項目必須由本機補驗，不能冒充完整 RELEASE。

### Benchmark 邊界

舊四輪已完成資料、失敗冷快取及備份位置全部保留，本輪未重跑。舊版「只續跑兩輪」命令只適用原本產品／測試來源；目前整合版應由現有產品身分防護拒絕和它們混合比較，不要繞過這項檢查。原機仍可在保留的原 CTest 分支補其冷／暖兩輪，但結果不能證明本整合版已通過或加速。

若之後要對整合版建立公平比較，須用相同產品的上游 `4794ab3c54f9edb018885112381ad9c8341c07bc` 作新 baseline，另建量測集合；不覆寫、不重跑舊四輪。指令見 [TESTING.md](TESTING.md#新舊耗時比較)，此次未啟動。#100 的結果快取不得使暖快取只跑空殼：benchmark 只對新量測工作樹封存核心成功收據，保留編譯／模型快取，CTest FULL 強制重跑核心斷言。ResumeReport 匯入的完成輪次不執行、不碰證據。尚無整合版耗時或加速百分比，P3 計時問題仍暫緩。

實際安裝、前景 App、多螢幕、高對比、讀屏，以及未納入清冊的特殊測試仍未驗。只在新功能分支提交／推送，不更動 main 或原 CTest 分支，不 force push、不建立 PR 或發布。
