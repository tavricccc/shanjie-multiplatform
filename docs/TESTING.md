# Windows 驗收流程

Windows 測試由 CMake／CTest 統一列出、選擇與排程；Rust 仍由 Cargo 編譯和執行。Visual Studio 2022 的 x64、x86 Preset 使用不同 build 目錄與產物，不能共用架構相異的 harness。

## 常用指令

```powershell
pwsh -File scripts/test.ps1 -Level FAST
pwsh -File scripts/test.ps1 -Level AFFECTED
pwsh -File scripts/test.ps1 -Level FULL
pwsh -File scripts/test.ps1 -Level RELEASE
```

`FAST` 與 `AFFECTED` 讀取 `origin/main` 到目前 HEAD 的差異，也包含 staged、unstaged 和未追蹤檔案。可以用 `-BaseRef <ref>` 指定比較基準；分支已含廣泛變更時，`-BaseRef HEAD` 可只檢查目前工作樹，正式整合仍須 FULL。只有明確的文件檔可自動略過；程式碼檔不靠行首正則式判斷純註解。FAST 保持快速基線，AFFECTED 才依影響加選或擴大至 FULL。明確指定 FULL／RELEASE 時，一律執行完整集合，不套用文件略過規則。

GitHub Actions 固定使用 `pwsh -File scripts/test.ps1 -Level CI -CiSafe`，不讀分支差異，即使修改 CMake、測試框架或 CI 也不擴大。它執行上游資料完整性、Windows core overlay、shared／windows Cargo 單元測試及測試框架自檢；不安裝 .NET、不編譯 WinUI、不下載完整 Git 歷史、不傳輸大型 target／模型快取，也不做產品 release build、完整核心套件、TSF／UI 或封裝。保留既有 `ci` workflow 與 `unit-tests` job。

PR 指向 main 時，開啟、更新提交與重新開啟會跑輕量 CI；`workflow_dispatch` 也只跑相同集合。一般開發分支 push、本機編譯、commit 或 CTest 都不觸發 Actions。完整驗收在本機執行；未另加自動或手動 FULL workflow。既有 release workflow 仍只在 main 的版本檔變更或明確手動啟動時執行，手動預設僅驗證，發布仍受原本的版本、相依 job 與模式檢查約束。

直接使用 CTest：

```powershell
cmake --preset windows-x64
cmake --build --preset windows-x64 --target shanjie_tsf_smoke
ctest --preset windows-x64 --show-only
ctest --preset windows-x64 -L english --output-on-failure
ctest --preset windows-x64 -R 'full\.private\.popup-lifecycle\.' --output-on-failure
pwsh -File scripts/test.ps1 -Level AFFECTED -RetryFailures
```

CTest 單獨執行時，先用 `scripts/build.ps1 -Architecture x64` 準備產品、模型與資料；`scripts/test.ps1` 會依測試需要自動完成這些步驟。每次入口驗收在 ignored 的 `build/reports/<run>/` 產生 `report.json`、CTest 輸出日誌、每次執行的 `LastTest-<preset>[-retry].log` 和保留完整案例輸出的 JUnit XML。CTest 本身也能以 `-L` 標籤或 `-R` 名稱選測、`--repeat until-pass:2` 重跑失敗案例；預設不重跑已通過案例。

## 層級與變更影響

| 層級 | 選取內容 | 用途 |
| --- | --- | --- |
| CI | 固定上游資料、core overlay、shared／windows 單元測試與框架自檢 | 自動／手動輕量 CI；永不擴大，不建置產品 |
| FAST | 快速基線；明確的純文件變更可略過 | 本機日常快速錯誤檢查 |
| AFFECTED | FAST 加上變更對應的 Cargo／TSF／UI／Harness 回歸 | 功能完成後的相關驗收；無關 WinUI 的變更略過 UI publish |
| FULL | FULL 標籤的全部 x64 測試案例及產品建置 | PR、整合前完整 x64 驗收 |
| RELEASE | x64 FULL、x86 TSF／混打／Rust 檢查、兩架構候選 host、輸入互動與 mode harness、WinUI 驗收和安裝包校驗 | 正式發布前 |

影響規則集中在 `scripts/test-plan.psm1`。profile／bridge 對應 TSF、設定更新與 WinUI；shared 輸入政策對應 TSF 與引擎互動；原生候選／模式元件對應 TSF、UI 與 mode。其餘 Windows／shared 共用邏輯、核心、模型、依賴及不明影響會擴大至 FULL；專用 harness 選相應案例。無關 WinUI 的 AFFECTED 可略過 UI publish。CI 的固定集合不使用這些選測規則。

