# model-v6／classes-v5 測試預期值

使用固定上游 `f8ea0bbed4e05ceda653cc73221660f3ee3f6873` 的原始 `reference/proto/lm_eval.py` 產生，沒有透過 Windows Engine 或複製它的輸出建立預期值。

配對原始檔的 SHA-256：

- model-v6：`fc2bddd1f0c2029deb7e7568f64868c11f535b3d5c5c7c97c3d0599c8b2cde42`
- classes-v5：`9e36fa1be4518adb34bcddc2c79761006b0b06c539aa64a4564ba00397cf0262`
- kn-v1：`cbefbced072f7e9d40dbfc3d2fad27c29f82e44bf801800dfb17b4eec962597e`

將三檔放在同一測試資料夾後，以 Python 3 執行（`PATH_TO_MODEL` 需換成該資料夾的 `bigram.sjlm`）：

```powershell
python upstream/shanjie/reference/proto/lm_eval.py --lm PATH_TO_MODEL --profile chat --dev 302 --dump chat.tsv
python upstream/shanjie/reference/proto/lm_eval.py --lm PATH_TO_MODEL --profile formal --dev 302 --dump formal.tsv
python upstream/shanjie/reference/proto/lm_eval.py --lm PATH_TO_MODEL --profile chat --rows upstream/shanjie/eval/probe/s2r-probe.txt --dump probe.tsv
```

取每份 dump 的第 2 欄等於 1 的列，第 3 欄就是 top-1。dev302 依列號合併 chat／formal，probe 保留單欄。預期值列數為 302／83，預設啟用上游 demote，沒有左側前文。三組 top-1（以 LF 串接、不含尾端 LF）的 SHA-256：

- chat：`4071ac031786fde7be47edf15bc2f4e53df93a9d93bceacf8e5a830af951dcab`
- formal：`56dba5a79c22eabd2c46e2ade21db2d3d470c1032ce269b50ba809cb2fdfc711`
- probe：`d32193c6306c236950cfd55149bf49ec128798d94ca4975e1a6d9c9e34dcf0e4`

`s2h-lm-context.txt` 使用相同三檔、`--context --dev 302` 依檔名排序讀取上游 `eval/dev/*.txt`。分別執行 chat／formal（例如 `python upstream/shanjie/reference/proto/lm_eval.py --lm PATH_TO_MODEL --profile chat --dev 302 --context --dump context-chat.tsv`），依上游相同格式保存兩個摘要。chat／formal 的 top-1 SHA-256 分別為 `d8ac58795e5649a557bcd62e7639cee2a9bbdbbe13860f13794ff95092ce5be8`、`3cd198d63b2efa6696ecf7fae1d3f6a1fb063f3c1b7b6cc32c86606685eb3292`。

原始評測語句與評測器來自上游，資料歸屬沿用包內 LICENSES。此處只有公開評測輸出，沒有私人字典或學習資料。KN 僅供上游整合測試使用，不會打包，也不表示 Windows 產品已啟用 KN。Windows 出貨配置未啟用 KN 的比較另外記於 VERIFICATION.md。
