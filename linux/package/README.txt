善解注音 Linux（IBus）

需求：Fedora 44 GNOME（或其他有 IBus、GTK 4.12+、libadwaita 1.6+、curl 的 x86_64 發行版）。

安裝：解壓後以一般使用者執行
  ./install.sh
腳本會用 sudo 安裝系統檔，並把「善解注音」加到 GNOME 輸入來源的最後一項。
安裝到其他位置：PREFIX=/usr/local ./install.sh

設定：應用程式清單中的「善解設定」，或執行 shanjie-settings。
個人設定、詞庫、學習與下載的模型位於 ~/.local/share/shanjie。

移除：./uninstall.sh
個人資料預設保留；./uninstall.sh --delete-user-data 會一併刪除，無法復原。

GNOME 上的差異：候選框由 GNOME 繪製（外觀、字型大小、主題跟隨系統）；
中／英狀態顯示在頂端列，沒有游標旁提示；尚未提供反白加詞。
