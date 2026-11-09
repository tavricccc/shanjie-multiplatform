# Windows 驗收流程

Windows 測試由 CMake／CTest 統一列出、選擇與排程；Rust 仍由 Cargo 編譯和執行。Visual Studio 2022 的 x64、x86 Preset 使用不同 build 目錄與產物，不能共用架構相異的 harness。

## 常用指令

```powershell
pwsh -File scripts/test.ps1 -Level FAST
pwsh -File scripts/test.ps1 -Level AFFECTED
pwsh -File scripts/test.ps1 -Level FULL
pwsh -File scripts/test.ps1 -Level RELEASE
```

`FAST` 與 `AFFECTED` 讀取 `origin/main` 到目前 HEAD 的差異，也包含 staged、unstaged 和未追蹤檔案。可以用 `-BaseRef <ref>` 指定比較基準。純文件或可辨識的整行註解變更時，入口直接產生「未執行」報告，不啟動 CMake。`FAST` 執行快速基線；`AFFECTED` 再依實際變更加選相關回歸。無法判定影響範圍的檔案會擴大至 FULL。

直接使用 CTest：

```powershell
cmake --preset windows-x64
cmake --build --preset windows-x64 --target shanjie_tsf_smoke
ctest --preset windows-x64 --show-only
ctest --preset windows-x64 -L english --output-on-failure
ctest --preset windows-x64 -R 'full\.private\.popup-lifecycle\.' --output-on-failure
ctest --preset windows-x64 --repeat until-pass:2 --output-on-failure
```

CTest 單獨執行時，先用 `scripts/build.ps1 -Architecture x64` 準備產品、模型與資料；`scripts/test.ps1` 會依測試需要自動完成這些步驟。每次入口驗收在 ignored 的 `build/reports/<run>/` 產生 `report.json`、CTest 輸出日誌和 JUnit XML。CTest 本身也能以 `-L` 標籤或 `-R` 名稱選測、`--repeat until-pass:2` 重跑失敗案例；預設不重跑已通過案例。

## 層級與變更影響

| 層級 | 選取內容 | 用途 |
| --- | --- | --- |
| FAST | 上游資料靜態清冊、Windows core overlay、`shared` 與 `windows` Rust `--lib` 測試 | 日常快速錯誤檢查；不明影響會保守擴大 |
| AFFECTED | FAST 加上變更對應的 Cargo／TSF／UI／Harness 回歸 | 功能完成後的相關驗收；無關 WinUI 的變更略過 UI publish |
| FULL | FULL 標籤的全部 x64 測試案例及產品建置 | PR、整合前完整 x64 驗收 |
| RELEASE | x64 FULL、x86 TSF／混打／Rust 檢查、兩架構候選 host、輸入互動與 mode harness、WinUI 驗收和安裝包校驗 | 正式發布前 |

影響規則集中在 `scripts/test.ps1`。`shared/` 會測 shared 與 Windows crate；`windows/src/tsf/` 會加跑 TSF 回歸；鍵盤、引擎、模型、profile、上游、prepare／build、依賴、CTest 架構及不明來源會擴大至 FULL；各專用 C++ harness 只選相應案例；`ui/` 會選 UI 相關回歸及產品建置中的 WinUI publish；mode harness 有獨立影響標籤，單獨變更時可略過 WinUI publish。Codex Cloud 的 Linux 環境不會把 Windows 測試標記為通過。

## 舊流程覆蓋對照

FULL 保留舊 `verify.ps1` 的 x64 驗收內容，並包含原 CI 的 shared／windows Rust 單元測試。`verify.ps1` 留作相容及 benchmark 基準入口；CTests 不把它註冊成單一測試。

| 舊流程／測試 | CTest 案例 | 保留的驗證與條件 |
| --- | --- | --- |
| `verify-upstream-data.ps1` | `fast.upstream-data`, `full.dist-data` | 上游詞庫、詞包、模型清冊及建置後檔案完整性 |
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

目前 CMake 宣告 85 個 x64 FULL CTest rows；這些 rows 可各自顯示結果，但有些 row 內會保留多個原始組態斷言。x86 Release 宣告 83 個架構專屬 rows；x64 Release 共 91 rows，包含額外 Shanjie profile smoke、離屏 harness、互動驗收與封裝檢查。WinUI publish 是產品建置的一部分，會獨立記錄在 `report.json` 的建置計時，不再重複註冊成另一個建置測試。CI-safe 模式會依影響規則編譯 WinUI 並執行離屏 harness，但略過需要本機互動 Windows session 的驗收。實際執行數量、未執行案例、失敗輸出與耗時以每次 CTest XML 和 `report.json` 為準。

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

## 執行環境

| 環境 | 可驗證內容 |
| --- | --- |
| Codex Cloud Linux | 原始碼與報告設計、JSON/YAML 等靜態檢查；沒有 MSVC、Windows SDK、WinUI、TSF runtime 時不得宣稱 Windows 測試通過 |
| GitHub Actions Windows | PR 跑 CMake／CTest AFFECTED，Release workflow 跑 RELEASE；UI 變更會編譯 UI，影響無法縮小的建置或測試框架變更會擴大至 FULL。`-CiSafe` 會把需要本機互動 UI session 的項目列為未執行，並記錄原因 |
| Windows 本機 | FAST、AFFECTED、FULL、RELEASE 全部實際 Windows 測試；WinUI 驗收與候選 host 可使用本機 Windows／SDK |

## 新舊耗時比較

同一台 Windows、同一 commit、相同完整覆蓋可在完成 commit 後執行：

```powershell
pwsh -File scripts/benchmark-verification.ps1 -Commit HEAD
```

Benchmark 建立兩個 detached worktree、初始化相同固定 submodule，依序執行舊 `verify.ps1` 與新 FULL 各一次 fresh workspace cache、一次 warm workspace cache，將比較、JUnit、日誌、CTest 分組列數、Rust core test binary 數與英文／提交／私人桌面實際案例數放在 `build/benchmark/<run>/`。共享的 Cargo registry／NuGet 套件快取可能已暖，因此報告會把 cold 定義為乾淨的專案 build/test outputs，並註明此限制。

計算方式：`saved = legacy wall − CTest FULL wall`，`speedup = saved / legacy wall × 100%`。分項報告會比較 C++ harness、Rust／WinUI 建置、模型／資料準備與核心測試階段。CTest 本身是排程與報告管理工具，單獨替換帶來的時間差不會冒稱為 runtime 最佳化；平行化效果沒有獨立 ablation 前也不會歸因為已證實收益。AFFECTED 的選測時間另行記錄，不能拿來和 FULL 的不同覆蓋範圍作公平比較。

尚未在 Windows 實測時，所有冷／暖改善均標記「尚未量測」。首次 Windows FULL 和 benchmark 完成後，效益以產生的機器報告為準。
