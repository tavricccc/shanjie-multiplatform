# Windows 移植驗證紀錄

最新日期：2026-10-09。上游 pin：正式 v0.3.0 的 `8a6fe0750284f7daa46d197545f50eee9bb3387c`，與子模組／UPSTREAM.json 一致。下方舊日期均為歷史證據，不表示本轮已安裝或仍提供舊模型／排列。

## 2026-10-09 最新模型、Rust TSF 與橫向候選

### 最新方向：移除毛玻璃，純白實色與細邊線

依使用者最新指示，完全刪除 backdrop.rs／背景 HWND／WCA composition API 與相關 features，只保留單一 NOACTIVATE layered 候選窗。正文固定 #FFFFFF、alpha 255；外角 alpha 只作曲線抗鋸齒。邊線 #D3D3D8、0.5 DIP、內縮 0.25 DIP；字詞 #222225、序號 #65656E、藍色反白與既有字級／幾何保留。候選在兩種設定主題下均為白底；WinUI 控制項改稱「設定介面主題」。

x64／x86 建置、原生候選／WinUI 背景檢查及 x64 TSF 檢查通過；候選 owner 為 null，NOACTIVATE、滑鼠、過期點擊、隱藏／重開、隱藏後主題通知均通過。新鮮 reviewer 逐張核對 36 張候選與 2 張設定 appearance PNG，確認正文 RGB255／alpha255、外角alpha0、細邊線與單窗契約；disposition **ship，限定本輪背景原生繪製／離屏審查**。本輪未執行系統安裝、前景 App、高對比、讀屏或多螢幕驗收。下方所有毛玻璃、透明度與雙層窗口紀錄已是歷史，不代表目前產品。

### 後續外觀修正：無邊線、純白底調與毛玻璃

最新使用者截圖回報右側異常輪廓及不夠通透：淺色 tint 從 0.82 調至 0.55，深色調至 0.74，小序號改用相同文字色以維持可讀性。背景取消負 margin 的系統 frame extension，停用 non-client rendering／Windows 11 自動圓角裝飾；另外用 GetRgnBox 檢查既有 blur region 四邊對稱內縮 1 px，遵循 GDI 輸入／輸出邊界語意。來源與既有 caption／NOACTIVATE 契約保持分離。下段 0.82 與上一審查為本日早先版本的歷史證據，不代表目前參數；本輪檢查結果另記於下方。

本輪 x64／x86 建置與候選／WinUI 背景檢查通過：原生 acrylic 啟用、讀回 DWMWA_NCRENDERING_ENABLED=false、實際裁切範圍 454×38 → (1,1,453,37)、NOACTIVATE、主題通知後雙層保持隱藏。新鮮 reviewer 對照使用者截圖並逐張核對 36 張前景 PNG，淺色 RGB 255／alpha 140，深色 alpha 189；disposition ship 限於 foreground／底調／native frame 契約。沒有宣稱桌面陰影已消失或日常透度已驗收，也未更新使用者系統安裝。

使用者指定移除描邊，淺色底調改 #FFFFFF，背景使用原生 acrylic。`backdrop.rs` 隔離動態 user32 composition API／私有 accent policy，非 layered 材質 HWND 在下方、owned layered 文字與 0.82 alpha tint 在上方。兩層皆 NOACTIVATE，大小、位置與隱藏同步；高對比或系統停用 AdvancedEffects 時遵循實色設定。

x64／x86 重建與 TSF 背景 harness 通過；兩架構原生候選、模式圖示、WinUI 離屏保存／擷取也通過。原生 acrylic policy 返回成功；新增 hide 後發送三種 theme／settings 訊息的斷言，兩 HWND 均保持隱藏。BMP／PNG 在保存前將 Direct2D premultiplied BGRA 轉成 straight alpha，避免白色半透明底被錯誤加深。

新鮮 reviewer 逐張核對 36 張最新前景層 PNG：無描邊、淺色純白、alpha 209／255、角落透明與閱讀層級符合契約。初輪唯一 material fix 是隱藏後主題通知會重新顯示底層；修正並重建／擷取後，同一 reviewer 判定該項 resolved，disposition **ship，限於該 fix list 與前景層／原生 policy 證據**。WM_PRINTCLIENT 不包含 DWM 實際桌面模糊，未將這些 PNG 當成真實毛玻璃觀感驗收；本輪沒有更新系統安裝。

