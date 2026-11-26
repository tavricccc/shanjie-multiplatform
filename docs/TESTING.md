# Windows 驗收流程

`full.core.learning-status.x64`、`full.core.privacy.x64` 及對應的 x86 RELEASE 案例以隔離檔案驗證 local／pipe 學習失敗、恢復、明確關閉及隱私暫停。`private.tsf-compat` 的 `privacy-*` 用啟用前文的普通欄位做正向控制，確認前文查詢發生於第一次組字寫入前。同一階段在關閉前文、private、password 與 PIN 案例須為零讀取，隱私案例另驗證零學習寫入。組字後 TSF 自身的讀取次數另行記錄。`modifiers-*` 使用 `SetKeyboardState` 加真實 TSF callback；`range-refresh` 記錄未變文字重寫及最終組字結果。均不代表前景 App 或實體鍵盤已驗收。

`measure-tinylm-compare.ps1 -Comparison FixedCoreModelSwap` 只替換模型；`ShippingConfigurations` 另需 `-OldRoot`、`-OldChecker`、`-OldData`，使用舊版各自核心與完整配套。報告保留原始輪次、來源／執行檔與配套雜湊、實際融合及三檔參數。父程序先驗證 hash，再啟動量測程序；首次開啟不是 OS 冷快取，同程序再次開啟才代表資源快取。核心按鍵／rerank 與 `measure-engine.ps1` 的 IPC 按鍵分開解讀。 正式長上下文量測若超過每個子程序預設 300 秒，可用 `-ArmTimeoutSeconds` 明訂較長的期限，保留樣本數與配對順序。

`-Comparison DecodeWarmup` 在相同配套與隔離 profile 下交替啟動資源預載／解碼預熱的 resident host，記錄啟動到 ready 後首個成功 status（含輪詢／IPC）、首鍵、首音節尾端、後續延遲及 host 自身記憶體。實驗入口只接受測試身分；普通啟動維持資源預載。host 檢查 reset 後沒有組字、候選、前文、學習記錄／前綴及 rerank job；metadata／hash 程序的記憶體不算入 host 峰值。

Windows 語意校正排程實驗使用預設關閉的 `rerank-timing` Cargo feature；僅在 `SHANJIE_TEST_PROFILE=1` 下允許安全白名單切換專用 `Shanjie rerank` 執行緒優先序並輸出純數值 QPC 時間線，不新增一般使用者設定，也不改動既定 CI／FAST／AFFECTED／FULL／RELEASE 選測規則。

歷史新舊模型比較只在模型／配套／相關算法／比較工具變更或明確要求時安排，選測規則只提示條件式審查，不自動綁到每次 RELEASE。DecodeWarmup 為啟動政策決策實驗，得出結論後不例行重跑；單純排程修改不要求重做模型 benchmark。IPC 量測仍依既有按鍵／核心請求變更規則。

Windows 測試由 CMake／CTest 統一列出、選擇與排程；Rust 仍由 Cargo 編譯和執行。Visual Studio 2026 的 x64、x86 Preset 使用不同 build 目錄與產物，不能共用架構相異的 harness。

## 常用指令

```powershell
pwsh -File scripts/test.ps1 -Level FAST
pwsh -File scripts/test.ps1 -Level AFFECTED -ChangeScope Worktree
pwsh -File scripts/test.ps1 -Level FULL
pwsh -File scripts/test.ps1 -Level RELEASE
```

`FAST` 與 `AFFECTED` 讀取 `origin/main` 到目前 HEAD 的差異，也包含 staged、unstaged 和未追蹤檔案。可以用 `-BaseRef <ref>` 指定比較基準；分支已含廣泛變更時，`-BaseRef HEAD` 可只檢查目前工作樹，正式整合仍須 FULL。只有明確的文件檔可自動略過；程式碼檔不靠行首正則式判斷純註解。FAST 保持快速基線，AFFECTED 才依影響加選或擴大至 FULL。明確指定 FULL／RELEASE 時，一律執行完整集合，不套用文件略過規則。

比較範圍須明確：`-ChangeScope Branch`（預設）使用 `-BaseRef ...HEAD` 的 merge-base 差異，保留上游 `verify-changed` 累積比較；`Commit` 只取 `HEAD^..HEAD`；`Worktree` 不取已提交差異。三者都加上 staged／unstaged／untracked 檔案，報告記錄 `change_scope`／`change_range`，PlanOnly 顯示來源。`-BaseRef` 只用於 Branch，原本 `-BaseRef HEAD` 的用法保留。窄範圍不會自動認列較早的分支提交；找不到基準或無法比較仍保守 FULL。

```powershell
pwsh -File scripts/test.ps1 -Level AFFECTED -ChangeScope Worktree -PlanOnly # 日常未提交修改
pwsh -File scripts/test.ps1 -Level AFFECTED -ChangeScope Commit -PlanOnly   # 最後一筆提交＋未提交修改
pwsh -File scripts/verify-changed.ps1 -Architecture x86 -ChangeScope Worktree -DryRun # x86 選測計畫
pwsh -File scripts/verify-changed.ps1 -Base origin/main -ChangeScope Branch -DryRun # 全分支整合
```

影響規則只維護一份。上游 `verify-changed.ps1 -Base <ref>` 轉交 AFFECTED，`-DryRun` 對應 `test.ps1 -PlanOnly`，不執行 CMake、建置或測試，報告狀態為 `planned`；它列出變更原因、要求的標籤與額外驗證提示，實際 CTest 清冊仍須 Windows configure。`verify-changed.ps1 -Architecture x86`（或 `test.ps1 -Level AFFECTED -Architecture x86`）先套用同一影響規則：純文件跳過全部建置、測試與封裝；有界變更保留共用 x64 FAST 基線並執行對應 x86 回歸，settings／core 等共用檢查仍在 x64 執行；未知影響或無法比較才擴大至雙架構 RELEASE。`selection.preset_labels` 記錄架構分配，`requested_level` 與 `level` 區分請求及後備層級；`-UseExistingBuild` 不繞過快取驗證。`verify-selection.ps1` 的純選測 API 回傳 CTest `Full`／`Labels`／`Skip`／`Reasons`／`AdditionalVerification`，不保留第二套 Steps／Suites 排程。移動檔案以原、新兩個路徑判斷，移入 docs 不會掩蓋原本的程式碼變更。

僅編譯、確認產物或建立開發候選包，不自動啟動 FULL／RELEASE；修改程式依必要範圍執行 FAST／AFFECTED，先以 PlanOnly 檢查是否擴大。正式發布仍使用有同提交完整收據防護的 `release.ps1`，開發候選包不等於正式發布完成。

