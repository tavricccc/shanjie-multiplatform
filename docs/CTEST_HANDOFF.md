# CTest 交接（2026-11-10）

分支：`refactor/ctest-verification`；遠端：`HolyBearTW/shanjie-multiplatform` 的同名既有分支。本輪 P2 修正以原交接 `8c5d0030bd4b98f12055b21a043c203acd6617c2` 為基底；最新 SHA 以包含本文件更新的 `git rev-parse HEAD` 為準。JSON 的 `code_checkpoint` 保留原 Windows 交接前的歷史檢查點 `688f1cb72024c5c48c129d77b3df38041a0b62e5`；本輪變更與驗證見 `latest_fix_update`。既有歷史已合入 `64c22ef536ae5f106e684bcd31e8ff0a36233838`；本輪沒有同步上游或修改 main。

**最終 Commit 尚未完成 RELEASE 驗收。** 不得將較舊的成功紀錄套到最新提交，也不得把 Linux Cloud 的檢查當成 Windows RELEASE。使用者要求台灣時間 02:00 硬性停止；本次測試已提前停止，不應留下背景測試。

## 已完成內容

- CMake／CTest 保留本機 FAST、AFFECTED、FULL、RELEASE、增量建置、模型／資料／UI 內容快取、資源鎖與平行排程；修正 PowerShell 語法、模組匯出、x86 staticlib 路徑、參數陣列與乾淨 WinUI 輸出的空陣列問題。
- AFFECTED 的未知影響保守擴大到 FULL；FAST 維持快速基線。報告記錄 fixtures、全部英文／提交組合、首次失敗與重試、JUnit、原始 CTest 日誌、包含與自身耗時。
- 日常 `ci`／`unit-tests` 固定 `test.ps1 -Level CI -CiSafe` 五項：上游資料、core overlay、shared／windows Rust 單元測試及框架自檢。PR 到 main／PR 更新與手動 dispatch 執行；一般分支 push 不執行。沒有 FULL 擴大、完整產品／模型／WinUI／TSF／安裝包建置、大型 target 快取或完整歷史下載。
- 本機正式產物走 `scripts/release.ps1`；`package.ps1` 預設只做候選包。提升正式產物前核對乾淨來源、commit、四處版本、x64／x86 必要清冊、實際 JUnit／子案例及產物雜湊；CI-safe、缺測、舊紀錄不能授權。有效紀錄直接重用，避免 package 與 RELEASE 循環。
- 測試使用獨立 profile／私人桌面及自有核心，WinUI 在建立子程序前隔離。修正 Windows 單元測試共享 sink 的平行衝突、混打快取身分測試的程序隔離，以及加詞浮層隱藏早於詞典存檔的實際競態；最後一項僅以既有有界等待保留原內容斷言，沒有修改產品行為。

## 本輪 P2 最小修正（Linux）

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

原始 baseline 是未修改的 `64c22ef` verify，加原 CI 的兩套單元測試，確保和 FULL 覆蓋一致。四個完成結果各有 1,003 個實際產品案例／組態，識別碼集合相同；數值與集合雜湊見 [精簡機器報告](ctest-handoff-results.json)。

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

## 下一步

Linux Codex Cloud：本輪指定的兩項 P2 修正及針對性檢查已完成；提交／推送此單一修正後停止。不得把這些檢查當成 MSVC／TSF／WinUI／x86／RELEASE 通過，也不要重跑或丟棄已完成的四輪數據。

Windows 原機續跑：先確認乾淨工作樹、工具鏈版本與原測試條件。舊版量測曾因防毒自動恢復而提前中斷，原因未完全確定；若再遇無完成報告，保留失敗，不冒充成功。正式腳本從 PATH／vswhere／CMake 或明確環境設定找工具，沒有個人路徑依賴；ignored `build/` 的環境與私人桌面 helper 僅是這台電腦的操作工具，不是程式相依。

在相同原機條件下，以下只補 CTest 冷／暖兩輪，沿用前四輪。歷史 legacy 與新 CTest 的百分比停用；主要比較仍為原始 baseline 對 CTest，六組實際案例／組態仍須相同。精簡 JSON 本身不是可用來發布的收據。

```powershell
$latest = (Get-Content build/reports/ctest-review-preflight/benchmark-current-root.txt -Raw).Trim()
$benchmarks = Split-Path $latest -Parent
$env:SHANJIE_BENCHMARK_ROOT = $benchmarks
pwsh -File scripts/benchmark-verification.ps1 -Commit HEAD `
  -ResumeReport "$benchmarks/ee01a3ce6efb40eaac656f7cc9e4b459/comparison.json" `
  -HistoricalLegacyReport "$benchmarks/913d804fd8bd490fb77f032cdefe2fee/comparison.json" `
  -KeepWorktrees
```

Benchmark 完成後，對最終乾淨 HEAD 跑 `pwsh -File scripts/test.ps1 -Level RELEASE`。可在這次 RELEASE 設定一個隔離的 `CARGO_TARGET_DIR`，補驗真實 x64／x86 產品仍輸出至本工作樹 `windows/target`、CMake 連結及打包正常，完成後還原環境；不要為此另跑整套驗收。新 HEAD 必須產生自己的完整收據，不能沿用先前 commit 的成功紀錄。再以 `pwsh -File scripts/release.ps1` 核對並重用該收據、提升同一候選包；確認沒有重跑有效 RELEASE。檢查當前 CI／FAST 與必要靜態檢查；沒有新的程式修改就不要為求心安重跑已過的套件。實際安裝、前景 App、多螢幕、高對比、讀屏，以及未納入清冊的特殊 ignored 測試仍未驗，不屬於已通過的 CTest 宣稱。

只在此功能分支工作；不修改或 push main，不 force push、不建立 PR 或正式發布。本機驗收目錄與無關工作檔不刪除。
