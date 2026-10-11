# 本機英文辨識詞表

`data/english-common.txt` 衍生自 BlindingDark 的 Rime Easy English，固定來源：
https://github.com/BlindingDark/rime-easy-en/tree/54a4a07289412efc54134092c0d945f895a71ed3

上游 `easy_en.dict.yaml` SHA-256：`4f039026b2746fa9b0d4d7a248cdf866b64609dca2317708f04e9e68ac7d868a`。
依 LGPL-3.0 授權；完整授權見同目錄 `easy-en-LGPL-3.0.txt`。
本專案的改動：只保留 3–32 個小寫英文字母、詞與輸入碼相同、權重至少 990000 的項目，去重後排序；不使用其程式碼或自動完成邏輯。
產生腳本為 `scripts/prepare-english.ps1`。詞表隨來源及套件提供，可修改後重新執行核心 prepare 與 build；沒有網路查字或上傳輸入內容。