`release.ui.models` 在獨立 profile 及離線模型 fixture 驗證原作者優先、讀取失敗改用鏡像、同名模型跨來源沿用與新版更新，也檢查模型說明卡片的標題／內文，以及兩種視窗尺寸下捲到底的備註與提示範圍；附件大小、雜湊與原生收據仍實際核對，不代表正式來源網路或實機安裝驗收。

CI 與 Release 的子模組初始化共用 `scripts/sync-upstream.ps1`，官方讀取失敗才改用鏡像，固定 commit 不變。

GitHub Actions 固定使用 `pwsh -File scripts/test.ps1 -Level CI -CiSafe`，不讀分支差異，即使修改 CMake、測試框架或 CI 也不擴大。它執行上游資料完整性、Windows core overlay、shared／windows Cargo 單元測試及測試框架自檢。跨 Run 僅快取 Cargo registry 與 `shared/target`、`windows/target` 的單元測試編譯產物；Cargo 仍驗證自身 fingerprint 並實際執行測試，不快取模型、完整產品或測試成功結果。CI 不安裝 .NET、不編譯 WinUI、不下載完整 Git 歷史，也不做產品 release build、完整核心套件、TSF／UI 或封裝。保留既有 `ci` workflow 與 `unit-tests` job。

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

影響規則集中在 `scripts/test-plan.psm1`。恢復上游已有的細粒度規則：platform／prepare-core 選 Rust、core、混打、英文／提交；英文改動仍含混打；加詞只選 word／popup；原生候選選 candidate-host／tsf-compat；WinUI 選 settings-ui／candidate-host。新增的標籤只拆分既有案例，不新增或刪除斷言。installer／runtime log 保持 Windows 單元測試，CI／選測／發布交易腳本由輕量 framework 自檢負責。

上游未界定影響的 shared／TSF／profile 共用邏輯、模型、資料、依賴與未知路徑仍保守 FULL；保留需要的 UI／interaction 附加集合，避免把原本的未知 fallback 改成漏測。CMake 重構若仍在 Branch 差異內也會合理擴大，不能悄悄忽略；日常用 Worktree 或 Commit 明確限定範圍。CI 固定集合不使用影響規則。

下列是目前 x64 清冊的 AFFECTED 計畫數，包含固定 FAST 基線與必要 fixture，不代表已執行，也不換算成速度提升。`tests/ctest_integration.ps1` 在已有組態時，使用實際 CTest inventory 與同一選取函式核對，未組態時明示該檢查未執行。

| 變更範例 | 上游自動選測 | 本版 CTest 計畫（含 fixture） |
| --- | --- | --- |
| `ui/` | Rust＋settings UI／native host | 16 項，settings-ui＋candidate-host，不額外跑 mode／TSF 全套 |
| 英文鍵 harness／mixed_regression | Rust＋混打＋英文 | 29 項，包含雙操作範本的上游混合詞與英文補全 |
| submission harness | Rust＋提交 | 32 項 |
| platform／prepare-core | Rust＋core＋混打＋英文／提交＋measure | 58 項。IPC 延遲另行量測 |
| 加詞／浮層 | Rust＋add-word＋popup lifecycle | 22 項，不把全部 TSF／WinUI 加進來 |
| 原生候選 | Rust＋native candidate-host＋TSF compatibility | 48 項 |
| `shared/src/lib.rs`／未知路徑 | 未知影響，完整 verify | FULL 118 項。shared input 另含善解 TSF／interaction，共 120 項 |
| 只有文件 | 不建置、不測試 | 0 項；規則文件更新只提示人工核對 |
| CI workflow／交易選測規則 | 對應 Rust／交易或規則自檢 | 固定 FAST 基線 5 項；相較原本只跑 Python／選測的情境會多基線檢查，不聲稱每條路徑都更少 |

`verify-changed` 原本會自動執行 platform 的 `measure` 步驟；現在 AFFECTED 清楚列為 `required-extra`，由開發者／Codex 依 Markdown 完成並保留結果，**不自動執行、不把未量測列為 PASS**。本版沒有新增跨報告的全自動去重／收據合併系統；既有成功證據須按上述身分核對，已完成的相同驗證不額外啟動舊 wrapper。

每次 JSON 都列出 CTest 選中／未選項目的理由，以及各架構英文鍵 37 組與提交流程的實際子案例。原鍵混打的 Shift 英文與 Caps Lock 後接注音各驗證正式拼寫關閉與開啟。WinUI 啟動三情境、停用控制項三種鍵盤各 initial／reopen，另有 9 個具參數的子案例。`results[].attempts` 和子案例的 `attempts` 保留每次執行，重試後通過會標示 `flaky`，首次失敗 JUnit／日誌不被覆寫。CTest fixture 會明列在計畫中；選中卻未執行的項目不能讓整輪通過。CI-safe 略過的互動項目有獨立原因，不列為 PASS。

`ctest --preset windows-x64 -R 'english[.]mixed-words' --output-on-failure` 用同一個英文鍵 harness 驗證 Windows／善解操作及左右錨點。x86 使用 `windows-x86` preset。案例用私人桌面和獨立學習檔，走 TSF、Named Pipe 與出貨資料，涵蓋空白／聲調切分、數字選字、英文補全與 `^en:` 提交。候選窗的 Tab 標籤與英文提示由 native candidate-host harness 驗證。

計時事件含 stage ID、parent ID、原始包含子階段耗時及 `exclusive_duration_ms`。後者扣除子階段區間聯集，避免巢狀重複加總；平行工作的合計仍可能大於 wall-clock。探測程序使用有期限的等待並只清理自己啟動的程序；CTest 另設每項 timeout。

## 驗證規則自檢

固定 CI 的 `fast.infrastructure` 會從 CMake 已登記的 case ID 與 `ctest-case.ps1` 的實際 script delegate，核對所有 `scripts/verify*`、`test*`、`measure*` PowerShell 入口。未直接由 CTest 管理者沿用 `test-plan.psm1` 的唯一例外表，說明 dispatcher、舊 wrapper、fixture helper、必要額外量測或條件式專用驗證。新增入口未分類、刪除 delegate 的 CTest 登記或留下過期例外都使框架自檢失敗；不新增另一份測試排程或額外 CI job。