### 本次上游與功能驗證

- `prepare-core.ps1` 覆蓋契約通過。新 pin 與前一 pin 的 core/src、C header、詞庫及授權相同；更新包含上游 fixture／文件與 macOS 候選高度／位置。子模組追蹤檔案乾淨。
- `verify.ps1` 完整核心回歸 185 項通過：lib 41、engine 32、demote 6、learning 45、lm 16、prediction 43、replay 2；標準／倚天各 302 句。5 個上游 ignored 效能測試、global_eps_table、Unix 權限與 macOS 效能仍排除，沒有計入通過數。
- `prepare-models.ps1` 核對 model-v4／classes-v2 官方 SHA-256、全部映射陣列及代表性分數等價，清除退休模型資料／快取。包只允許最新固定配對，舊模型設定遷移至 v4，没有執行期替代版本。
- x64／x86 生產 TSF DLL 皆由 Rust cdylib 建置成功；x64 WinUI 與 native manager 建置成功。產品 C++ 已移除，`tests/support/` 僅為測試 ABI adapters。
- Windows crate 兩個必要測試通過：schema／preset 遷移保留個人資料并移除退休選項；巢狀退休 payload 刪除。實際包內 manager 的隔離 apply-profile 檢查通過：固定 schema 4／v4／horizontal，保留詞庫指向、學習與字級。
- x64／x86 的 Windows／善解兩種背景 TSF harness 均通過真正 composition、edit session、caret、commit、cancel、candidate UI element 與 COM lifetime。只有 active TIP keyboard-sink 註冊使用 ThreadProxy；未註冊系統輸入法。language bar AddItem 在未註冊 harness 不可用，不阻擋輸入啟用；實際工作列仍待安裝驗收。
- x64 interaction 通過最新模型、缺少／錯配詞類拒絕、passive／entered 預測、即時停用、Big5、Ctrl 標點、拼音、字碼／符號、個人詞庫、交錯學習及清除 epoch。
- x64／x86 原生候選與模式圖示 harness 通過同程序、NOACTIVATE、mouse sequence、隱藏／重開、client＝window、RGBA 角落透明與提示自動隱藏。候選 36 張實際原生擷取覆盖淺深、14／16／32 DIP、自然詞寬、720 px 工作區、聯想、emoji、被動／進入預測。當前環境 120 DPI；不是實際多螢幕验收。
- WinUI 離屏保存／重讀通過舊設定備份、最新模型與 horizontal 遷移、保留詞庫／學習／外觀，以及實際 ToggleSwitch 草稿雙向綁定。設定淺深擷取包含六頁與長頁細節；Rust共用業務邏輯，新畫面不提供模型／排列選單。
- 全部 PowerShell 7 腳本語法通過；Rust dependency licenses 從 Cargo.lock 鎖定版本收集全文。資源量測與端點界線見 PERFORMANCE.md。
- 最終套件的 native manager 校驗通過；ZIP 內 589 個檔案逐一 SHA-256 核對通過，僅有 model-v4＋classes-v2、沒有退休安裝腳本，WinUI 旁的 Rust DLL 與 x64 TSF DLL 相同。ZIP 約 111.68 MiB。

候選擷取來源 `build/native-review/{x64,x86}`，36 張保留 alpha 的 PNG 副本在 `.impeccable/review/{x64,x86}`；WinUI 在 `build/ui-review-final`。沒有影像生成素材或 HTML detector。獨立 reviewer 已逐一核對 36 張候選與 6 張相關設定圖，可見畫面全部 match／adaptation；唯一 material fix 是舊 DESIGN.md 的世界與來源記錄。新鮮 documenter 已修正 DESIGN.md／sidecar，同一 reviewer 判定该項 resolved，最後 disposition 為 **ship，限定該文件 fix list**。

