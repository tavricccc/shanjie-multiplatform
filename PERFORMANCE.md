# 延遲與資源量測

這些是背景 harness 實測，不是各種 App 的日常使用保證。只代表各次指定模型、核心與測試程序，不包含螢幕刷新或完整 App 的資源。

## 2026-10-09 Rust／model-v4 版本

原生候選在目前 120 DPI 環境，九項橫向、30 次隱藏重開；包括同步 Direct2D 繪製與 UpdateLayeredWindow。x86 冷啟動受當時負載影響，沒有以單次樣本推論架構速度。

| 項目 | x64 | x86 |
| --- | ---: | ---: |
| 資源初始化 | 13.677 ms | 71.756 ms |
| 初次顯示 | 9.086 ms | 29.811 ms |
| 重開中位數／p95 | 1.453／1.527 ms | 1.734／1.865 ms |
| 介面 Private Bytes 增量 | 16.93 MiB | 16.01 MiB |
| 介面 Working Set 增量 | 15.94 MiB | 13.90 MiB |

單一 x64 v4 引擎、學習與預測關閉、100 次短讀音／600 鍵：載入 369.317 ms，Private 增量 36.08 MiB，Working Set 增量 82.75 MiB，輸入後 Private 增量 36.42 MiB；按鍵呼叫中位數 0.001 ms／p95 0.033 ms。工具 `measure-engine.ps1` 連結 Rust Windows staticlib，C++ 只作量測 adapter。

背景 TSF Windows profile 的整個 harness 程序為 Private 40.3 MiB／Working Set 101.8 MiB；善解 profile（預測開啟）73.4／133.7 MiB。兩份預設的預測開關不同，這不是可泛化的多 App 總量或唯一實體 RAM。

## 2026-10-07 歷史量測

下方 v1／v2、舊原生介面與 WinUI host 數值只屬於當日版本，不能當成現在 v4／Rust 產品的耗用。

## 選字介面

候選已改成同程序 Win32／Direct2D software DC／DirectWrite，不再為選字啟動 WinUI 子程序。輸入法啟用時先初始化隱藏視窗；模型原本就在 `Engine::open` 載入，採唯讀映射，無須另加模型 preload 子程序。設定窗仍用 WinUI，只有開啟設定才啟動。

| 項目 | x64 | x86 |
| --- | ---: | ---: |
| 原生視窗與繪圖資源初始化 | 12.678 ms | 13.513 ms |
| 初始化後第一次顯示並完成同步繪製 | 7.340 ms | 8.589 ms |
| 後續隱藏／重開，中位數（30 次） | 2.227 ms | 2.599 ms |
| 後續 p95 | 2.424 ms | 2.834 ms |
| 選字介面額外 Private Bytes | 11.79 MiB | 11.64 MiB |
| 選字介面額外 Working Set | 14.54 MiB | 13.30 MiB |

舊 WinUI host 同機樣本：未預載從程序啟動到候選可見為 200–286 ms；預載後首次可見為 21–24 ms，後續約 1.4–1.8 ms，host 本身 Private Bytes 約 96 MiB、Working Set 約 134 MiB。舊測試使用三個候選與跨程序可見回覆；新測試使用九個候選並等待同步繪製完成，所以後續數字不能直接當成相同端點的速度排名。原生介面省去整個候選子程序，而不是只延後它的記憶體分配。

新量測的記憶體數字是測試程序開始前後的差值；不包含完整 App、TSF、引擎與設定窗。Working Set 包含共享頁，不能把多個 App 的 Working Set 加總當成唯一實體 RAM。Private Bytes 是私人提交記憶體，也不等於全部常駐 RAM。舊版已開啟 App 的 host 仍會存在，直到原 App 關閉或輸入法解除啟用；本次沒有終止使用者程式。

計時以 QPC 量測，不包含鍵盤硬體、App edit session 等待與螢幕刷新；無法保證實際 0 ms。啟用後尚未完成初始化就開始輸入，也仍須付出初始化時間。

## 核心與模型

各模型以獨立程序啟動，隔離個人資料，學習關閉；重複輸入固定測試讀音 100 次，共 600 個按鍵樣本。輸出不記錄使用者文字。

| 項目 | model-v1 | model-v2 |
| --- | ---: | ---: |
| `Engine::open` | 366.533 ms | 354.031 ms |
| 載入後 Private Bytes 增量 | 37.05 MiB | 37.05 MiB |
| 載入後 Working Set 增量 | 77.24 MiB | 77.91 MiB |
| 輸入後 Private Bytes 增量 | 36.43 MiB | 36.43 MiB |
| 按鍵呼叫 p95 | 0.042 ms | 0.042 ms |

這是單一引擎、短測試讀音的成本；長句、個人詞庫、學習與多個 TSF 實例可能不同。模型檔案頁可由 Windows 共用，但每個引擎仍有自己的詞庫和狀態。上述介面與引擎是分開量測，沒有直接加總宣稱整個輸入法的 RAM。

新版 Windows 操作預設停用即時預測，`Engine::open` 在載入模型前呼叫上游 prediction 開關，覆蓋層因此不建立預測索引；v3／v4 的詞類陣列與原有模型陣列一樣採唯讀映射。這是已核對的資源配置行為，不是新版 RAM／延遲實測。

舊版現有 WinUI 程序曾在 10 秒觀察內 CPU time 增量為 0，僅是該觀察期間與計時粒度下未測到 CPU，不能推論所有情況 CPU 都是零。新候選不做背景 polling；中英提示只在顯示期間啟用一次性 1400 ms timer。

## 重跑

```powershell
pwsh -File scripts/package.ps1
pwsh -File scripts/verify-ui.ps1 -Benchmark
pwsh -File scripts/measure-engine.ps1
pwsh -File scripts/verify-mode.ps1
```

候選與模式驗證都放在畫面外，不取得前景焦點。畫面存於 `build/native-review` 與 `build/mode-review`；設定頁存於 `build/ui-review-final`。實際工作列深色模式、Chrome／Office 日常操作、多螢幕 DPI 與讀屏尚未驗收。