在 Windows 已 configure 時，自檢另以實際 inventory／同一選測函式確認：按鍵與 profile 變更保留標準／善解 TSF、shared 單元及既有九宮格／全半形／反引號案例；profile 邏輯／範本還包含 WinUI。純範本變更不再只落入缺少善解／WinUI 的 FULL。這是已知驗證規則的回歸保護，不能證明任意函式分支都有測試；新行為仍需加入合適斷言。

`report.json` 的 `verification_summary` 區分 CI／FAST 基線、選定集合通過、計畫與失敗；必要額外量測、未選中的必要 CTest 及已知 binary 覆蓋待審會列為 `pending`／`follow-up-required`，不能被選定集合的 `status=passed` 混作全部驗收完成。其他條件式專用工具列在 `conditional_reviews`，依實際差集判斷，不自動全跑。manager 的 `after-update` 通知失敗／日誌不由 `cargo test --lib` 或 package 的 `verify-package` 模式涵蓋，修改該 binary 會提示補針對性回歸。這份摘要不取代既有 RELEASE 的清冊、快照與產物雜湊防護。

## 舊流程覆蓋對照

FULL 保留舊 `verify.ps1` 的 x64 驗收內容，並包含原 CI 的 shared／windows Rust 單元測試。`verify.ps1` 留作相容及 benchmark 基準入口；CTests 不把它註冊成單一測試。

| 舊流程／測試 | CTest 案例 | 保留的驗證與條件 |
| --- | --- | --- |
| `verify-upstream-data.ps1` | `fast.upstream-data`, `full.dist-data` | 上游詞庫、詞包、模型清冊及建置後檔案完整性 |
| 測試框架本身 | `fast.infrastructure` | 影響分類（含上游 19 情境）、fixture、完整子案例、重試歷程、空快取、必要輸入、探測 timeout 與計時階層；另含 Cargo／移動路徑、整合快取自檢與上游 13 個 Python 發布交易測試（FakeGitHub，無網路發布） |
| `prepare-core.ps1`、Rust `shared/windows --lib` | `fast.prepare-core`, `fast.rust.*` | 與 CI 相同的 core overlay 與 Rust 單元測試 |
| `test-core.ps1` | `full.core-suite` | 上游 library 與所有整合測試 binaries；優先以單執行緒序列跑延遲隔離組（`engine_lm`、`engine_replay`、`engine_newword`），再依歷史耗時與上次失敗動態排序並行執行其餘重型測試（含 `engine_cap`、`engine_share`、`engine_auto_english`、`engine_tone3` 等），並維持實體記憶體與可用 CPU 限制 |
| ignored `mixed_regression` | `full.mixed-regression.x64` | 使用建置模型與資料執行混打、數字回歸 |
| `tests/tsf_smoke.cpp` | standard、cold 各一項 | 真實 TSF context、標準 profile 與無模型冷啟動；不註冊輸入法 |
| `verify-add-word.ps1` | `full.word.add.x64` | 加詞浮層、主題與模型選擇；獨立 LOCALAPPDATA 和核心端點 |
| `verify-english-keys.ps1` | 20 個 `full.english.<scenario>.x64` 項目 | 每個 scenario 保留原本混打模式、鍵盤配置、組字／提交斷言；加入上游兩種模式的 digit-navigation |
| `verify-submission.ps1` | `full.submission.*`，含 `async-keys`、`rebuild-local`、`rebuild-pipe`、`segment`、`input-digits` | 保留提交、取消、焦點、忙碌、退休及原有數字組合；新增數字語境、第一聲、學習檔與詞典保留，並驗證延遲編輯鎖的輸入順序及組字詞段範圍 |
| `verify-private-desktop.ps1` | 40 個 TSF 相容及 15 個浮層生命週期項目 | 每案不同 profile、TEMP 與 Private Desktop；不切到該桌面、不操作使用者桌面 |
| `verify-interaction.ps1`、`verify-mode.ps1` | `release.interaction.x64`、`release.mode.x64`、`release.mode.x86` | 隔離 profile 的引擎互動回歸、兩架構離屏模式列／圖示 smoke；不送出桌面輸入 |
| Release profiles／x86 | `release.tsf-smoke.shanjie.x64`, `release.*.x86` | 額外 Shanjie profile TSF smoke，以及 x86 Rust／混打與上述 TSF、提交、私人桌面矩陣 |
| `verify-ui.ps1`、WinUI compile、`package.ps1` | `release.ui.*`, `release.package` | WinUI 工作流程、候選 host x64／x86 與安裝包校驗；不安裝到實機 |
| 上游 WinUI 啟動／停用控制項回歸 | `release.ui.startup.{clean,reopen,different-directory,glass-fallback}`、`release.ui.disabled.keyboard{1,3,2}` | 獨立啟動路徑、已有 profile、不同工作目錄、無法建立玻璃時的純色備援；標準／善解／倚天停用設定的初次與重開檢查，保留非標準鍵盤的鄰鍵列斷言 |

FULL 的 x64 清冊可用 `ctest --preset windows-x64 -L full --show-only` 查看；RELEASE 另選兩個 preset 的 release 案例。實際執行與通過的數量以該輪 JSON／JUnit 及參數子案例為準。WinUI publish 屬於產品建置，另有計時，不重複註冊成測試。混打的快取身分斷言在獨立 Cargo 程序執行，其餘輸入回歸仍可平行，保留全部斷言。私人桌面每案明確啟動自己的核心，使用相同隔離 profile 探測就緒並在 finally 回收該 Process，不掃描或停止使用者的核心。發布驗收另含 x86、WinUI、離屏互動與單檔安裝包校驗；離屏或私人桌面不等於實際安裝、前景 App、高對比、多螢幕或讀屏驗收。

私人桌面案例 `shift-retain-windows`、`shift-retain-shanjie`、`shift-finish-eten` 分別驗證行程內核心、Named Pipe 與倚天送出行為。案例涵蓋切換時丟棄未完成讀音、英數保留原大小寫、切回中文續打、英數 Enter 一次送出及送出後交回宿主。用 `ctest --preset windows-x64 -R 'shift-(retain|finish)' --output-on-failure` 選測，x86 改用 `windows-x86`。

`punctuation-mcbopomofo` 以標準注音和 Named Pipe 驗證麥注音排列的十個映射、Apple 表的其餘按鍵與一次提交。核心測試涵蓋兩種鍵規則、逐鍵開關及倚天注音鍵。`release.ui.verify` 檢查選項、按鍵標籤和操作預設匯出／匯入。

