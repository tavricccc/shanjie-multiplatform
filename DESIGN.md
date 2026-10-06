# Windows 原生候選窗

操作模式：Operate。使用者在各種應用程式輸入文字，候選介面應讓位給正在編輯的文件。

遵循使用者選定的 Windows 原生方向。色彩來自 COLOR_WINDOW、COLOR_WINDOWTEXT、COLOR_HIGHLIGHT、COLOR_HIGHLIGHTTEXT，保留系統高對比設定。
Microsoft JhengHei UI 16 DIP，行高 36 DIP，內距 8 DIP。選取以系統反白呈現，不使用透明玻璃與動畫。
每列最多 9 個候選，使用數字、方向鍵與滑鼠操作。視窗不取得焦點，靠近組字游標，顯示器底部空間不足時移至游標上方。
依所在應用程式視窗 DPI 縮放，候選文字過長以省略號表示，完整文字仍由 TSF UI element 提供。
原生視覺與觸控鍵盤遮擋尚待真實 App 驗證；不得將編譯通過視為視覺驗收。