每次 JSON 都列出 CTest 選中／未選項目的理由，以及英文鍵 30 組、提交 77 組（每架構）的模式、鍵盤、按鍵、預期結果與實際結果。`results[].attempts` 和子案例的 `attempts` 保留每次執行，重試後通過會標示 `flaky`，首次失敗 JUnit／日誌不被覆寫。CTest fixture 會明列在計畫中；選中卻未執行的項目不能讓整輪通過。CI-safe 略過的互動項目有獨立原因，不列為 PASS。

計時事件含 stage ID、parent ID、原始包含子階段耗時及 `exclusive_duration_ms`。後者扣除子階段區間聯集，避免巢狀重複加總；平行工作的合計仍可能大於 wall-clock。探測程序使用有期限的等待並只清理自己啟動的程序；CTest 另設每項 timeout。

## 舊流程覆蓋對照

FULL 保留舊 `verify.ps1` 的 x64 驗收內容，並包含原 CI 的 shared／windows Rust 單元測試。`verify.ps1` 留作相容及 benchmark 基準入口；CTests 不把它註冊成單一測試。

| 舊流程／測試 | CTest 案例 | 保留的驗證與條件 |
| --- | --- | --- |
| `verify-upstream-data.ps1` | `fast.upstream-data`, `full.dist-data` | 上游詞庫、詞包、模型清冊及建置後檔案完整性 |
| 測試框架本身 | `fast.infrastructure` | 影響分類、fixture、完整子案例、重試歷程、空快取、必要輸入、探測 timeout 與計時階層 |
| `prepare-core.ps1`、Rust `shared/windows --lib` | `fast.prepare-core`, `fast.rust.*` | 與 CI 相同的 core overlay 與 Rust 單元測試 |
| `test-core.ps1` | `full.core-suite` | 上游 library 與所有整合測試 binaries；原本依實體記憶體排程與 `engine_lm`／`engine_replay` 序列延遲斷言不變 |
| ignored `mixed_regression` | `full.mixed-regression.x64` | 使用建置模型與資料執行混打、數字回歸 |
| `tests/tsf_smoke.cpp` | standard、cold 各一項 | 真實 TSF context、標準 profile 與無模型冷啟動；不註冊輸入法 |
| `verify-add-word.ps1` | `full.word.add.x64` | 加詞浮層、主題與模型選擇；獨立 LOCALAPPDATA 和核心端點 |
| `verify-english-keys.ps1` | 19 個 `full.english.<scenario>.x64` 項目 | 每個 scenario 保留原本混打模式、鍵盤配置、組字／提交斷言 |
| `verify-submission.ps1` | 21 個 mode 項目及 `input-digits` 項目 | 每項提交、取消、焦點、忙碌與退休流程斷言；數字輸入保留所有原始組合 |
| `verify-private-desktop.ps1` | 18 個 TSF 相容及 15 個浮層生命週期項目 | 每案不同 profile、TEMP 與 Private Desktop；不切到該桌面、不操作使用者桌面 |
| `verify-interaction.ps1`、`verify-mode.ps1` | `release.interaction.x64`、`release.mode.x64`、`release.mode.x86` | 隔離 profile 的引擎互動回歸、兩架構離屏模式列／圖示 smoke；不送出桌面輸入 |
| Release profiles／x86 | `release.tsf-smoke.shanjie.x64`, `release.*.x86` | 額外 Shanjie profile TSF smoke，以及 x86 Rust／混打與上述 TSF、提交、私人桌面矩陣 |
| `verify-ui.ps1`、WinUI compile、`package.ps1` | `release.ui.*`, `release.package` | WinUI 工作流程、候選 host x64／x86 與安裝包校驗；不安裝到實機 |

CTest 清冊以 `ctest --preset windows-x64 --show-only` 與 x86 preset 為準。WinUI publish 屬於產品建置，另有計時，不重複註冊成測試。混打的快取身分斷言在獨立 Cargo 程序執行，其餘輸入回歸仍可平行，保留全部斷言。私人桌面每案明確啟動自己的核心，使用相同隔離 profile 探測就緒並在 finally 回收該 Process，不掃描或停止使用者的核心。發布驗收另含 x86、WinUI、離屏互動與單檔安裝包校驗；離屏或私人桌面不等於實際安裝、前景 App、高對比、多螢幕或讀屏驗收。