`ctest --preset windows-x64 -R 'submission[.]async-keys' --output-on-failure` 可單跑編輯鎖回歸，x86 使用 `windows-x86` preset。每架構涵蓋標準／倚天搭配 Windows／善解互動。案例記錄外層 HRESULT、`phrSession`、非同步回呼次數、宿主收到的 Tab、候選選取與文件結果，並驗證前文查詢失敗後首個寫入回呼仍開始組字，失焦或停用後舊回呼不寫入。這些案例在私人桌面的 TSF 模擬宿主執行。

### 專用驗收與實際差集

#### G05：CLI／rowstats 特殊測試

`full.core.rowstats` 保留既有手動入口，分類為 `special;rowstats`；名稱的 `full` 前綴不代表屬於 FULL。用途是固定上游 raw 與 prepared Windows overlay 的 CLI 差分：兩側各 25 個 golden＋7 個 Judge2 斷言，chat／formal × 三資料集 × 兩種詞包配置共 12 組 byte 比對，以及輸出副本的一 byte 負向控制。英文按鍵、TSF、其他 OS／CPU 不由此案例證明。

上游核心、CLI、rowstats、模型／詞包或相關 overlay 改動時，先評估並明確指定才執行。CI、FAST、AFFECTED（含未知影響擴 FULL）、FULL、Beta、正式 RELEASE 與 release workflow 手動 verify 均不自動選入；預設 CTest presets 也排除 `special`。Release 收據依 full／release labels 核對，不要求這個特殊案例，其他必要清冊及快照防護不變。

已有對應核心、模型與資料後，手動執行：

```powershell
cmake --preset windows-x64
ctest --test-dir build/cmake/windows-x64 -C Release -R '^full[.]core[.]rowstats$' --output-on-failure
```

手動命令明確指定案例並使用 `--test-dir`，不套用 preset 的特殊測試排除；加 `--show-only` 只列計畫，不是 PASS。本輪固定點 `006c8f1d` 實跑 CTest **828.35 秒**：兩側編譯 15.18／17.25 秒、golden／Judge2 264.22／266.84 秒、差分 258.09 秒、負向控制 0.007 秒。這次來源／編譯未命中快取、未重用測試結果。各段為實測 wall-clock，COST 只作手動排程估值；沒有同條件 workflow 基準，不換算成 Actions 整體增量。原始紀錄在 ignored `build/reports`，屬於該固定點。更新 #130 後的清冊沿用手動政策。

核對基準為上游 `4794ab3` 的 AGENTS「驗證：做多少」、ARCHITECTURE「驗證」、PERFORMANCE「重跑」、原 `verify-selection.ps1`／`verify-changed.ps1`，以及各入口實際呼叫的 harness。上游明確規定按鍵路徑／核心請求須額外量測、UI 變更須 UI 驗收；其餘七支工具沒有逐檔必跑表，只有 shared／Windows 邏輯須執行「相關 verify」的通則。以下依實際依賴提示審查範圍，不臆造七支一律必跑的規則。

認列及補測順序：

1. 先核對仍有效的原始結果：來源／產物、斷言、架構、模式／鍵盤／按鍵／預期值、profile／模型、核心類型與權限／桌面環境。相同案例已成功就認列，不因舊 Markdown 命令再跑另一個 wrapper。
2. CTest 已登記或本次要求某標籤，不代表已執行。以本次實際 selection、JUnit、JSON、subcases 與嘗試結果確認；必要缺項用 CTest 的 `-R`／`-L` 補跑，不重新執行整支舊入口。需先準備該架構的產品及 CMake harness；`ui` 檢查還需 WinUI 輸出。CiSafe 略過不算成功。
3. 部分重疊時只補必要差集。支援 scenario／case／架構的工具須選最小範圍；沒有篩選器或證據未包含足夠配置時，先人工列出差異與不可避免的重複，不能自動啟動整套，也不能直接宣稱完整涵蓋。
4. 功能正確性不能取代延遲樣本；不同 resident／行程內核心、故障、舊 DLL、前景／離屏等條件也不能混為同一案例。一般成功結果去重不會繞過正式發布的同 commit 收據防護。

