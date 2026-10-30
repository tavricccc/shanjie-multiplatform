# Windows 0.13.0 pre-release 驗證紀錄

日期：2026-10-27。基底為 `b1a6fd2f872e8874ca1ef77837a29ec9ca7e1e21`，上游 pin 沿用 `a639e8ada773327a1ddf06b0f96b4c32db67d6c2`。變更分批提交於混打、模型清單、原生配套、唯讀輔助程式與發布流程；上游追蹤來源沒有直接修改。0.13.0 為 pre-release，建議一般使用者先用 0.12.2。

## 已完成的自動驗證

| 範圍 | 結果與界線 |
| --- | --- |
| Windows Rust release | x64／x86 成功，未啟用 input-diagnostics；WinUI Release x64 成功，版本 0.13.0 |
| 混打與 TSF | x64 135/135、x86 135/135。涵蓋 r3／r4／r6／r7／r3467、多個不完整字母、原數字串、數字／字母接中文、有效注音、純中文及倚天 |
| 既有 TSF／浮層回歸 | x64／x86 各 18/18 相容性、15/15 生命週期；在未切換的私有桌面驗證 Shift、文件交接、adb 單次提交、取詞防護與取消／重入／重開 |
| Windows 單元測試 | 43 passed、3 ignored；其中兩項需明確指定 helper 的測試另行執行並通過，剩餘一項 HKCU registry fixture 未執行 |
| 共享模組 | 12/12，使用 Windows 鎖定相依測試 shanjie-shell；字典換行、合併保存、設定遷移及舊配套行為 |
| 加詞輔助程式 | 自建跨程序唯讀／可編輯 Edit、空選取、密碼排除、超長選取與 worker 逾時回收通過。只回收自己建立的子程序，不停止其他程序 |
| 浮層生命週期 | 自建來源視窗中，淺／深色下的滑鼠取消、Esc、重開與來源存活通過；面板讀音鍵不攔截其他快捷鍵的單元測試通過 |
| 模型資料與發布故障契約 | Python 18/18；退步與未評測可選、新命名、身份／雜湊／依賴／分母驗證、完整分頁、部分結果／空清單拒絕、來源換檔、非強制競態重試、403 停止及無空提交 |
| 新舊 reader 分流 | 隔離配套準備成功，語意重排實際執行 2 次；新 reader 接受 v2 receipt，同一隔離資料用已安裝 0.12.2 DLL 掃描時拒絕。未修改真正安裝目錄 |
| 模型備註 UI | 隱藏的隔離 WinUI：退步仍可下載、原生下載／保存備註、取消保留模型與字典草稿、確認只改草稿、不隱含儲存、離線保留備註，6 項通過 |
| 本機安裝包 | 原生 verify-package 通過；643 個列入雜湊的檔案加 manifest，zip／單檔安裝程式共 644 個檔案逐一核對成功；不含個人 profile、字典、學習、診斷包、TODO 或 .deleted_files。只讀驗證封裝，未啟動安裝 |
| GitHub 乾跑 | [run 40405152577](https://github.com/tavricccc/shanjie-multiplatform/actions/runs/40405152577) 成功，在 GitHub runner 以 source `eed64851fb8668719e2023b9b7fa09b9bb6f815e` 探索並驗證 16 組配套的 x64／x86 載入、輸入、提交及公開 20 句比較；乾跑不發布資料 |

測試資料在工作區 `build/todo-0.13.0-20261027/`，使用公開詞例與隔離 LOCALAPPDATA。設定驗證使用可驗證的公開附件快取及模擬網路故障；不等同所有網路環境已實測。模型評測只作備註，不當作通用準確率保證。

原生建置保留既有 COM export／測試 extern 宣告警告，WinUI 保留既有 x:Bind analyzer 警告。此環境的 NuGet vulnerability feed 無法連線，不宣稱完成線上弱點稽核。共享模組獨立工作區解析曾受網路與快取版本限制，改以 Windows 已鎖定相依執行後 12 項通過。

## GitHub 發布與線上驗證

- [0.13.0 pre-release](https://github.com/tavricccc/shanjie-multiplatform/releases/tag/v0.13.0) 已發布；[release run 40408380255](https://github.com/tavricccc/shanjie-multiplatform/actions/runs/40408380255) 成功，附單檔安裝程式、zip 與各自 SHA-256。建置 source 為 `b9b16222fe6f03fa24fc46df1c868d604b0e7866`；Linux 工作依本次 Windows 發布範圍略過。
- [model-catalog run 40408380240](https://github.com/tavricccc/shanjie-multiplatform/actions/runs/40408380240) 的 verify／publish 都成功，已啟用每日自動發布。公開端點實際下載後，通過 Python 與本版原生 reader 驗證：16 組、兩架構均可用、pending 0；其中 13 組具有至少一項固定小測試集低於基準的備註，仍全部保留可選。
- 首次資料 revision 為 `e2e0cc27c3c098d529cf7b70d7cc76108a2c469de1453cf5a2e153f88a32b221`。未來成功排程可更新資料與證據，不必重建本版安裝程式。
- 發布後再次查證最新正式版仍為 `v0.12.2`、0.13.0 為 prerelease、模型 workflow 為 active。main 的 README.md blob 與更新前相同：`e6587589d408799d7fa4c45c418151c2c6169540`。

## 尚未實際安裝驗收

本輪不執行安裝、UAC、輸入法註冊、登入工作修改或重啟 Explorer。以下仍需安裝 pre-release 後由使用者驗收，不能以自建視窗或 API 成功代替：

- Codex 回覆區、瀏覽器公開網頁、Win+R／AyuGram 等真正宿主的雙 Ctrl、取詞、浮層讀音鍵／加入／滑鼠取消／Esc 與原選取保存。
- 登入預載、重啟、關閉設定、輔助程式崩潰後恢復、快速切換使用者與多工作階段的單例。
- 多螢幕、不同 DPI、畫面邊緣、系統淺深色／高對比及長時間使用。
- 真正升級後個人字典／學習／模型沿用、舊 DLL 共存及管理員／AppContainer 宿主。延伸矩陣見 [待辦](../../TODO.md)。

Excel 儲存格反白取詞依使用者決定取消，不再列修復或驗收目標。過往 x86 check 286 的間歇條件未定位，不以本次通過宣稱根因已修。
