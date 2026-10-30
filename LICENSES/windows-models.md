# Windows 安裝包的模型

核對版本：善解 v0.6.0，`a03e8bf158cbeb8f0a1ec645711469bb6b8b7e0e`。本包內建的模型組與上游 App 相同，放在 `data/models/builtin/`；其他相容配套由設定的線上模型管理另行下載。本文件補充同目錄的上游 `data.md`，不取代上游詞庫與模型的授權與署名；各模型的語料來源、各來源授權與署名以 `data.md` 對應的列為準。

| 包內檔案 | 原始 Release 資產 | 資料授權 |
| --- | --- | --- |
| `bigram.winmap` | [通語 SJ-LM-7-9.7M](https://github.com/Nanako0129/shanjie/releases/tag/sj-lm-7-9.7m) `bigram.sjlm` | CC BY-SA 4.0 |
| `classes.sjc` | [辨類 SJ-WC-6-512C](https://github.com/Nanako0129/shanjie/releases/tag/sj-wc-6-512c) | CC BY-SA 4.0 |
| `kn.sjkn` | [承接 SJ-KN-2-397K](https://github.com/Nanako0129/shanjie/releases/tag/sj-kn-2-397k) | CC BY-SA 4.0 |
| `trigram.sjt3` | [連文 SJ-CX-2-7.7M](https://github.com/Nanako0129/shanjie/releases/tag/sj-cx-2-7.7m)（書面設定） | CC BY-SA 4.0 |
| `trigram-chat.sjt3` | [連文 SJ-CX-2-950K](https://github.com/Nanako0129/shanjie/releases/tag/sj-cx-2-950k)（聊天設定） | CC BY-SA 4.0 |
| `tinylm.sjtl` | [會意 SJ-SR-7-15M](https://github.com/Nanako0129/shanjie/releases/tag/sj-sr-7-15m)（語意校正） | CC BY-SA 4.0 |
| `colloquial.sjlm` | [常談 SJ-CL-2-69K](https://github.com/Nanako0129/shanjie/releases/tag/sj-cl-2-69k)（聊天設定的語意校正） | CC BY-SA 4.0；口語池各來源的授權見 `data.md` |

署名：**Wikipedia contributors、Tatoeba contributors**。常談口語池逐一來源的署名（YODAS 影片、Tatoeba 作者、WildChat 與 MASSIVE 的出處）在同目錄的 `colloquial-attribution.tsv`，原樣取自上游附件。

Windows 只對 `bigram.sjlm` 做檔案格式轉換：`bigram.winmap` 使用 SJWM0002，把相同的模型數值與配對詞類陣列排成可唯讀映射的格式，增加來源雜湊與對齊資訊；沒有重新訓練或修改詞頻，轉換後仍依 CC BY-SA 4.0 散布。其餘檔案是未修改的上游附件。來源網址、完整 SHA-256 與映射檔名記在包內 `MODEL-SOURCES.json`；轉換程式位於 `platform/map_model.rs`、`platform/mapped_lm.rs` 與 `scripts/prepare-models.ps1`。

程式碼的 Apache-2.0、詞庫與詞包的 MIT／CC BY-SA、圖示的 MIT 各自適用，不將整個 Windows 安裝包視為單一程式碼授權。

外部下載的配套（例如 model-v6＋classes-v5、model-v6＋classes-v4）同為上游以 CC BY-SA 4.0 發布的資料，來源與完整 SHA-256 見 `MODEL-CATALOG.json` 與各官方 Release。這些檔案不在本安裝包內；使用者模型與個人詞庫分開保存。
