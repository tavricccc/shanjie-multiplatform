# 善解 Windows

## Platform
Windows 10 / 11 原生 TSF 輸入法。

## 使用目的
將善解上游注音引擎接入 Windows，支援標準與倚天鍵盤、整句轉換、候選選擇。
使用者已選定 Windows 原生風格，優先低資源與日常操作。
Windows 操作與上游操作以個人 INI profile 切換；Windows 預設遵循微軟注音常用鍵位，不修改上游核心。

## 維護界線
Windows 是固定版本上游 C ABI 的 consumer。引擎演算法與資料來源由上游負責。
平台覆蓋檔只接入建置副本，子模組保持乾淨。不上網傳送輸入內容。

## 尚未驗證
真實應用程式相容性、簽章發佈、ARM64。
