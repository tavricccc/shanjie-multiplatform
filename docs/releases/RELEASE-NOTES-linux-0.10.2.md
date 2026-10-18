# 善解 Linux 0.10.2（首個 Linux 測試版）

在 Fedora 44 GNOME（Wayland）以 IBus 使用善解注音。引擎、設定與安裝全部是 Rust，與 Windows 版共用同一份上游核心、輸入決策、設定檔格式（schema 7）與內建 model-v6＋classes-v5。

## 安裝

下載 `shanjie-linux-0.10.2-x86_64.tar.gz`，解壓後以一般使用者執行 `./install.sh`。腳本以 sudo 安裝系統檔，並把「善解注音」加到 GNOME 輸入來源最後一項，不切換目前來源。需求：IBus、GTK 4.12+、libadwaita 1.6+、curl。

- 引擎：`/usr/libexec/ibus-engine-shanjie`（IBus component `/usr/share/ibus/component/shanjie.xml`）
- 設定：`/usr/bin/shanjie-settings`，應用程式清單的「善解設定」，或輸入法選單的設定
- 個人設定、詞庫、學習與下載的模型：`~/.local/share/shanjie`
- 移除：`./uninstall.sh`；個人資料預設保留，`--delete-user-data` 會移到 `~/.local/share/shanjie.deleted_files/<時間>`

## 功能

- 標準注音、倚天、漢語拼音、臺灣拼音；中英切換（Shift、Ctrl＋空白、Caps Lock）、全半形、Windows 注音順序與上游英文詞表混打、標點與符號快捷鍵、聲調符號、預測、聯想、學習、個人詞庫，與 Windows 版相同的按鍵決策。
- 候選使用 GNOME 原生 IBus 候選框，橫向或直式；未完成組字在切換焦點時提交一次。
- 設定程式（GTK 4＋libadwaita）沿用 Windows 設定的九頁與全部項目：操作預設載入／匯入／匯出、模型選擇與線上相容配套下載（可續傳，SHA-256 核對後轉換驗證）、個人詞庫三方合併儲存、清除學習。儲存後經 session bus 通知執行中的引擎立即套用，「重新啟動輸入法」重建核心。

## GNOME 上的差異

- 候選框外觀、字型大小與主題由 GNOME 決定；「候選文字大小」不生效，「主題」只套用到設定視窗。
- 中／英狀態顯示在頂端列，沒有游標旁提示。
- 尚未提供反白加詞（連按兩次 Ctrl／Ctrl＋Shift＋F12）。
- 中英混打時，候選框上方沒有 Windows 那一列「整句＋注音提示」，只顯示注音提示。

驗證範圍見 [VERIFICATION.md](../../VERIFICATION.md)。
