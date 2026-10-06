# Windows 原生候選窗

操作模式：Operate。使用者在各種應用程式輸入文字，候選介面應讓位給正在編輯的文件。

遵循使用者選定的 Windows 原生方向。色彩來自 COLOR_WINDOW、COLOR_WINDOWTEXT、COLOR_HIGHLIGHT、COLOR_HIGHLIGHTTEXT，保留系統高對比設定。
Microsoft JhengHei UI 16 DIP，行高 36 DIP，內距 8 DIP。選取以系統反白呈現，不使用透明玻璃與動畫。
Windows profile 預設直列最多 9 個候選；→／Tab 展開為每列 9 個的格狀候選。Shanjie profile 保留上游橫列。使用數字、方向鍵與滑鼠操作。視窗不取得焦點，靠近組字游標，顯示器底部空間不足時移至游標上方。
個人設定可直接選擇直列、橫列或預設格狀顯示。原生設定窗使用 Windows 下拉選單、核取方塊與儲存／取消按鈕；預設選擇只改畫面中的草稿，儲存才寫入個人 INI。
依所在應用程式視窗 DPI 縮放，候選文字過長以省略號表示，完整文字仍由 TSF UI element 提供。
已看到原生候選窗並保存截圖；排版通知後重新定位以處理 TS_E_NOLAYOUT。觸控鍵盤遮擋、跨 App 焦點與實際多 DPI 仍須驗證。
