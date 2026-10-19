# 延遲與資源

以下數字都來自背景 harness 的單次或少量樣本，只代表當時的模型、核心與測試程序，不是各 App 的日常體感保證。沒有量過真正登入、前景 App 的鍵盤到畫面延遲。

## 常駐核心（0.5.0 起）

所有前端透過本機管道共用 x64 核心程序 `ShanjieCore`，登入後在背景預載。多個會話、多個程序使用後，詞庫與模型的載入計數仍各為 1。

| 項目 | 樣本 |
| --- | --- |
| 核心閒置 Private／Working Set | 約 39.8／95.2 MiB（Working Set 含模型映射頁，可能被 Windows 換出） |
| 首次自動啟動核心到會話可用 | 570 ms |
| 核心已停止時立即按第一個字母（含冷啟動） | x64 約 840 ms，x86 約 930–1430 ms；之後七個字母都正確保留 |
| 預載後六鍵組字（Windows profile） | x64 3.4 ms，x86 2.6 ms |
| 重新啟用加首鍵與取消 | x64 7.4 ms，x86 6.0 ms |

改成共用核心的目的，是讓新 App 不再各自載入模型與詞庫，而不是讓解碼器變快；IPC 會增加每鍵的通訊成本。登入預載完成前輸入，第一鍵仍要等核心就緒。

## 等待上限（0.5.2 起）

| 階段 | 上限 |
| --- | --- |
| 首次建立會話（含載入回覆） | 30 秒 |
| 一般請求讀寫 | 750 ms |
| 重連、確認、重建 | 1.5 秒 |
| 學習鎖單次等待 | 250 ms |
| TSF 自動重試 | 5 秒，逾時保留組字並提示 |

這些是單一階段的上限，不是鍵盤到畫面的保證。I/O 使用 overlapped 與 CancelIoEx，等取消完成才釋放緩衝區。管道最多 64 個實例，額外連線約 1 秒後明確失敗。

## 前端行為

- TSF Activate 不同步建立核心或候選 renderer；Win 快捷鍵在取 context 前放行，閒置失焦不等待學習鎖。
- 按鍵路徑不查 INI，不監控檔案。設定由設定程式儲存時主動通知。
- 即時預測關閉時（Windows 預設），核心不建立預測索引。
- 候選窗不做背景 polling；中英提示只在顯示期間開一次性 1400 ms timer。

## 重跑

```powershell
pwsh -File scripts/package.ps1
pwsh -File scripts/verify-ui.ps1 -Benchmark
pwsh -File scripts/measure-engine.ps1
pwsh -File scripts/verify-mode.ps1
```

擷取放在畫面外，不取得前景焦點；輸出在 `build/native-review`、`build/mode-review`、`build/ui-review-final`。
