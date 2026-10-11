# 設定介面擴充

Operate 模式；沿用 WinUI 3 的控制項、字體、系統色彩、840×680 DIP 初始尺寸與固定儲存／回饋列。

## Direction contract

THESIS：讓使用者找到造成操作差異的設定，載入微軟按鍵預設後能逐項改動；每個控制項都接到實際行為。

OWN-WORLD：既有 Windows 原生介面與 DESIGN.md；擴充分組，不建立新的視覺風格。

STORY：先選操作預設及模型，再按「按鍵、組字、選字、預測與學習」調整；Windows 預設兩種預測皆關閉。草稿儲存後通知所有已啟用實例。

FIRST VIEWPORT：左側原生 NavigationView，右側標題與同列標籤／開關；說明只解釋狀態和操作。儲存、重新載入與回饋維持固定位置。模型及完整配對的說明可捲到並讀完。

本次局部修訂：在「組字與提交」保留「中英混打自動判斷」，移除免聲調／簡拼開關及其舊設定。原始按鍵顯示於組字區，注音另以小提示呈現，完整聲調轉中文；拼音鍵盤時停用混打控制項但保存偏好。「介面與選字框主題」同時套用至 WinUI 與原生候選，其餘控制項、可捲動表單及固定儲存列保持既有幾何。

FORM：延伸既有設定表單；沒有新的視覺世界或構圖抽籤。共用 SettingToggle，設定解析、schema 遷移、個人詞庫與 ViewModel 分開維護。

FINISH：unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance

驗證採本機真實 XAML 離屏 render，涵蓋淺／深色及新控制項；不啟用使用者視窗，不等同新安裝輸入法的實際 App 驗收。