安裝器已實作完整套件雜湊校驗、兩架構註冊與實際路徑／DLL 雜湊核對、註冊失敗回復、唯一版本 payload、舊版／舊根目錄清理及 reboot queue。解除安裝在任何刪除前詢問保留詞庫與學習、全刪或取消，清 COM／TSF／本帳號輸入法清單／Installed Apps／安裝目錄／所選個人資料。**本輪未執行系統安裝或解除安裝，管理員流程與重新啟動後清理尚未實測**；fixture 通過不能冒充實機結果。

前景記事本／Chrome／Office、實際工作列、讀屏、高對比、多螢幕與 App 容器仍未驗收。没有更動真實個人設定、詞庫或學習，也沒有重新啟動使用者 App。先前已停止的 Shift 日常問題未重開。

## 2026-10-08 後續預設值更新

依使用者目前已儲存的 schema 3 設定更新 Windows 範本及 C++／WinUI 內嵌預設：學習開啟，Caps Lock 英文、提交後空白、拼音單空白轉換、反引號標點關閉。個人詞庫路徑與內容不納入產品預設，既有設定不改寫；schema 1／2 仍保留舊版隱含值。這次只重建受影響的 x64／x86 DLL 與 x64 設定程式，再使用已有模型打包；依使用者要求不追加測試，下列回歸及 capture 證據屬於這次預設值更新前。

## 2026-10-08 四版模型、配對詞類與 schema 3

- 六份原始 Release 附件 SHA-256 與 `MODEL-SOURCES.json` 一致；v1／v2 的九個模型陣列、v3／v4 的九個模型及三個詞類陣列，連同標量與代表性語言分數的映射等價核對通過。缺少詞類表或將 classes-v1 配給 v4 時明確拒絕，放回正確配表才成功。沒有舊模型 fallback。
- 上游與平台核心回歸 **185 個通過**：lib 41、engine 32、demote 6、learning 45、lm 16、prediction 43、replay 2；標準／倚天各回放 302 句。demote 的 1 項與 prediction 的 4 項 upstream ignored 量測未執行；global_eps_table 仍排除，Unix 權限與 macOS 效能排除沿用既有界線。降權測試的 Unix symlink fixture 在建置測試副本改為 copy，保留行為測試。子模組追蹤檔案未改。
- 最新 x64／x86 Rust release core 與 TSF DLL、self-contained x64 WinUI 設定程式建置通過。x64 Windows／Shanjie 背景 TSF harness 通過，包含 v3／v4 重載、即時預測與聯想開關、Enter 提交選項、空白政策、未轉換拼音在重建／失焦時保留。harness 仍以 ThreadProxy 代理 keyboard-sink 註冊，未替代實際 edit session 或 composition。
- x64 interaction harness 通過四版模型、詞類錯配、被動／進入預測、即時停用、Big5、Ctrl 標點、額外標點／聲調開關、重複 ↑ 循環保持游標、拼音單空白、符號鍵盤等必要檢查。測試使用隔離個人資料；沒有修改使用者 profile、詞庫或學習。
- x64／x86 原生候選 harness 通過 passive／entered 預測、滑鼠 sequence、NOACTIVATE、過期 frame、隱藏／重開與單框。滑鼠 fixture 改用實際 client 尺寸取得命中點，通過目前 192 DPI 環境；這不是多螢幕 DPI 驗收。預測 BMP 在 `build/native-review/{x64,x86}/prediction-{1,2}-{passive,entered}.bmp`。
- WinUI 離屏檢查通過 schema 2 備份／保留模型、學習與外觀、Windows／Shanjie 預設、四版模型保存／重讀，以及實際即時預測／Caps Lock ToggleSwitch 雙向更新共用草稿。淺／深兩種主題擷取六頁頂端、額外按鍵、候選政策、外觀、詞庫及錯誤回饋；證據在 `build/ui-review-final/`。命令列 profile 保留偏好的隔離檢查與 PowerShell 7 腳本解析通過。
- `download-asset.ps1` 的實際 classes-v2 範圍下載完成，組合後 SHA-256 與官方值一致；模型轉換另外核對六份快取原檔。模型與詞類授權補充在包內 `LICENSES/windows-models.md`。
- 獨立 impeccable reviewer 核對 22 張設定 PNG 與 8 張兩架構預測 BMP；初輪其他視覺項為 match，要求確認 Caps Lock 開關及深色詞庫的導覽殘留。複查相同三張畫面確認開關可見、殘留消失，兩項均 resolved，disposition 為 **ship，限定該兩項 fix list**。擷取先 render 一幀再等待原生 transition 靜定，未變更使用者的控制項動畫或外觀。