| 原入口／原始條件 | 實際 CTest 對應與差異 | 最小獨立驗證及必要前置 |
| --- | --- | --- |
| `verify.ps1`、`test-core.ps1`、`verify-regressions.ps1`；原完整驗收／相關邏輯 | 上表 `fast.*`、`full.*` 保留核心、混打、TSF／英文／提交／私人桌面；RELEASE 加 x86。已實際通過的相同案例不再跑舊入口 | 必要缺項用 CTest 選原生案例；原核心記憶體限制排程及延遲斷言保留 |
| `verify-ui.ps1`；`ui/` 與原生候選介面變更 | WinUI 改動選 x64 `settings-ui`＋`candidate-host`；原生候選依上游只選 `candidate-host`＋`tsf-compat`。RELEASE 加 x86 host。FULL 不含 WinUI 驗收 | 相同成功案例不再跑舊 UI 腳本；缺項用相應 CTest 標籤／`-R`，先建置相應產品／harness。native-only 不需要再驗整套 WinUI |
| `verify-input-features.ps1`；相關輸入行為需審查 | 使用 `tests/submission.cpp`。`input-retain` 與 CTest `input-digits` 共用 71／72 號組字／提交斷言，但 profile 的 prediction、escape、shift、punctuation 等不同；`input-space-commit` 的 70 號空白提交斷言也不能以一般 input-digits 取代。不能只按同按鍵文字認定等價 | 既有匹配證據可認列；只對必要且未驗證配置用 `-Architectures`、零起算 `-CaseIndices`，需要對應 DLL、data／tools、MSVC。`-ResidentCore` 與預設行程內核心分開判斷 |
| `verify-profile-refresh.ps1`；profile 通知／更新時序相關 | `tests/profile_refresh.cpp` 使用 resident core、prediction 故障注入及 normal／queued／latest／escape／focus／deadline。CTest submission refresh／refresh-twice 使用行程內核心，不能替代這個故障矩陣 | 只選受影響且無有效結果的 `-Architectures`、`-Scenarios`、`-Faults`；需要 DLL、dist core、MSVC 與獨立 profile |
| `verify-core-recovery.ps1`；broker／復原／請求交付／詞包切換效能 | `-Cases pack-switch` 分別比較本地及 Named Pipe 的獨立建置、背景共用建置，記錄建置與按鍵 p50／p95、記憶體、實際模型載入次數及退休的 pipe 工作階段；`rebuild-*` 驗證應用程式切換時機，無法取代此量測 | 先建相應版本及模型，在其他量測未執行時執行 `pwsh -File scripts/verify-core-recovery.ps1 -Architectures x64 -Cases pack-switch`。其他故障情境仍按 `-Cases` 選擇；僅停止自己的隔離核心 |
| `verify-dictionary-store.ps1`；詞典 ABI／發布儲存相關 | `tests/dictionary_store.cpp` 的 `sj_dictionary_save` 首次發布、截斷修復、SHA-256 檔名與 x64／x86 同時儲存；word-add／一般單元測試不等於此跨程序 ABI 目的 | 需兩架構 DLL 與 MSVC；新 build fixture，不碰私人詞典。無 case／架構篩選，需先說明剩餘必要內容與重複範圍，不自動重跑 |
| `verify-profile-coexistence.ps1`；確有 profile 格式／遷移相容需求 | `tests/profile_coexistence.cpp` 同時讀新舊 DLL；目前版本 UI 保存不能取代舊 DLL 共存 | 必須提供 `-OldPackage`、正確 `-OldSchema`；用 `-Architecture` 限縮。沒有舊套件就是未驗，不能算 PASS；證據留在工作區 |
| `verify-update.ps1`；Caps Lock／焦點／resident core 相關 | `tests/caps_lock.cpp` 與 `shanjie-core-check` 的 Caps Lock 矩陣、midword／fields／immediate／resident 模型；不是安裝升級測試。與 CTest English／smoke 只有部分行為相似，不能僅按名稱抵銷 | 先比對實際案例配置；需要兩架構 DLL、core-check、資料／模型、MSVC。只有 `-ExplicitCore` 等環境參數，沒有 scenario／架構篩選；若仍必要須明列無法拆分限制，不能自動跑整套 |
| `verify-no-admin.ps1`；一般權限環境的組合工具 | 組合 core／TSF 雙架構、interaction、mode、UI、單元測試與候選封裝，沒有新增獨立斷言；RELEASE 具備對應案例 | 同普通權限／來源／參數環境中已成功的對應案例直接認列，勿再跑整個 wrapper；不同權限／環境須另核對，只以 CTest 補必要缺項。不驗 UAC 安裝 |
| `measure-engine.ps1`；**改按鍵路徑或核心請求時必要** | `tests/engine_perf.cpp` 與 `tests/support/engine.cpp` 量 IPC／每鍵及核心資源；一般 TSF 功能與 upstream core 延遲斷言不等價，未納入 CTest | `pwsh -File scripts/measure-engine.ps1`；需 x64 release 靜態庫、dist core／data、MSVC；`-Learning` 是選用的 8000 筆合成資料條件。有效同條件測量可認列 |
| `verify-ui.ps1 -Benchmark`；PERFORMANCE 的舊重跑範例 | 上游與目前都只有該參數宣告，沒有額外使用分支；實際仍是已映射的 UI 案例，不是另一份效能量測 | 相同成功 UI 結果不用重跑；若要新視覺目的才依 scenario 加 `-Visual`，不同目的需另外記錄 |

### 計畫提示的能力與界線

`test-plan.psm1` 是唯一程式化規則來源，`additional_verification`／`AdditionalVerification` 只提供以下提示，不自動執行，也不加入 CTest PASS 數量：

- `ctest-planned`：相關驗證由所要求的 CTest 標籤負責，舊 wrapper 不再加跑。只是計畫；實際成功仍查結果。
- `ctest-needed`：已確認的必要 UI 條件未被目前標籤／CiSafe 涵蓋，只補缺少的 CTest 項目。
- `required-extra`：按鍵／核心請求的獨立延遲量測，CTest 沒有等價項目。
- `review`：依專用 harness 的真實依賴提示人工核對必要差集／舊套件／環境，不代表所有情境都必跑。
- `optional`：只修改量測工具時，可依需要取得新樣本；不是一般功能驗收。

`verify-input-features.ps1 -CaseIndices 0` 必須只選第零組，不可把數值 0 當成沒有篩選而跑整套；選測回歸會檢查此邊界，預設未提供篩選時仍保留完整案例。

路徑提示不是行級語意分析；其他檔案若實際改到按鍵／核心請求，仍適用 AGENTS 的正向規則。AGENTS、TESTING、ARCHITECTURE、PERFORMANCE 的變更會提示核對新增要求，**不會自動理解任意 Markdown 自然語言**。新增要求須同步唯一模組與選測回歸；只在 Markdown 提到某支腳本不會自動變成必跑命令。純文件仍略過建置／測試，CI 仍只執行固定五項。

## 排程順序與歷史耗時

CTest 與核心測試統一採用「上次失敗優先，其次耗時較長優先」原則，讓重型測試與延遲敏感案例盡早開始，縮短重大失敗的等待時間。排程不啟用 `--schedule-random`，不改變測試選取規則、層級定義或任何斷言。

- **冷啟動與多波次重型優先**：`CMakeLists.txt` 依實測耗時為各測試設定初始 `COST`。在 `scripts/test.ps1` 中，當選中集合同時包含多核心重型測試（`PROCESSORS >= 2`，例如 `full.core-suite`）與單核心測試時，會先將多核心重型測試連同其必要 `FIXTURES_SETUP` 排在第一波次（`heavy`），避免 CTest 平行排程時以大量短測試回填而將多核心測試餓死到最後；其餘單核心測試在第二波次（`remaining`）以 `-FS` 排除已完成的 setup fixture，不再重跑 fixture，並依成本降序執行。各波次各自保存獨立日誌與 JUnit XML，不互相覆寫，並寫入 `report.json` 的 `scheduling` 欄位。
- **CTest 歷史耗時與跨波次失敗記憶**：CTest 於各架構 build 目錄獨立維護 `Testing/Temporary/CTestCostData.txt`（`windows-x64` 與 `windows-x86` 分開記錄，不混用跨架構耗時）。首輪無歷史時依冷啟動 `COST` 降序；後續輪次自動套用各案例實測歷史耗時，並將上次失敗項目置頂。多波次執行時，前一波次通過前會保留屬於後續波次的上次失敗清單，避免前波次寫回 `CTestCostData.txt` 時清除後續波次的失敗優先權。
- **系統負載與資源限制**：`scripts/test.ps1` 提供選用 `-TestLoad <N>` 轉傳 CTest `--test-load`，可與成本降序排程並存。既有的 `FIXTURES_SETUP`／`FIXTURES_REQUIRED`、`RESOURCE_LOCK`（兩組模型 runtime slots、`core-build`、`cargo-windows-*`、`interactive-ui`）及 `-Jobs` 平行度限制完整保留。
- **核心測試動態排程**：`scripts/test-core.ps1` 先單執行緒序列執行延遲隔離組（`engine_lm`、`engine_replay`、`engine_newword`，冷啟動依 `engine_lm` → `engine_replay` → `engine_newword` 排序），再依動態成本降序並行執行其餘核心測試（冷啟動基準涵蓋 `engine_cap`、`engine_share`、`engine_learn`、`engine_auto_english`、`engine_pool_recur`、`engine_digits`、`engine_english`、`engine_pack`、`engine_fuzzy`、`engine_tone3` 等）。每次執行結束在 `finally` 更新 `build/test/core-cost-data.json`（預設路徑，可用 `-CostHistoryPath` 覆寫），以 `0.7 * 舊均值 + 0.3 * 新實測` 平滑更新並記錄 `failed_last`；上次失敗者下次置頂，歷史檔損毀時自動警告並回退至冷啟動基準，實體記憶體門檻 `-MemoryLimit` 與並行限制維持不變。

