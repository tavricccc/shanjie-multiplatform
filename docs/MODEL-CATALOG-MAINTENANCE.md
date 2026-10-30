# 模型清單自動維護

0.13.0 Windows 新增 format:2 reader、配套驗證與備註確認。內建模型是 `MODEL-SOURCES.json` 所記錄、與上游 App 相同的整組模型（id `builtin`）；上游 pin 維持 `UPSTREAM.json`，資料更新不替換程式碼。

## 上線與日常操作

工作流程：`.github/workflows/model-catalog-maintenance.yml`。每日 UTC 01:23（臺灣 09:23）執行；亦可在 Actions 選 **Run workflow**：

- `verify`：完整探索、原生探測、評測、組裝與上傳證據，不更新線上清單。
- `publish`：先驗證，再以有限寫入權限發布資料。
- Repository variable `MODEL_CATALOG_AUTOPUBLISH=true`：允許排程及 main 的相關變更在成功驗證後自動發布；移除／設為 false 即停用自動發布，手動 verify 仍可用。

初次以功能分支乾跑成功後才啟用自動發布。GitHub 公開 repo 長期無活動（60 天）可能停用排程；若缺少近期成功紀錄，請在 Actions 重新啟用並手動 verify。排程可能延遲。

`model-catalog` 為初始無程式碼歷史的獨立資料分支，只寫 `MODEL-CATALOG-V2.json`、`MODEL-CATALOG-STATE.json`、`reports/<sha256>.json`。端點：

`https://raw.githubusercontent.com/tavricccc/shanjie-multiplatform/model-catalog/MODEL-CATALOG-V2.json`

main 的 `MODEL-CATALOG.json` 保留給 0.12.2 等舊版。新 receipt 使用舊 reader 不接受的識別，避免舊版忽略新備註／輔助檔；舊版合法配套在新版仍可使用。

## 驗證契約

流程是 `discover → probe → advisory evaluation → assemble → publish`，實作在 `scripts/model_catalog/catalog.py`、`probe.ps1` 與 `windows/src/bin/shanjie-model-check.rs`。

1. 完整分頁查詢 `Nanako0129/shanjie` 公開 Releases；固定來源與合法附件名稱，核對 GitHub 記錄及實體內容的大小／SHA-256。不執行上游附件中的指令。
2. 依實際 magic 與內嵌雜湊配對。目前能力含 SJLM0001、SJCL0001、SJKN0002、SJT30001、SJTL0001–0004、SJWM0002 映射及無詞類模型。口語模型作獨立主模型，保留來源／授權附件，不混入一般模型。
3. 同一 source commit 的 x64、x86 原生核心在隔離資料、停用學習及正式核心連線的子程序驗證完整配套、映射等價、載入、輸入與一次提交。語意配套另確認實際重排。架構證據遺失／重複時停止組裝；只對通過的架構宣稱可用。
4. 同一 runner、核心配置及公開資料比較基準與候選。資料為 pinned upstream `eval/bench/suite-v1/dev302.txt` 前 20 句，收錄於 `evaluation.txt`；記錄資料 SHA-256、配置、分母與雙方分數。聊天／正式模式分開，不挪用歷史 302 句或 macOS 成績；小集合只提供有限參考。
5. 退步、未評測、未比較效能、預覽版與其他差異只形成備註。新格式／能力未支援、配對錯誤、損壞或載入失敗不能宣稱可用。網路與測試逾時為尚未確認，保留舊清單與錯誤證據。
6. 發布前重查官方附件 ID、名稱、大小、digest 與 URL。僅更新允許資料檔，採非強制、以舊 head 為 parent 的原子更新；競態重新讀取合併，權限不足則失敗，不改 main。只有時間改變時不建立空提交。

## 檢視結果與故障處理

Actions 的 `model-catalog-publication` artifact 保留 30 天，含清單、狀態、每組每架構結果與發布計畫。長期證據隨資料分支保存，摘要列出成功時間、配套數及 revision。

- 探索／runner 失敗：讀失敗步驟與 pending reason，修復後重跑，不刪除使用者模型。
- 409／422：最多重新讀取 head 並重試三次；仍失敗則重跑 workflow。403 不當成競態重試。
- 上游同標籤換檔：停止發布，重新探索／驗證新雜湊，不原地替換舊配套內容。
- 新格式／附件：查看 unsupported，先擴充原生載入與能力契約並補測，不僅改清單繞過檢查。
- 用戶端離線：用成功快取與本機 receipt 備註；故障不自動切換使用中的模型。

本機契約測試：`python -m unittest discover -s scripts/model_catalog -p "test_*.py" -v`。`probe.ps1` 需要 Windows、兩架構 MSVC 與 Rust；原生檢查使用公開資料，不讀個人字典或學習檔。

## Actions 執行環境

0.13.1 將模型與 release 工作流程的官方 Actions 固定為 Node.js 24 版本：checkout 7.0.1、cache 6.1.0、setup-python 7.1.0、setup-dotnet 6.0.0、upload-artifact 7.0.2、download-artifact 8.0.2，均使用完整 commit SHA。版本依各官方 repository 的 release、action.yml 與 README 核對；例如 [checkout 的 runner／憑證變更](https://github.com/actions/checkout/tree/v7.0.1)、[artifact 的 archive 契約](https://github.com/actions/upload-artifact/tree/v7.0.2)。

runner 最低 2.327.1；目前使用 GitHub 託管 runner。未使用 Docker action 的認證轉送、fork PR 特權入口或新的非 archive 模式；checkout 保持 `persist-credentials: false`。模型 artifact 原目錄、最小權限、verify／publish 分流與非強制原子資料分支更新不變。

release 的手動執行預設 `verify`，只建置測試並留下 artifact。`publish` 只建立尚不存在的版本；既有 Release 不編輯、不上傳覆寫。新版本沿用 RELEASE-CHANNEL.json 的 pre-release／latest 政策；0.13.1 不取代 0.12.2 為建議正式版。