Windows 新安裝預設 v4，兩種預測皆關閉；schema 1／2 保留現有個別選項與未指定模型時的 v1，設定窗讀取時備份原檔再遷移。完整操作／設定索引在 `MICROSOFT-IME-DIFFS.md`。這些結果是背景整合與離屏驗證；本輪沒有系統安裝、前景 App 操作、讀屏、高對比或實際多螢幕驗收，也未重開先前已停止的 Shift 日常除錯。

## 2026-10-07 WinUI 3 版本

後續 emoji／中英提示修正：候選 DrawTextLayout 啟用 Windows 彩色字型；x64／x86 淺深 captures 包含表情、膚色、ZWJ、愛心與中英混合內容。Windows 原生 emoji 字型目前將臺灣旗幟顯示為 TW，沒有宣稱所有旗幟圖樣可用。游標提示移除白色底板與外圈留白，改成 28 DIP 透明 layered window；依原 SVG 幾何以 GDI+ 在實際 monitor DPI 繪製，工作列仍使用 ICO。96／120／144／192 DPI 兩種圖示 capture 保留 alpha，角落透明像素與 NOACTIVATE／自動隱藏檢查通過。新畫面尚未安裝後日常驗收。

此輪 x64／x86 建置、原生候選／模式提示 harness 及背景 TSF 回歸通過。獨立 impeccable reviewer 的 disposition 為 ship，範圍限於彩色 emoji 與透明矢量提示的背景 captures／來源，未包含真實桌面合成、多螢幕或更新後前景日常操作。

最新架構：依使用者確認，候選改為 DLL 內 Win32／Direct2D software DC／DirectWrite；WinUI 只保留設定頁。移除匿名 mapping/event、候選 IPC、XAML 候選與其 interop，啟用時建立隱藏原生視窗。保持原密度、系統 AccentDark1／AccentLight2、字體、字級與三排列；受限工作區逐格裁切，不會畫入下一個候選。彩色中／A ICO 嵌入兩架構 DLL，來源見 `assets/mode/README.md`；游標旁提示不搶焦點，1400 ms、開始打字或失去焦點後隱藏。

量測與方法界線見 `PERFORMANCE.md`：新候選介面增加約 11.6–11.8 MiB Private Bytes，原生資源初始化約 13 ms，初始化後首繪約 7–9 ms；舊獨立 WinUI host 約 96 MiB。這些不是完整 App 或整個輸入法記憶體，也不是螢幕刷新延遲。本次尚未自動安裝、未終止既有 App；下方舊 WinUI 候選／IPC 驗證記錄保留為歷史，不能當作新版原生實作的證據。

最終 x64／x86 建置通過；兩架構原生候選 harness 通過同程序、滑鼠 sequence、NOACTIVATE、單框、隱藏／重開。Windows／Shanjie 兩種背景 TSF 操作回歸通過。兩架構中英圖示與 96／144 DPI 提示大小、手動隱藏／1400 ms 自動隱藏通過。獨立 impeccable reviewer 逐張核對 24 張一般、4 張 720 px 窄工作區候選與模式提示 BMP；跨格修正及文件來源兩項均 resolved，最後 disposition 為 ship，限定該 fix list。窄畫面只證明每格裁切，不宣稱省略號可見。WinUI 設定離屏保存／重讀與五頁 captures 通過。

最新增量：保留聯想功能，在設定用「預測尚未輸入的後續詞」開關關閉。Windows Enter 選定最後候選後，透過核心 Enter 正式提交（保留學習提交流程）並關窗，不接續開聯想。格狀第一列 ↑ 停留而不收回列表；TableList Loaded 只更新 panel 尺寸，不重新切換候選排列與視窗。

