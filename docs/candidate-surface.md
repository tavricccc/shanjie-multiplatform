# 候選窗方向契約

用途：Operate。使用者在文件中快速選字，候選不得搶焦點。

THESIS：對照 local-ime-reranker-public 的 Syllune 候選設定，保持緊密、無陰影與單層外框。

OWN-WORLD：淺色 #F5F6F7／#D8DADD／#24272B；深色 #24272B／#4A4F55／#F5F6F7。依使用者後續指定，反白使用 Windows accent 系列主題色，不採來源的淡青色。

STORY：先完成組字，再用所選開啟鍵進入候選；選字後可顯示聯想，失焦或取消會關閉。

FIRST VIEWPORT：外框圓角 6 DIP、1 DIP 邊框、內距 8×6 DIP；數字與字詞同列，間距 4 DIP、候選內距 4 DIP。保留直列、橫列與九欄格狀；18 DIP 候選字級仍由個人設定控制。

FORM：使用者指定參考，無隨機概念選擇；已核准候選改為 TSF 同程序 Win32 popup，設定保留 WinUI 3。Direct2D software DC render target 與 DirectWrite 繪製，實際 layout metrics 決定欄寬與列高；沒有獨立候選 host 或 IPC。保留 NOACTIVATE，顯示／隱藏、換頁及反白均無動畫。整個 HWND 為客戶區，只繪製單一不透明外框，不保留原生 caption／resize frame；選取圓角 4 DIP。

COLOR：`UISettings` 讀取淺色 `AccentDark1`／深色 `AccentLight2`，分別配白／黑字。高對比讀系統 window、window text、highlight 與 highlight text 色；來源已核對，尚未實際驗收。

FIT：字級 14–32 DIP，保留三種排列。長詞依實際量測加寬；窄工作區每格獨立 clip；來源設定 character ellipsis，本次 720 px captures 僅證明裁切不跨格，未驗證省略號可見，空間足夠時小字級長詞完整顯示。

EMOJI：候選字詞 `DrawTextLayout` 啟用 `D2D1_DRAW_TEXT_OPTIONS_ENABLE_COLOR_FONT`，數字標記保持原文字繪製。支援目前 Windows 字型已有的彩色 emoji、膚色與 ZWJ 序列；實際字形取決於已安裝字型，Windows 的旗幟目前呈現 TW 字母，不宣稱支援所有 emoji。其餘候選字體、量測、內距與排列維持原契約。

MODE：工作列保留使用者指定參考專案的 `rime/icons/chinese.ico`／`english.ico`，嵌入 DLL resource 101／102。游標旁提示使用 `assets/mode/chinese.svg`／`english.svg` 的 24×24 原始幾何，由 GDI+ 依實際螢幕 DPI 即時繪製 28 DIP 圖示，不縮放 ICO。SVG 同樣來自 `C:/Users/Tavric/projects/local-ime-reranker-public/rime/icons`，MIT 授權全文保留於 `assets/mode/LICENSE`。視窗使用 `WS_EX_LAYERED`、`UpdateLayeredWindow` 與 32-bit premultiplied alpha，無 `COLOR_WINDOW` 白底與外圈內距；6 DIP 只作 caret 間距。1400 ms 後或下一次輸入／失焦時隱藏，維持 NOACTIVATE；同 TSF 程序，不使用額外 host。

EVIDENCE：背景原生 BMP 在 `build/native-review/x64`／`x86`，模式圖示 capture 在 `build/mode-review`；無 HTML detector。來源為 `src/candidate_host.h`／`.cpp`、`candidate_palette.h`／`.cpp`、`mode_indicator.cpp` 與 `mode_button.cpp`。窄工作區長詞修正已重擷取，24 張一般與 4 張 720 px captures 有效；獨立 reviewer 確認裁切不跨格，兩項修正完成，限於本次 fix list；高對比、實際深色工作列、前景應用程式及多螢幕尚未驗收。背景圖片不等同日常使用驗收。

本次修正證據：`build/native-review/{x64,x86}/candidates-{1,2}-emoji.bmp` 呈現彩色 emoji 字詞；`build/mode-review/{chinese,english}-{96,120,144,192}.bmp` 保留 alpha，呈現指定 DPI 下的透明模式圖示。實際螢幕 DPI 來源已核對，圖片不等同多螢幕實際操作驗收；本次 emoji／透明提示的獨立 finish reviewer disposition 為 ship：😀／👍🏽／👩‍💻／❤️ 與混排彩色呈現、四種指定 DPI 的中／A 清楚、alpha 透明且白框消失均符合契約；旗幟 TW 仍屬字型限制，前景應用程式與實際桌面尚未驗收。前述窄工作區修正已完成狀態保持不變。

FINISH：unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance
