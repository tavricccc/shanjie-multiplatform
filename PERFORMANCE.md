# 延遲與資源量測

這些是背景 harness 實測，不是各種 App 的日常使用保證。只代表各次指定模型、核心與測試程序，不包含螢幕刷新或完整 App 的資源。

## 2026-10-12 Windows 0.5.0 共用核心

隔離整合測試中，首次自動啟動核心到會話可使用的單次樣本為 570 ms。常駐核心在多個會話與不同程序使用後，詞庫和模型載入計數仍各為 1；停止核心後重新連線及同時首次啟動的樣本亦各只載入一次。核心閒置 Private 約 39.8 MiB、Working Set 約 95.2 MiB，未包含個別 App 或繪圖程序，也不保證頁面永遠不被 Windows 換出。

新架構的「每程序背景預載」主要建立連線與會話，保留的模型與解析詞庫位於共用核心。主程序可能同時由 Windows 自動載入目前已安裝的舊 TIP，因此 TSF harness 的整個程序記憶體不能拿來精確推算新版前端的獨立成本。

隔離 TSF Windows profile 樣本：預載後六鍵組字 x64 3.418 ms，x86 2.635 ms；重新啟用加首鍵／取消 x64 7.434 ms，x86 5.977 ms。IPC 會增加每鍵通訊成本；目標是新 App 不再反覆承擔模型及詞庫初始化，而非宣稱解碼器本身更快。登入預載完成前，首鍵仍可能等待核心就緒；兩架構立即連續輸入案例均未遺失字元。

證據：`build/update-checks-20261012-222719/`、`build/version-0.5.0-tsf-x64.log`、`build/version-0.5.0-tsf-x86.log`。尚未測量真正登入時間或前景 App 鍵盤到畫面延遲。

最後一次測試在每個架構及鍵盤組合前停止共用核心，立即交付第一個字母：x64 標準／倚天為 839.775／846.270 ms，x86 標準／倚天為 930.872／1431.570 ms。等待後，七個字母均正確保留。這些是包含核心冷啟動的原生回呼等待時間；登入預載尚未完成時仍會有停頓，並非首鍵零延遲。對應 `first-keys-immediate` 四項結果。

## 2026-10-11 背景預載與核心常駐

啟用時在背景載入核心與拼音資料，同程序保留一個備用核心，TSF 停用／重建後可重用。背景 harness 用載入計數確認：預載期間銷毀再建立服務、預載後再次重新啟用，均只有一次載入；每次不靠重新讀模型恢復。DLL 隨 App 程序常駐，COM 計數在清理後仍歸零。沒有改動 WinUI 或候選 renderer。

| 本輪背景樣本 | x64 Windows profile | x86 Windows profile | x64 善解 profile |
| --- | ---: | ---: | ---: |
| TSF 啟用 | 1.920 ms | 2.379 ms | 4.572 ms |
| 測試等待背景預載完成 | 498.209 ms | 463.709 ms | 729.164 ms |
| 預載後首次六鍵中文組字 | 1.114 ms | 1.476 ms | 8.677 ms |
| 重建服務、啟用、首鍵及取消合計 | 1.275 ms | 1.051 ms | 1.644 ms |
| 預載後程序 Private／Working Set | 38.6／96.2 MiB | 37.9／95.8 MiB | 71.8／128.0 MiB |

預載等待以 5 ms 間隔檢查完成計數，包含排程誤差，並非核心函式的精確耗時；首鍵樣本是在預載完成後才開始。新程序若立即輸入仍會等待尚未完成的預載，沒有宣稱消除全新 App 冷啟動。上述記憶體是整個 headless 測試程序；不含實際 App／候選繪圖，Working Set 也不能跨 App 直接加總成唯一實體 RAM。善解 profile 開啟預測，不能與 Windows profile 當成相同工作量比較。

證據：`build/resident-tsf-x64.log`、`resident-tsf-x86.log`、`resident-tsf-shanjie-x64.log`。Win 快捷鍵、閒置失焦、背景載入失敗時的啟用／放行、組字與設定更新檢查皆通過。尚未更新系統安裝，沒有前景 App 或鍵盤到螢幕刷新量測。

## 2026-10-11 main／model-v5 延後載入版（歷史）

