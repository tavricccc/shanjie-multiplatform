# Windows 安裝包的模型與詞類表

核對版本：善解上游 `1c16dffa6be9f8900d9df9819343885b90427c01`。本文件補充上游 `LICENSES/data.md` 的 model-v3／classes-v1 清單，涵蓋本包全部模型；不取代上游詞庫授權與署名。

| 包內目錄 | 原始 Release 資產 | 資料授權 |
| --- | --- | --- |
| `data/models/model-v1/` | [model-v1/bigram.sjlm](https://github.com/Nanako0129/shanjie/releases/tag/model-v1) | CC BY-SA 4.0 |
| `data/models/model-v2/` | [model-v2/bigram.sjlm](https://github.com/Nanako0129/shanjie/releases/tag/model-v2) | CC BY-SA 4.0 |
| `data/models/model-v3/` | [model-v3/bigram.sjlm](https://github.com/Nanako0129/shanjie/releases/tag/model-v3)、[classes-v1/classes.sjc](https://github.com/Nanako0129/shanjie/releases/tag/classes-v1) | CC BY-SA 4.0 |
| `data/models/model-v4/` | [model-v4/bigram.sjlm](https://github.com/Nanako0129/shanjie/releases/tag/model-v4)、[classes-v2/classes.sjc](https://github.com/Nanako0129/shanjie/releases/tag/classes-v2) | CC BY-SA 4.0 |

依上游資料來源與授權聲明，模型來自中文維基百科語料、Mozilla Common Voice 繁中（臺灣）句子、Tatoeba 中文句子及 gpt-oss-120b 產生的臺灣口語合成句。署名：**Wikipedia contributors、Tatoeba contributors**。維基語料為 CC BY-SA 4.0；Common Voice 為 CC0；Tatoeba 為 CC BY 2.0 FR；合成句是模型輸出。模型及其同批計數產生的詞類表以 [Creative Commons Attribution-ShareAlike 4.0 International](https://creativecommons.org/licenses/by-sa/4.0/) 散布。

Windows 的修改為檔案格式轉換：`bigram.winmap` 使用 SJWM0002，將相同模型數值及配對詞類陣列排成可唯讀映射的格式，增加來源雜湊與對齊資訊；沒有重新訓練或修改詞頻。v3／v4 旁的 `classes.sjc` 是未修改的官方附件。轉換後的資料仍依 CC BY-SA 4.0 散布。所有來源網址、完整 SHA-256、配對與映射路徑記在包內 `MODEL-SOURCES.json`；轉換程式位於 Windows 專案的 `platform/map_model.rs`、`platform/mapped_lm.rs` 與 `scripts/prepare-models.ps1`。

程式碼的 Apache-2.0、詞庫的 MIT／CC BY-SA、圖示的 MIT 各自適用，不將整個 Windows 安裝包視為單一程式碼授權。
