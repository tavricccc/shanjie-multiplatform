# 模型清單自動維護

0.13.0 Windows 新增 format:2 reader、配套驗證與備註確認。內建模型仍是 `MODEL-SOURCES.json` 所記錄的 model-v6＋classes-v5；上游 pin 維持 `UPSTREAM.json`，資料更新不替換程式碼。

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
