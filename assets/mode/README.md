# 中英圖示來源

沿用上游善解印章圖示（`upstream/shanjie/site/favicon.svg`）的形狀：圓角方框、內框與粗體襯線字。中：上游紅底「解」；英：灰藍底「A」，框與字相同，供模式區分。

`scripts/make-icons.ps1` 以 Noto Serif TC Black 繪製並輸出：`*.ico`（16–256 px，PNG 內嵌，在 x64／x86 DLL 的 101／102 resource，供工作列使用）、`*.png`（128 px 母圖，由游標旁提示 `include_bytes!` 後依實際 DPI 縮放）與 `*.svg`。16／20／24 px 省略內框，讓字形保持清楚。授權說明見 `LICENSES/mode-icons.md`。