安裝包提供 model-v1／model-v2，維持既有設定預設 v1。v2 的官方 SHA-256 `8847b73a7b9cf127b4882328191c3c5250fe9a55912926d5050e351ab644d240` 核對通過；兩版 9 個映射陣列及標量等價檢查通過。新背景 TSF regression 通過 v1→v2→v1 即時重載、Enter 後沒有 UI element／不再攔截下一次 Enter、格狀第一列 ↑ 保持展開、聯想仍可開啟、關閉設定即清掉現有聯想且後續數字選字不再顯示。善解上游操作 profile 與兩架構 host checks 通過；WinUI 模型與預測開關保存／重讀、淺／深畫面通過獨立窄範圍審查。這些不包含前景 App 日常驗收；沒有將使用者現有模型改成 v2，也未自動安裝。

後續候選介面修正：依使用者要求移除動畫與雙層外框。ListView／GridView 共用靜態 ContentControl template，只有立即套用的 VSM setters，ItemContainerTransitions 為空；不再使用預設 ListViewItemPresenter 轉場。取消原生 caption／resize frame 與 WinUI 非客戶區保留，WM_NCCALCSIZE 回傳完整客戶區；DWM 非客戶區繪製與視窗轉場均停用。兩架構 host harness 確認 client 尺寸等於 window 尺寸、DWM non-client rendering 關閉，NOACTIVATE／隱藏／重開／退出仍通過。淺／深與各排列 XAML captures 通過，獨立視覺審查 verdict 為 ship；保留原本 accent、字體與間距。此修正版尚未做前景 App 日常驗收，也未自動重新觸發先前已取消的安裝 UAC。

本次候選修正版：上游更新至正式 0.2.1；與舊 pin 比較 core、詞庫及授權沒有差異。112 個核心／學習／回放測試通過，標準／倚天各 302 句；排除界線維持下文。

- 設定窗及套用 profile 通知已啟用的 TSF 實例立即重讀；直接改 INI 在下一鍵讀取。核心設定變更先提交組字再重建。
- Windows Down 開啟政策不再由一般提交觸發聯想；聯想移至選定候選後，保留原組字，Enter 關閉聯想並提交。聯想選取會提交組字與後續詞，和微軟私有引擎仍有差異。
- 新增背景 TSF regression：第一聲空白不開候選、完整組字空白不彈聯想、同一服務 live trigger 更新、thread focus 結束候選、核心重載保留文字。此 harness 仍使用 keyboard-sink 代理，不代表新安裝 App 的日常驗收。
- IPC 不再因 writer 正在換 frame 而丟失自動重設 event；過期 dispatcher frame 不可重開，已知 HWND 立即隱藏。兩架構 WinUI host harness 通過。
- 真實 XAML captures 覆蓋淺／深、三種排列、14／18／32 DIP、長詞及聯想。反白使用 Windows accent，外框參考 Syllune；獨立 reviewer 修正 presenter 反白圓角後 verdict 為 ship。圖片位於 build/ui-review-final；沒有新生成素材。

尚未驗證修正版在記事本、Chrome、Office 的註冊 TIP 日常操作、多螢幕 DPI、讀屏或高對比。UI 畫面外驗證不會啟用使用者視窗；上述行為通過不構成真實應用程式驗收。

最終 x64／x86 建置、兩種操作 profile 與核心重載保留文字檢查通過；103.4 MiB ZIP 的 484 個檔案 SHA-256 核對通過。本次安裝 UAC 回傳「操作被使用者取消」，兩架構 COM 路徑仍指向舊版本 `2000710DDE0EC67B2DC7D1EC`，DLL hash 均不等於新建置；因此修正版尚未安裝。換版需要一次重新開啟原本載有舊 DLL 的程式，這與新版設定的即時套用是不同階段。

記事本實際測試：透過 computer-use 插件在已安裝版本輸入 s、u、3，↓ 開啟 WinUI 候選，下一次 ↓ 反白第二項「妳」，Enter 選取後組字為「妳」。已看到實際候選文字，並非只確認 HWND 可見。Chrome 測試因 Computer Use 無法辨識新分頁 URL 的安全檢查而中止；Chrome 的使用者回報仍未驗證或排除。

