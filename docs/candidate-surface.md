# 候選窗方向契約

用途：Operate。快速選字，視窗不得取得焦點。

THESIS：依使用者最新指示移除毛玻璃，改成純白實色橫向膠囊與很細的淡灰 border。只有橫向，不恢復直向／格狀。

OWN-WORLD：正常模式固定 #FFFFFF、alpha 1；字詞 #222225、序號 #65656E、邊線 #D3D3D8。邊線 0.5 DIP、中心內縮 0.25 DIP；藍色選取 #006EE6 配白字。設定介面的淺深主題不再改變候選白底；高對比採系統 window／text／highlight 色。

STORY：選字、預測、聯想、窄区箭頭、mouse sequence 與失焦取消沿用原契約。被動預測沒有序號或反白，Tab 進入才顯示；普通候選不展開格狀。

FIRST VIEWPORT：字詞 16 DIP、序號 9 DIP、選取膠囊至少 24 DIP、外部高度至少 30 DIP、內距 3 DIP、項間距 2 DIP。依個人字級／16 縮放，字級 14–32；自然詞寬、工作區定位、caret 間距 6 DIP 不變。

FORM／FIT：單一 Rust Win32 layered popup，Direct2D／DirectWrite，NOACTIVATE、無動畫、無陰影；alpha 只供外角抗鋸齒，膠囊內部不透明。backdrop.rs、背景 HWND、WCA API 與透明度配方已移除，不保留 fallback 或第二層背景。字詞依自然寬度排列，窄區使用箭頭，只有單項超長才省略。

EVIDENCE：來源 windows/src/candidate.rs；原生 WM_PRINTCLIENT 擷取 build/native-review/{x64,x86}，PNG .impeccable/review/{x64,x86}。這輪圖片可核對實際白底、細邊線、文字與外角 alpha；背景 harness 不等同註冊 TIP 前景 App 或實際多螢幕／高對比驗收。過往玻璃、雙層窗口與其 policy 的結果均屬歷史。

FINISH：必要背景檢查、獨立視覺審查、DESIGN.md 與已驗證界線同步。
