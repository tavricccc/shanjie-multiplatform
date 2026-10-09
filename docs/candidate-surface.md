# 候選窗方向契約

用途：Operate。輸入時快速辨識與選字，視窗不得取得焦點。

THESIS：依使用者提供的 macOS 善解截圖與後續指定，使用無邊線的單行毛玻璃膠囊、小型序號與藍色選取膠囊。只保留橫向；直向與九欄格狀移除。

OWN-WORLD：淺色底調純白 #FFFFFF、alpha 0.55；深色 #343443、alpha 0.74，讓後方 Windows acrylic 更通透。文字與小序號共用 #222225／#F5F5F7，保持背景變化時的可讀性；選取 #006EE6 搭白字，文字與反白維持不透明。高對比或系統關閉 AdvancedEffects 時使用實色底。

STORY：完成讀音後開啟候選，數字、滑鼠或 Enter 選詞，左右與 Tab 導覽；一般候選的上下移動映射至左右，不進入格狀。預測與聯想仍由個人設定控制；被動預測沒有序號或反白，Tab 進入選詞才顯示。聯想顯示 Shift＋序號。失焦、取消與選取後依原輸入規則關閉。

FIRST VIEWPORT：預設字級 16 DIP、序號 9 DIP、選取膠囊高 24 DIP、外框高至少 30 DIP、外框內距 3 DIP、項間距 2 DIP；序號前 4 DIP、序號與文字間 6 DIP、尾端 7 DIP。依實際 DirectWrite metrics 加高，個人字級 14–32 DIP 按比例調整。caret 間距 6 DIP，工作區不足時移至上方並夾在工作區內。

FORM／FIT：Rust TSF 同程序 Win32：native acrylic HWND 在下方，owned layered HWND 在上方畫 tint、字詞與反白。兩層同步大小、位置與隱藏，皆 NOACTIVATE。移除負 margin 的 DWM frame extension，明確停用 non-client rendering 與 Windows 11 自動圓角裝飾；只有自行控制的膠囊裁切。GetRgnBox 核對 blur region 四邊對稱內縮 1 px，不能把 GDI 的輸入右／下邊界直接當成輸出範圍。Windows 10 沒有圓角屬性時僅略過該屬性，不改用另一 renderer。字詞自然寬度與選字契約保持不變。

MODE：工作列 ICO 維持資源 101／102，caret 提示由 Rust tiny-skia 畫既有 SVG 幾何、28 DIP、1400 ms，透明與 NOACTIVATE。設定介面保留 WinUI 3，業務邏輯共用 Rust。

EVIDENCE：來源 windows/src/candidate.rs、backdrop.rs、native.rs、tsf/。SetWindowCompositionAttribute／ACCENT_ENABLE_ACRYLICBLURBEHIND 在獨立模組管理，API 沒有 SDK import/header，動態從 user32 解析，沒有靜默實色替代。x64／x86 harness 的 acrylic policy 成功啟用且窗口不搶焦點／同步隱藏。WM_PRINTCLIENT 只擷取字詞與 tint 層，不包含 DWM 背後的實際桌面模糊；BMP 先把 premultiplied BGRA 轉成 straight alpha 才保存。PNG 在 .impeccable/review/{x64,x86}。實際桌面毛玻璃、多螢幕、高對比尚未實機驗收，不用離屏 PNG 宣称真實合成已驗證。

FINISH：完成必要背景驗證、獨立 finish review、DESIGN.md 與產生物來源紀錄。