使用者遠端畫面回報看不到候選；重測時記事本前景啟動兩次失敗，Computer Use 回報 `failed to activate captured window`，停止輸入操作。保存 `build/notepad-test-result.png`（選取「妳」後的畫面）；此前工具擷取有完整候選文字。此差異不能證明遠端軟體是根因，也不能據此宣稱 Chrome 問題修好。

工作列中英圖示原先設定 TEXTCOLORICON，卻傳回固定黑色的 32-bit 彩色 icon。已改成真正 1-bit AND／XOR 單色遮罩，由 TSF 套用主題文字色；中／英兩狀態、x64／x86 的 GetIconInfo 格式契約測試通過。來源為 Microsoft 的 ITfLangBarItemButton::GetIcon 與 TF_LBI_STYLE_TEXTCOLORICON 文件。新圖示尚未做安裝後工作列的深色模式實際驗收。

後續密度調整：依使用者照片移除注音按鈕與頁次 footer、取消 item container 預設留白、依實際文字測量候選大小、收緊五頁設定並把開關排成同列。驗證使用畫面外的真實 WinUI capture，包含 14／18／32 DIP 字級、長詞、聯想與三種排列；沒有改候選核心與方向鍵政策。

密度複查發現 GridView 預設 item 尺寸仍沿用短詞配置，14 DIP 四字詞出現省略；已讓 container 與 ItemsWrapGrid 使用同一計算寬高並完成 layout 後更新 panel。候選仍保留九欄的上游格狀數字與方向鍵契約。

另外以明確 panel 寬度隔離 ScrollViewer viewport 的像素取整，淺／深色最小字級圖均完整顯示同列 1–9。獨立複查 verdict 為 accept／ship。x64／x86 host harness、profile 保存與離屏 capture 通過；新版 ZIP 的 UI SHA-256 與最後建置一致。

- x64／x86 TSF DLL 與 self-contained x64 WinUI 3 host 編譯成功；此次沒有更新系統安裝。
- 核心測試共 112 個通過（lib 33、engine 32、learning 45、replay 2），標準／倚天各回放 302 句。Unix／macOS 排除與 global_eps_table 界線沿用下文。
- 背景 TSF harness 通過。此回測量 Private Bytes 74.8 MiB、Working Set 164.3 MiB，僅代表該測試程序，未計入 WinUI host 或真實多 App 總量。
- `verify-interaction.ps1`：排列與按鍵政策分離、Unicode／Big5／符號、個人詞庫、聯想詞、漢語／臺灣拼音、交錯引擎學習、注音提交與 epoch 清除通過。測試使用隔離的個人資料夾。
- `verify-ui.ps1`：x64／x86 實際啟動 WinUI host、繼承 handle、NOACTIVATE、隱藏／重開、退出通過；真實 XAML 離屏 capture 包含淺／深色五頁、三種候選排列、儲存成功／錯誤、善解操作說明。
- 獨立 impeccable 審查找到固定儲存列的回饋可見性、profile 說明及維護文件三處問題，已統一修正。Capture 位於 `build/ui-review-final/`。
- 複查修正後的 preset severity 殘留也已解決，獨立 verdict 為 accept／ship。兩張 error-then-preset 跟隨系統淺色，不算深色證據。
- 新 ZIP 103.4 MiB，包內所有 SHA-256 核對通過，最終 UI／腳本／文件與來源一致，所有 PowerShell 腳本解析通過。命令列預設保留個人詞庫指向的隔離驗證通過。

這些是背景整合與離屏視覺驗證，不代表新版本已在記事本、Chrome、Office、讀屏、高對比、多螢幕 DPI、App 容器通過日常驗收。下文的安裝／記事本記錄只適用於 10 月 6 日的舊 Win32 介面版本。

## 2026-10-06 舊版本紀錄

## 已驗證