## 失敗快速停止與診斷

`scripts/test.ps1` 預設以 `-FailurePolicy fail-fast` 運作，搭配 FAST 基線前置閘門與重型優先排程，讓早期失敗迅速回報並避免無效資源消耗；如需在特殊排查時強制跑完已選項目，可顯式改傳 `-FailurePolicy continue`。

- **FAST 基線前置閘門**：當 `AFFECTED`、`FULL` 或 `RELEASE` 同時選取 `fast` 標籤項目與更廣的回歸集合時，會依序以 `fast` → `heavy`（多核心重型，如 `full.core-suite`）→ `remaining` 三波次執行。FAST 基線任一項失敗即立即停止，不啟動重型核心套件、其餘 TSF／UI 測試、x86 preset 或封裝；FAST 已完成的 `CoreOverlay` fixture 會透過 `-FS` 傳給後續波次，不重複執行。
- **平行案例安全收尾與核心快速停止**：`fail-fast` 會傳入 CTest `--stop-on-failure` 並設定 `SHANJIE_TEST_STOP_ON_FAILURE=1`。CTest 偵測到首個失敗後立即停止啟動尚未排程的測試，並等待已啟動的平行測試正常結束與釋放 `RESOURCE_LOCK`、子程序後才退出，避免強制終止留下殘留程序。`scripts/test-core.ps1` 的延遲隔離組（`engine_lm`、`engine_replay`、`engine_newword`）在任一失敗時即刻停止後續測試；並行組偵測到失敗後不再從佇列啟動新二進位檔，僅等待已啟動程序安全收尾並寫回 `core-cost-data.json`。
- **flaky 重試與首次失敗保留**：啟用 `-RetryFailures` 時，僅針對失敗項目執行一次 `--rerun-failed`，絕不自動把整套測試重跑一遍；首次失敗的日誌、JUnit XML、`results[].attempts[0].failure` 與 `flaky_tests` 均完整保留且不被覆寫。若重試成功救回且該波次因 `--stop-on-failure` 尚有未啟動案例，只針對未啟動項目補跑一次（獨立 `-resume` 日誌與 JUnit），已通過項目不重跑。
- **結構化報告、選擇性診斷與 RELEASE 防護**：`report.json` 的 `failure_handling` 記錄停止政策、是否提前停止、首次失敗摘要、已執行與未啟動案例數，未啟動案例於 `results` 明列原因而非 PASS。失敗後若需額外診斷特定案例，可顯式指定 `-DiagnosticTests <案例名稱...>`，診斷結果保存於獨立 `-diag` JUnit 與 `failure_handling.diagnostics`，不會改變失敗狀態。任何必要前置失敗都不會核發 `packaging-permit.json` 或授權正式發布提升。

## Runner 環境診斷與效能紀錄

`.github/workflows/ci.yml` 與 `.github/workflows/release.yml` 在既有 Windows job 的建置測試前後呼叫 `scripts/runner-diagnostics.ps1`（不新增額外 job、不安裝診斷工具、不更動既有快取鍵與固定 CI 五項）：

- **執行前環境摘要（`-Phase pre`）**：於 Job Log 印出並寫入 `build/reports/runner-diagnostics.json`，涵蓋 Runner OS／Image 標籤與版本／Windows Build／架構、CPU 廠牌／型號／邏輯與實體核心數／時脈／虛擬化與 Hypervisor 狀態、實體記憶體與系統提交量、系統與工作磁碟容量與媒體類型、開發工具版本（Visual Studio、MSVC、CMake、Ninja、Rustc、Cargo、PowerShell）、Git 與 CTest 設定，以及各快取命中狀態與還原大小（MB）。任何因權限或環境無法可靠取得的欄位一律標示 `unavailable`，不填入推測值。
- **執行期間效能彙整（`-Phase post`）**：`scripts/test.ps1` 執行期間透過低開銷 Windows kernel32 API（`GetSystemTimes`、`GlobalMemoryStatusEx`）取樣 CPU 平均／峰值與實體 RAM 峰值並寫入 `report.json` 的 `resource_usage`；測試結束（`if: always()`）由 `-Phase post` 合併整體 wall-clock、CTest 平行度、實際選取測試數、各階段建置與測試耗時與逐案耗時，隨既有 `build/reports/**` Actions Artifact 保存。
- **機密資訊防護**：診斷腳本採嚴格白名單存取環境欄位，不列舉或傾印一般環境變數，並對輸出中的憑證 URL、GitHub token、Bearer 標頭與常見機密鍵值執行遮蔽。
- **跨 Run 比較歸因界線**：JSON 內建 `comparison_dimensions` 與 `attribution_notes`，跨 Run 比對耗時必須同時檢視硬體規格、映像版本、工具鏈、快取命中與還原大小、平行度與實際測試數量，以及共享宿主的未知噪音因素，不可單憑 CPU 型號不同便將耗時差異完全歸因於 CPU。

## 重複工作與隔離

