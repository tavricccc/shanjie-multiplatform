# 圖示來源

三個單色圖示：深灰圓角方塊（純色，無漸層、無內框）加白色粗體無襯線字。

- `shanjie`：解，輸入法本身——TSF 輸入法清單的圖示（DLL 資源 100，圖示索引 0）、設定程式、「已安裝的應用程式」與磁貼圖片。
- `chinese`：中，中文模式（DLL 資源 101）。
- `english`：A，英文模式（DLL 資源 102）。

中／A 的 16–24 px 圖示與浮動提示的 128 px 母圖，字形約占八成高度；ICO 的 32–256 px 圖層維持原大小。只重建模式圖示可用 `scripts/make-icons.ps1 -ModesOnly`。

`scripts/make-icons.ps1` 以 Noto Sans TC 粗體繪製並輸出 `*.ico`（16–256 px，PNG 內嵌）、`*.png`（128 px 母圖；中、A 的版本由游標旁提示 `include_bytes!` 後依 DPI 縮放）與 `*.svg`。授權說明見 `LICENSES/mode-icons.md`。