TSF Activate 不再同步建立核心或候選 renderer，OnTestKeyDown 只判斷按鍵；Win 快捷鍵在取 context、查 compartment 與任何核心 I/O 前放行。按鍵路徑沒有 INI metadata／變更監控。儲存時由設定程式主動通知；手動改檔由明確重啟讀取。

同機背景 TSF Windows profile：舊 v4 的 x64 Activate 為 442.997 ms、啟用後程序 Private 40.6 MiB／Working Set 103.4 MiB；新版 v5 最後樣本如下。這是啟用端點的改變，包含模型差異，不能當作相同模型解碼演算法的速度排名。新引擎成本移到首次確定要輸入時，沒有被消除。

| 最後背景樣本 | x64 | x86 |
| --- | ---: | ---: |
| TSF 啟用，headless | 1.982 ms | 3.934 ms |
| 啟用後程序 Private／Working Set | 3.8／19.5 MiB | 4.0／17.6 MiB |
| 無模型、原生隱藏通道啟用 | 1.725 ms | 2.347 ms |
| 閒置失焦，100 回呼合計 | 0.004 ms | 0.011 ms |
| 左 Win 快捷鍵，400 回呼合計 | 0.156 ms | 0.211 ms |
| 右 Win 快捷鍵，400 回呼合計 | 0.297 ms | 0.484 ms |
| 另一執行緒持有學習鎖時提交組字 | 0.011 ms | 0.027 ms |

舊 x64 的 100 次閒置失焦為 31.372 ms，左／右 Win 的 400 回呼各為 7.780／7.731 ms。這些是回呼合計，並不是鍵盤到畫面或 Win＋R／E 開啟時間。忙碌鎖測試確認同名 mutex 已存在、提交文字不變；上游 reset 不學習／不持久化，所以不再等待學習寫入鎖。

首次六鍵中文組字仍包含模型載入：本輪 x64 Windows 樣本 364.769–434.559 ms，x86 457.692–992.968 ms；善解 profile 預測開啟的單次樣本為 731.426／982.895 ms。樣本有其他建置／離屏程序負載，不以這些單次值比較架構或保證體感。未驗證真正註冊 TIP 在 Explorer／Run／Chrome 的開啟與輸入延遲。

單一 x64 v5 引擎、預測關閉、100 次短讀音／600 鍵：

| 項目 | 學習關閉 | 學習開啟、8,000 筆隔離 fixture |
| --- | ---: | ---: |
| Engine::open | 348.597 ms | 360.813 ms |
| 按鍵中位數／p95 | 0.001／0.032 ms | 0.052／0.217 ms |
| 載入後 Private 增量 | 37.59 MiB | 38.86 MiB |
| 載入後 Working Set 增量 | 84.71 MiB | 86.46 MiB |

學習的 mutex／epoch 同步仍保留；learning.tsv 版本未變時，不重新解析整份資料，亦不破壞核心既有 append／rewrite 狀態。自己的寫入在釋放 mutex 前更新版本；其他實例寫入或清除在下一次學習操作載入。以上按鍵樣本不包含 Enter 持久化，不能當作學習提交磁碟延遲的量測。

192 DPI 的候選 renderer：x64／x86 初始化 15.164／19.705 ms、首次同步繪製 4.745／5.080 ms、30 次重開中位數 1.323／1.565 ms、p95 1.672／1.846 ms，Private 增量 15.00／14.09 MiB。資源現在只在首次真正顯示候選時建立；TSF 啟用不付這筆成本。端點不含螢幕刷新。證據與重跑命令見 VERIFICATION.md；學習樣本用 `pwsh -File scripts/measure-engine.ps1 -Learning`。

## 2026-10-09 Rust／model-v4 歷史版本

後續無邊線毛玻璃版本：同樣 120 DPI／九候選／30 次重開，x64 初始化 12.523 ms、首顯 5.992 ms、warm 中位數／p95 2.425／2.881 ms、Private／Working Set 增量 13.51／14.74 MiB；x86 分別 15.775 ms、6.465 ms、2.793／3.258 ms、12.87／13.37 MiB。端點包含兩 HWND 定位及同步文字／tint 繪製，不包含螢幕刷新或 DWM 真實桌面模糊的 GPU 延遲／記憶體。下表是加入毛玻璃之前的歷史同日樣本，兩者不作架構速度排名。

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
