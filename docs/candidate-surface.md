# 候選窗方向契約

用途：Operate。輸入時快速辨識與選字，視窗不得取得焦點。

THESIS：依使用者提供的 macOS 善解截圖與上游 CandidateCells／InputController，使用單行膠囊、細邊框、小型序號與藍色選取膠囊。使用者已明確選擇「只保留橫向」；直向與九欄格狀均移除。

OWN-WORLD：深色背景 #343443、邊框 #787884、文字 #F5F5F7、序號 #B9B9C5；淺色背景 #F2F2F5、邊框 #B5B5BD、文字 #222225、序號 #65656E；選取 #006EE6 搭白字。系統模式依 Windows 主題，高對比使用系統 window／highlight 色。

STORY：完成讀音後開啟候選，數字、滑鼠或 Enter 選詞，左右與 Tab 導覽；一般候選的上下移動映射至左右，不進入格狀。預測與聯想仍由個人設定控制；被動預測沒有序號或反白，Tab 進入選詞才顯示。聯想顯示 Shift＋序號。失焦、取消與選取後依原輸入規則關閉。

FIRST VIEWPORT：預設字級 16 DIP、序號 9 DIP、選取膠囊高 24 DIP、外框高至少 30 DIP、外框內距 3 DIP、項間距 2 DIP；序號前 4 DIP、序號與文字間 6 DIP、尾端 7 DIP。依實際 DirectWrite metrics 加高，個人字級 14–32 DIP 按比例調整。caret 間距 6 DIP，工作區不足時移至上方並夾在工作區內。

FORM／FIT：Rust TSF 同程序 Win32 layered popup；Direct2D／DirectWrite，透明 alpha 圓角，沒有 caption、resize frame、動畫或候選 host。各詞按自然寬度量測；窄螢幕只顯示放得下的項目，兩端箭頭翻動可見範圍，鍵盤選取始終可見；單一超長詞才使用省略。滑鼠回傳原候選索引與序號，過期點擊不提交。彩色 emoji 使用 Windows 可用字型；旗幟字形取決於作業系統。

MODE：工作列 ICO 維持資源 101／102，caret 提示由 Rust tiny-skia 畫既有 SVG 幾何、28 DIP、1400 ms，透明與 NOACTIVATE。設定介面保留 WinUI 3，業務邏輯共用 Rust。

EVIDENCE：來源 windows/src/candidate.rs、native.rs、tsf/；背景擷取 build/native-review/{x64,x86}，模式圖示 build/mode-review，WinUI build/ui-review-final。圖片來自實際原生繪製，無 HTML detector。背景擷取不等同前景 App、實際工作列、多螢幕或高對比實機驗收。最終審查與驗證結果記錄於 DESIGN.md、VERIFICATION.md。

FINISH：完成必要背景驗證、獨立 finish review、DESIGN.md 與產生物來源紀錄。