- Rust core、原生 TSF DLL：x64 與 x86 MSVC release 編譯成功。
- 上游 model-v1：SHA-256 `9879fd8b264b1c1f4c083ccedf84dc5625cd8d2595bd2d13150eed5a0520a923` 符合上游檔案。
- 映射模型：所有 9 個資料陣列、標量與代表性詞頻／分數核對一致。
- 109 個上游 core、engine、learning、replay 測試通過。標準與倚天各回放 302 句。
- x64 背景 TSF harness：真正 Windows context、edit session、composition、caret、commit、Esc、候選 element 與 COM lifetime 通過；不開視窗、不註冊系統輸入法。
- impeccable 對候選原生介面原始碼的機械檢查未回報問題；已實際擷取原生候選窗，確認文字與系統反白。多 DPI、讀屏尚未驗收。
- Windows／Shanjie 兩份 INI 均實際載入並通過背景 TSF 測試。Windows：↓ 開啟／移動、數字與 Enter 選字、Shift 中英切換；Shanjie：原本左右候選與 Shift 行為。
- 可見 TSF 測試重現首次 GetTextExt 無排版資料，收到 OnLayoutChange 後原生候選窗可見。
- 新版註冊 DLL 已在重新開啟的 Win32 記事本驗證：↓ 開啟直列候選、↓ 移至「妳」、Enter 選字／提交、Shift 切換英文、Shift＋空白後輸出全形 `ｂ`。
- 打包的 CMD profile 套用入口已實際寫入個人 INI，不要求管理員權限。
- 新增右側候選核心測試兩個，上游原有 109 個測試仍通過；原有左侧模式沒有被替換。
- TSF harness：外部選取通知後游標位置同步、句中右側選字、左右 Shift 切換中英通過。
- 原生設定窗已在本機開啟、讀取控制項、保存截圖並按「儲存」，核對個人 INI 的 `anchor=right`、`open=down` 與切換開關。
- 新的中英切換不再為空組字要求同步寫入鎖；有組字時以非同步 edit session 提交。系統模式按鈕已實作，尚待新安裝版本的實際系統匣驗收。
- 右鍵管理員安裝／移除入口已納入安裝包；PowerShell 腳本語法與包內 SHA-256 校驗通過。已透過 UAC 完成系統安裝註冊，未實際驗證右鍵入口互動與重新啟動後刪除。

背景 harness 在引擎啟用後量到整個程序 Private Bytes 41.3 MiB、Working Set 95.4 MiB。Working Set 包含模型映射頁面；此測量不能當作多 App 總耗用、閒置 CPU、輸入延遲或真實 App RAM 的實測。

## 驗證範圍

TSF harness 的文字儲存是記憶體中的 ITextStoreACP，TSF manager 和 context 是 Windows 實作。Windows 要求 active TIP 才可註冊 foreground keyboard sink，所以 harness 以 ThreadProxy 代替該單一步驟；未替代 edit session 或 composition。

上游的 Unix 權限／FIFO／連結拒絕測試與 macOS getrusage 效能測試在 Windows 建置副本中使用 cfg 排除。
上游 global_eps_table 是學習參數掃描，移植驗證不重跑它。這些項目不能計入 Windows 已通過。

## 尚待驗證

管理員解除安裝與重新啟動後刪除、數位簽章、Chrome/Edge/Office/Terminal 相容性、密碼與受保護欄位、焦點／滑鼠游標變更、實際多螢幕 DPI、觸控鍵盤遮擋、讀屏與 App 容器。

已在本機完成管理員安裝；核對 64 位元與 32 位元 COM DLL 路徑、兩個 TSF profile 註冊與全部安裝檔案 SHA-256。Microsoft Store 版 PowerShell 7.6.6 x64 已確認可用。使用者輸入法清單包含善解的兩個 profile，保留原本順序與既有項目；真實 App 驗證限於上述記事本操作。

上述舊版本保持學習關閉。2026-10-07 已加入 Windows 多程序協調與可選學習功能，驗證範圍見本頁最新紀錄。

本次完成可建置、可打包、已做背景整合驗證的開發版。日常可用與正式發佈仍需上述實際系統驗收。

新增原生設定窗與右側候選版已建置／打包。首次更新 UAC 回傳取消；2026-10-06 使用者要求重新觸發後，安裝退出碼為 0，兩架構 COM 註冊指向新版本目錄，全部安裝檔案 SHA-256 與設定窗執行檔均已核對。已開啟的程式仍可能載有舊 DLL，需重新開啟後驗收新行為。