### 專用驗收（不屬於舊 `verify.ps1` FULL 清冊）

以下既有腳本原本就不在完整 `verify.ps1` 呼叫樹中，保留作為需要專用 fixture 或歷史版本輸入的額外檢查，不能把它們說成 FULL 已執行。變更影響尚無法精確對應到單一 CTest 案例時，`scripts/test.ps1` 會保守擴大到 FULL；這些額外腳本仍需依下表明確啟動：

| 入口 | 驗證內容與條件 | 隔離／限制 |
| --- | --- | --- |
| `scripts/verify-input-features.ps1` | 約 86 組 Caps Lock、混打、符號、聲調、空白提交輸入；支援按 case index 縮小 | 私有 profile 與暫存 payload；預設同時編譯 x64、x86 harness |
| `scripts/verify-profile-refresh.ps1` | 延遲 profile 更新、排隊輸入、焦點切換與 fault injection；架構、fault、scenario 可選 | 每項獨立 profile；會啟動隔離 core |
| `scripts/verify-core-recovery.ps1` | pipe、fault、busy、restart、rerank、outstanding、capacity 等可靠性檢查 | 每項獨立 profile；會以測試 profile 對 core 發送停止請求 |
| `scripts/verify-dictionary-store.ps1` | x64/x86 ABI 同時寫入、損壞檔修復與 SHA-256 名稱驗證 | 只寫入新的 build fixture，不讀寫使用者詞典 |
| `scripts/verify-profile-coexistence.ps1` | 新舊 DLL schema 共存與舊版讀取期間的 profile 不回寫 | 必須明確提供舊版 package 路徑；證據只能寫入工作區 `build/` |
| `scripts/verify-update.ps1` | TSF Caps Lock／焦點欄位、首次輸入延遲、標點組字與 resident core 檢查 | 建立兩架構 payload 與獨立 profile；不安裝或註冊輸入法 |
| `scripts/verify-no-admin.ps1` | 無系統管理員的組合驗收 wrapper，包含 x64/x86、UI 與 package | 不執行 UAC 安裝；主要步驟由 RELEASE 覆蓋，不在 CTest 中再次巢狀呼叫整套流程 |

這些路徑沒有被刪除或併入 FULL 測試數；`tests/caps_lock.cpp`、`tests/dictionary_store.cpp`、`tests/profile_refresh.cpp`、`tests/profile_coexistence.cpp`、`tests/engine_perf.cpp` 仍是其對應的獨立 harness。`verify-profile-coexistence` 需要外部舊版 package，`engine_perf.cpp` 是明確的效能量測 harness，兩者不適合每次完整回歸自動執行。

## 重複工作與隔離

- C++ harness 由 CMake 統一用 MSVC／Windows SDK 建置。CMake object 與 per-architecture 輸出支援增量編譯；舊 PowerShell 腳本接受 `-HarnessPath` 供新入口重用建好的 exe。直接呼叫舊腳本仍保留原編譯 fallback。
- Cargo target 保留既有增量 fingerprint；上游 core 測試 source tree 依 upstream pin、輸入檔與 Rust 工具鏈驗證快取。
- core overlay、英文清單、upstream 資料 export、模型轉換與 WinUI publish 以來源、參數、架構、工具版本／upstream pin 及輸出內容 hash 驗證快取。失效時重新生成；只存在檔案不會命中快取。
- 模型測試分配到兩個 CTest resource slots，最多同時載入兩組模型；core suite 與混打回歸會鎖住兩個 slots，避免與其他模型載入競爭。私人桌面案例另用共用鎖逐案執行，確保各案例的桌面與核心清理不互相干擾。Core suite 仍使用原本記憶體限制排程。不同 profile 的核心不共用生命週期；shared/windows Cargo 共用的目標目錄也有獨立鎖。
- 測試 payload 僅建立在 `build/cmake/`，其中 `data`、`tools` 是指向 `dist/` 的唯讀 junction；測試不註冊輸入法、不碰正式使用者資料、不停止使用者核心。
- 加詞視窗與私人桌面案例在不切換的測試桌面執行，避免目前桌面的前景視窗變動關閉測試 popup；這不代表已驗證使用者正在操作的前景 App。

## 執行環境

