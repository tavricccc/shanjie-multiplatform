# 關於頁的貢獻者資料

名單來自 [GitHub Contributors API](https://api.github.com/repos/tavricccc/shanjie-multiplatform/contributors?per_page=100)，依 `contributions` 由多至少排列。這是 GitHub 對帳號的提交統計，不另計共同作者或 PR 數；只列人類帳號，不列 GitHub、Claude 等自動化帳號。

資料快照在 `ui/Assets/contributors.json`，包含擷取日期、對應 repository commit、帳號、提交數、貢獻摘要與頭像來源。開啟預覽卡片時，先透過 HTTPS 從 GitHub 的 `avatarSource` 載入頭像，失敗時使用 `ui/Assets/Contributors/` 的本地頭像。名單與提交數不會連線更新；點擊個人頁面或提交紀錄時開啟外部瀏覽器。

更新名單時：

1. 用 `gh api 'repos/tavricccc/shanjie-multiplatform/contributors?per_page=100' --paginate` 取得全部帳號與提交數，保留 `type=User`，排除 `[bot]` 與已知自動化帳號。
2. 更新 JSON 的日期、repository commit、提交數與名單，從帳號的 commit／已合併 PR 確認每人的摘要；不要把整合模型寫成訓練模型。
3. 有新帳號時，加入其公開 GitHub 頭像並記下 `avatarSource`。沒有公開名稱時使用帳號名稱。
4. `VisualVerifier` 的 `about-models` 項目涵蓋排序、排除機器人、頭像連線載入／失敗後使用本地檔、卡片主題與模型選單；`contributors` 只補驗貢獻者介面，一般 UI 驗證也會執行這些項目。

目前快照為 2026-11-09、`85843a42551574925f99f78f44f701a8efd7264a`。程式不會自動更新提交數。

互動參考 NitLink HolyBear Edition 的貢獻者預覽方式，使用善解的字體、配色與圓角，以及 WinUI 3 Desktop Acrylic 彈出層。滑入姓名停留 200 ms 後顯示卡片，離開姓名與卡片後延遲 180 ms 關閉，避免快速經過或移向卡片時反覆開關。姓名左側的 GitHub Mark 取自 [Primer Octicons](https://github.com/primer/octicons/blob/main/icons/mark-github-24.svg)，MIT 授權全文在 `LICENSES/octicons.txt`。
