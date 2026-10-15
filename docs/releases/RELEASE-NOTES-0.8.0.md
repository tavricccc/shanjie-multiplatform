# 善解 Windows 0.8.0

日期：2026-10-15。本版把上游同步到 `feat/model-v6-switch`，內建語言模型換成 model-v6＋classes-v4；重畫輸入法與模式圖示；修正語言清單中輸入法名稱後面出現亂碼；安裝時改用按鈕選擇標準或倚天注音。沿用 0.7.0 的混打、亂序注音與空白鍵設定，以及 0.6.1 的設定共存與提權等待。

## 上游與語言模型

- 上游 pin 由 main 的 `368eebc` 改為 `feat/model-v6-switch` 的 `f8ea0bbed4e05ceda653cc73221660f3ee3f6873`（約 1,040 個 commit，含 main 的 `76dc7ac`）。該分支尚未合入 main，待作者發佈後再改回 main 的對應 commit。
- 內建模型換成 model-v6＋classes-v4。設定頁的模型說明與連結同步更新；安裝包只允許最新模型，舊的 model-v5 不再提供。
- 候選資料契約新增 `pick_key` 欄位，學習儲存匯入新增 `is_full_context`、`NEXT_PREFIX`；Windows 常駐核心改用完整路徑的 atomic 型別。這些只是配合上游的 prepare 調整，輸入行為沒有因此改動。
- **未接入：** kn-v1 Kneser–Ney 側檔、tinylm-v5 與重排強度設定。Windows 沒有新增對應設定或載入；上游測試期待的 `kn.sjkn` 只在驗證腳本中下載，不打包。

## 安裝與語言清單

- **修正名稱亂碼：** 註冊輸入法時，描述字串（「善解注音（標準）」「善解注音（倚天）」）沒有結尾 NUL，Windows 語言清單讀取時會越過字串尾端，在名稱後顯示亂碼。現在描述與圖示路徑都補上結尾 NUL。要重新安裝並登出再登入，語言清單才會更新。
- **選擇鍵盤配置：** 安裝時選標準或倚天，原本是「是／否／取消」的訊息方塊，現在是 Windows 工作對話框：「標準注音」「倚天注音」兩個大按鈕加「取消」，內文顯示目前清單狀態，並說明個人詞庫與偏好會保留。`shanjie-manager.exe` 因此嵌入 comctl32 v6 manifest 依賴（`TaskDialogIndirect` 需要）。解除安裝詢問是否刪除個人資料的對話框仍是原本的是／否／取消。

## 圖示

輸入法清單、工作列模式、設定程式與 WinUI 資產共用同一套圖示：輸入法為單色「解」，中文模式為「中」、英文模式為「A」。改為純色、不使用漸層，並新增 `assets/mode/shanjie.*`；`scripts/make-icons.ps1` 可重新產生，授權說明見 `LICENSES/mode-icons.md`。

## 建置與驗證界線

見 [VERIFICATION.md](../../VERIFICATION.md)。已驗證：x64／x86 release 與 WinUI 建置、`package.ps1` 打包，以及上游整合測試、Windows interaction 與 TSF smoke（x64）。**未驗證：** 沒有重跑 `verify-update.ps1`、x86 TSF smoke 與 WinUI `--verify-ui`；沒有安裝到系統，因此亂碼修正、鍵盤選擇對話框與新圖示都還沒在實機確認。上游 pin 是未合入 main 的分支。