| 環境 | 可驗證內容 |
| --- | --- |
| Codex Cloud Linux | 原始碼與報告設計、JSON/YAML 等靜態檢查；沒有 MSVC、Windows SDK、WinUI、TSF runtime 時不得宣稱 Windows 測試通過 |
| GitHub Actions Windows | PR 與手動 ci 固定跑 CI 基線；不建置 WinUI 或產品。既有 Release job 才跑 RELEASE -CiSafe，略過的互動項目列為未執行，須本機補驗 |
| Windows 本機 | FAST、AFFECTED、FULL、RELEASE 全部實際 Windows 測試；WinUI 驗收與候選 host 可使用本機 Windows／SDK |

## 本機候選封裝與正式發布

```powershell
pwsh -File scripts/package.ps1 -UseExistingBuild   # 只建立測試用候選安裝包
pwsh -File scripts/test.ps1 -Level RELEASE          # 建置、完整驗收與候選包校驗
pwsh -File scripts/release.ps1                      # 取得或重用有效驗收，再提升正式產物
```

候選包位於 `build/release-candidates/<id>/`。`package.ps1` 不呼叫 test.ps1；RELEASE 的封裝項目只產生並校驗候選包，因此沒有循環依賴。FAST／AFFECTED／FULL 不要求正式發布紀錄。

本機正式入口先要求工作樹乾淨、Cargo／lock／WinUI／channel 四處版本一致。驗收紀錄必須是相同 commit、版本及兩架構的完整 RELEASE：逐項對照目前 CTest 清冊與原始 JUnit、JSON 結果和全部英文／提交子案例，不能只有 `status=passed`。CI-safe、跳過、未完成或缺少架構都會被拒絕。

紀錄保存驗收前後原始碼身分，以及所有出貨建置檔、候選安裝包、候選 metadata 和 JUnit 的 SHA-256。提升前重新核對，驗收後更動原始碼、版本或產物就失效。有效紀錄可重用，不重跑整套。`package.ps1 -Promote -ReportPath <report.json>` 也會執行同一防護，不能直接繞過；它把已驗證的候選包複製至 `dist/Shanjie-Windows-Setup.exe`，確認複製後雜湊，再寫 `dist/release.json` 的本機完整驗收標記。

GitHub Actions 的既有 Release job 仍能跑較長的 RELEASE -CiSafe，再匯出 CI artifact；標記明確表示它不是本機完整驗收。日常 PR 的固定輕量 CI 不變。本機正式入口不 push、建立 tag 或呼叫 GitHub Release，遠端動作另需授權。

## 新舊耗時比較

同一台 Windows、同一 commit、相同完整覆蓋可在完成 commit 後執行：

```powershell
pwsh -File scripts/benchmark-verification.ps1 -Commit HEAD
```

Benchmark 建立三個 detached worktree，各跑冷、暖一次：改造前的原始 `verify.ps1`、目前提交的相容 `verify.ps1`、目前提交的 CTest FULL。預設使用與目前提交共同、尚未採用 CTest 的 origin/main 祖先；無此基準時才用首次加入 CMakeLists.txt 的前一版，也可用 `-BaselineCommit <ref>` 指定。三份產品原始碼、詞庫、模型與 pin 必須一致；不同就拒絕宣稱驗收流程加速。工作副本與報告預設放在 `build/benchmark/`，空間不足時可用 `-OutputRoot <目錄>` 或 `SHANJIE_BENCHMARK_ROOT` 指定其他磁碟。

比較前，從實際 Cargo／核心日誌、TSF 輸出、英文 profile、提交與私人桌面結果產生案例識別碼及組態，六次必須完全一致。新增的框架自檢另行說明，不冒充原本產品覆蓋。原始 baseline 沒有 JUnit 或分階段計時，僅保留原生證據與外部量到的 wall-clock；不捏造缺少的資料。比較、JSON、JUnit、原始日誌及案例證據保存在 `build/benchmark/<run>/`，即使失敗也寫 `comparison.json` 並標示不可比較及未執行項目。

「改造前 vs CTest」與「同提交相容入口 vs CTest」分開計算，不能把後者宣稱為整次改造效益。冷快取指全新專案輸出；Cargo registry／NuGet 下載快取與 OS 檔案快取可能已暖。為隔離舊版的全域快取身分斷言，六次都設定 `RUST_TEST_THREADS=1`；不同核心 test binary 仍保留原本的記憶體限制平行排程。報告列秒數、分鐘、冷暖差異與包含／自身分類耗時；未執行或覆蓋不等價時不宣稱改善。GitHub Runner 的 CI 耗時另看實際 Actions 紀錄，不能用本機暖快取時間替代。