- C++ harness 由 CMake 統一用 MSVC／Windows SDK 建置。CMake object 與 per-architecture 輸出支援增量編譯；舊 PowerShell 腳本接受 `-HarnessPath` 供新入口重用建好的 exe。直接呼叫舊腳本的編譯 fallback 保留上游 #100 的 `Invoke-HarnessBuild` 快取，不再反覆編譯相同內容。
- Cargo target 保留既有增量 fingerprint；產品 build 明確指定工作樹的 `windows/target`，外部 `CARGO_TARGET_DIR` 不會讓複製 DLL／工具／staticlib 的路徑失配。上游 core 測試 source tree 依 upstream pin、輸入檔與 Rust 工具鏈驗證快取，失效時也只同步位元組有變的檔案，不破壞未修改來源的時間戳。
- AFFECTED 和直接呼叫 `test-core.ps1` 可沿用 #100 的整套核心通過收據，報告會列 `core_result_reused`。Beta 的 `RELEASE -CiSafe` 每次執行一般核心回歸。`engine_cap`、`engine_share`、`engine_lm`、`engine_replay` 和 `engine_newword` 在來源、編譯器、runner 版本、執行檔、參數、模型、測試資料及日誌雜湊全數吻合時重用通過收據，報告會列 `core_reused_binaries`。FULL、正式版 RELEASE、本機完整 RELEASE、直接 CTest 和 benchmark 實跑所有核心斷言。`test-core.ps1 -Rerun` 可手動強制實跑。
- Windows Rust 單元測試分別記錄 Cargo 編譯時間與測試執行時間。RELEASE 使用相同的最佳化 profile，可重用指紋相符的產品依賴。測試以 `--test-threads=1` 執行，避免 installer journal 與 progress 測試同時寫入程序全域的進度 sink。
- 每個架構的 36 個 `input-digits` 案例依三種混打設定各啟動一次 submission harness。每案重新建立 TSF document、context 及 text store，清除候選和組字，並沿用該組已載入的引擎。報告保留每案的預期文字、通過結果、耗時與 harness PID。
- core overlay、英文清單、upstream 資料 export、模型轉換與 WinUI publish 以來源、參數、架構、工具版本／upstream pin 及輸出內容 hash 驗證快取。失效時重新生成；只存在檔案不會命中快取。
- 模型測試分配到兩個 CTest resource slots，最多同時載入兩組模型；core suite 與混打回歸會鎖住兩個 slots，避免與其他模型載入競爭。私人桌面案例另用共用鎖逐案執行，確保各案例的桌面與核心清理不互相干擾。Core suite 仍使用原本記憶體限制排程。不同 profile 的核心不共用生命週期；shared/windows Cargo 共用的目標目錄也有獨立鎖。
- 測試 payload 僅建立在 `build/cmake/`，`data` 是指向 `dist/`（RELEASE 時指向本次快照）的唯讀 junction，`tools` 保留 #96 的私人執行檔副本，避免核心 IPC 使用正式工具路徑。相同工具內容不重複複製；舊版已知 tools junction 僅搬移連結本身後替換，不走訪或刪除目標。測試不註冊輸入法、不碰正式使用者資料、不停止使用者核心。
- 英文鍵、提交、加詞視窗與非視覺 TSF smoke 在不切換的私人測試桌面執行，隔離目前桌面的實際 Alt／Ctrl／Shift 與前景視窗變動；既有私人桌面案例也保持隔離。這不代表已驗證使用者正在操作的前景 App。
- WinUI 子程序建立前即設定隔離 profile 與行程內核心，避免 XAML 初始化時啟動已安裝的常駐核心；不列舉或停止工作副本外的核心。各階段結果保留在本次報告目錄，停用設定重開前封存舊 PASS 輸出。截圖依 #100 預設不產生；需要視覺審查時明確使用 `verify-ui.ps1 -Visual`，必要斷言照常執行。

## 執行環境

| 環境 | 可驗證內容 |
| --- | --- |
| Codex Cloud Linux | PowerShell 語法解析、選測規則、檔案／mock 框架自檢、Python 發布交易單元測試與 JSON/YAML 靜態檢查；不執行產品建置或新增 Linux 驗收流程，不能宣稱 MSVC、WinUI、TSF、x64／x86 或 RELEASE 通過 |
| GitHub Actions Windows | PR 與手動 ci 固定跑 CI 基線；不建置 WinUI 或產品。既有 Release job 才跑 RELEASE -CiSafe，略過的互動項目列為未執行，須本機補驗 |
| Windows 本機 | FAST、AFFECTED、FULL、RELEASE 全部實際 Windows 測試；WinUI 驗收與候選 host 可使用本機 Windows／SDK |

## 本機候選封裝與正式發布

```powershell
pwsh -File scripts/package.ps1 -UseExistingBuild   # 只建立測試用候選安裝包
pwsh -File scripts/test.ps1 -Level RELEASE          # 建置、完整驗收與候選包校驗
pwsh -File scripts/release.ps1                      # 取得或重用有效驗收，再提升正式產物
```

RELEASE 在建置前要求乾淨工作樹與一致版本；先提交預期來源，`-PlanOnly` 仍可查看未提交變更的後備計畫。完成雙架構產品建置後，將 DLL、UI、工具、資料、授權、封裝來源／packer 與兩架構 staticlib 複製至 `build/release-snapshots/<id>/`，以 commit、版本與 `snapshot.json` 的 SHA-256 綁定並設唯讀。複製前後核對來源與快照內容，拒絕混合建置或連結輸入。CMake harness 連結快照 staticlib；TSF 使用本次獨立 payload／工具副本及快照資料 junction；混打測試只在測試模組指定快照資料。WinUI 的可變 `installed.json` 留在私人 fixture，不寫入快照或使用者安裝。

x64、x86 的所有非封裝測試與參數子案例均通過後，才建立 `packaging-permit.json`；沿用正式收據的同一份 CTest／JUnit／JSON／子案例覆蓋檢查，只暫時排除 `release.package` 本身。接著單獨執行該 CTest 項目，從同一快照複製封裝內容、校驗、產生單檔候選安裝包。它有獨立的 `junit-windows-x64-package.xml`，不覆寫前一輪 x64 JUnit。直接用 CTest 執行封裝項目而沒有匹配快照與雙架構證據會失敗；正式驗收請使用 RELEASE 入口。

候選包位於 `build/release-candidates/<id>/`。`package.ps1` 不呼叫 test.ps1；RELEASE 的封裝項目只產生並校驗候選包，因此沒有循環依賴。FAST／AFFECTED／FULL 不要求正式發布紀錄；未知 x86 影響擴大至 RELEASE 時例外。獨立 `package.ps1` 仍可建立開發候選包，但沒有快照與雙架構封裝許可時不能用於正式提升。快照與封裝副本會增加本機磁碟需求，保留在 ignored `build/` 供收據重用，勿刪除仍需提升的候選快照。

