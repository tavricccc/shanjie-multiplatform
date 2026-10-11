# 候選窗方向契約

用途：Operate。快速選字，視窗不得取得焦點。

THESIS：維持實色橫向膠囊與細邊線，修正深色模式仍為白底的問題；中英混打增加與參考圖相同用途的獨立注音提示。只有橫向，不恢復直向／格狀或毛玻璃。

OWN-WORLD：淺色 #FFFFFF／字詞 #222225／序號 #65656E／邊線 #D3D3D8；深色 #202020／字詞 #F5F5F7／序號 #B8B8C2／邊線 #505055。邊線 0.5 DIP、中心內縮 0.25 DIP；藍色選取 #006EE6 配白字。兩主題皆不透明，與設定窗共用主題偏好；自動讀取 Windows AppsUseLightTheme，高對比優先採系統色。

STORY：選字、預測、聯想、窄區箭頭、mouse sequence 與失焦取消沿用原契約。被動預測沒有序號或反白，Tab 進入才顯示。原始按鍵如 gj 在文字區顯示，下方以藍底白字顯示 ㄕㄨ，沒有序號、不接受點選、聲調不作選字。提示沿用同一個原生窗口與尺寸。

FIRST VIEWPORT：字詞 16 DIP、序號 9 DIP、選取膠囊至少 24 DIP、外部高度至少 30 DIP、內距 3 DIP、項間距 2 DIP。依個人字級／16 縮放，字級 14–32；自然詞寬、工作區定位、caret 間距 6 DIP 不變。

FORM／FIT：單一 Rust Win32 layered popup，Direct2D／DirectWrite，NOACTIVATE、無動畫、無陰影；alpha 只供外角抗鋸齒，膠囊內部不透明。backdrop.rs、背景 HWND、WCA API 與透明度配方已移除，不保留 fallback 或第二層背景。字詞依自然寬度排列，窄區使用箭頭，只有單項超長才省略。

EVIDENCE：來源 windows/src/candidate.rs；本輪原生 WM_PRINTCLIENT 擷取 build/native-review/{x64,x86}/candidates-{1,2}-horizontal.bmp 與 mixed-hint-{1,2}.bmp。以本輪 BMP 為準，早期 .impeccable/review 中白底 PNG 為歷史。背景 harness 不等同註冊 TIP 前景 App 或實際多螢幕／高對比驗收。

FINISH：必要背景檢查、獨立視覺審查、DESIGN.md 與已驗證界線同步。