本機正式入口先要求工作樹乾淨、Cargo／lock／WinUI／channel 四處版本一致。驗收紀錄必須是相同 commit、版本及兩架構的完整 RELEASE：逐項對照目前 CTest 清冊與原始 JUnit、JSON 結果和全部英文／提交子案例，不能只有 `status=passed`。CI-safe、跳過、未完成或缺少架構都會被拒絕。

RELEASE 收據 schema 4 保存驗收前後原始碼身分、固定快照、雙架構封裝許可、候選封裝內容／安裝包、候選 metadata 和 JUnit 的 SHA-256。提升前重新核對，任一架構失敗、缺少必要證據或更動來源、版本、快照／候選產物均失效；schema 3 等沒有快照的舊收據必須重新驗收。後續一般建置改寫 `dist/` 不會取代快照裡已測過的檔案。有效紀錄可重用，不重跑整套。`package.ps1 -Promote -ReportPath <report.json>` 也會執行同一防護，不能直接繞過；它把已驗證的候選包複製至 `dist/Shanjie-Windows-Setup.exe`，確認複製後雜湊，再寫 `dist/release.json` 的本機完整驗收標記。

GitHub Actions 的 Release job 跑 `RELEASE -CiSafe` 並匯出 CI artifact。x64 和 x86 的 Cargo 產物各有獨立的鍵與路徑，兩個架構共用 registry 與 host 套件。產物依 Rust、MSVC、Windows SDK、編譯設定及來源選擇快取，registry 可跨工具鏈版本還原。Cargo 指紋、core overlay 收據及模型雜湊會核對還原內容。NuGet 及內建模型轉換產物也有快取。x64 的發布建置與測試使用 Thin LTO 與 8 個 codegen units，x86 沿用 Fat LTO 與 1 個 codegen unit。Windows Rust 單元與混打回歸共用各自架構的測試執行檔，快取身分斷言仍在獨立程序執行。互動驗收另依本機 RELEASE 清冊執行。

遠端發布保留上游 #86 的 `scripts/release.py`：先檢查平台產物、來源 manifest 與 SHA-256；只續作同來源草稿，發布過的 Release 不覆寫，artifact 名稱包含 run attempt。這套遠端發布交易與本機完整驗收收據各自保留原有防護，不用 CI-safe 產物冒充本機 RELEASE。既有 Linux 發布 job 原樣保留，本次未新增 Linux 工作。

## 新舊耗時比較

同一台 Windows、同一 commit、相同完整覆蓋可在完成 commit 後執行：

```powershell
pwsh -File scripts/benchmark-verification.ps1 -Commit HEAD
```

要量測時以 `-BaselineCommit <commit>` 指定比較基準，會建立新的量測集合。

Benchmark 建立三個 detached worktree，各跑冷、暖一次：改造前的原始 `verify.ps1` 加原 CI 的 shared／windows 單元測試、目前提交的相容 `verify.ps1`、目前提交的 CTest FULL。舊 verify 原本不含兩套一般單元測試；執行器以外部命令補上，保持原腳本不變，並分開記錄純 verify 與補充 CI 的耗時，再比較完整相同清冊。預設使用與目前提交共同、尚未採用 CTest 的 origin/main 祖先；無此基準時才用首次加入 CMakeLists.txt 的前一版，也可用 `-BaselineCommit <ref>` 指定。三份產品原始碼、詞庫、模型與 pin 必須一致；不同就拒絕宣稱驗收流程加速。工作副本與報告預設放在 `build/benchmark/`，空間不足時可用 `-OutputRoot <目錄>` 或 `SHANJIE_BENCHMARK_ROOT` 指定其他磁碟。

比較前，從實際 Cargo／核心日誌、TSF 輸出、英文 profile、提交與私人桌面結果產生案例識別碼及組態，六次必須完全一致。新增的框架自檢另行說明，不冒充原本產品覆蓋。原始 baseline 沒有 JUnit 或分階段計時，僅保留原生證據與外部量到的 wall-clock；不捏造缺少的資料。比較、JSON、JUnit、原始日誌及案例證據保存在 `build/benchmark/<run>/`，即使失敗也寫 `comparison.json` 並標示不可比較及未執行項目。

量測中斷後，可在同一台電腦、相同工具鏈與桌面控制下加上 `-ResumeReport <中斷的 comparison.json>`，重用已完整通過且保有原始證據的冷暖配對。原始 baseline 必須同 commit；候選流程的測試與建置來源也必須完全相同，只容許 benchmark 執行器或文件修正。改到其他來源就重跑候選流程；只完成冷快取時會重跑整對，以維持暖快取條件。報告記錄原始 commit 與來源，最終仍要求六次實際案例及組態相同。程序提前結束或沒有完成報告的輪次記為失敗，不能因 exit code 為 0 當成通過。

若要保留先前已完成的相容入口量測，避免重跑，可另加 `-HistoricalLegacyReport <保有成功 legacy_cold／legacy_warm 的 comparison.json>`。產品來源必須相同；這兩輪明確標為原 commit 的歷史參考，停用它們與目前 CTest 的加速百分比，不能當作目前 commit 已驗收。主要效能比較仍是原始 baseline 對目前 CTest FULL，實際案例與組態仍須完全一致。這不授權重用不同 commit 的 RELEASE 紀錄。

「改造前 vs CTest」與「同提交相容入口 vs CTest」分開計算，不能把後者宣稱為整次改造效益。冷快取指全新專案輸出；Cargo registry／NuGet 下載快取與 OS 檔案快取可能已暖。六輪清除環境中的 `RUST_TEST_THREADS`，保留各入口正常的排程：Windows 單元測試單執行緒、舊混打入口自己的單執行緒設定、新入口的獨立快取身分程序，以及原有上游核心記憶體限制排程；不把整套上游測試一併串行化。報告列秒數、分鐘、冷暖差異與包含／自身分類耗時；未執行或覆蓋不等價時不宣稱改善。GitHub Runner 的 CI 耗時另看實際 Actions 紀錄，不能用本機暖快取時間替代。

為保留 #100 的快取又維持相同執行覆蓋，baseline／legacy 的暖快取輪只封存該新量測工作樹的 `build/test/passed` 核心結果收據，仍保留編譯、模型與資料快取；CTest FULL 本身強制重跑核心斷言。AFFECTED 的結果重用收益不能列入此 FULL 比較。已從 ResumeReport 匯入的完成輪次不執行，也不碰其證據。

舊版 harness 仍會讀取所在桌面的鍵盤狀態，量測應使用沒有實際鍵盤輸入干擾的 Windows 測試工作階段。桌面與其他控制條件須一致套用六輪，並隨量測環境記錄。
