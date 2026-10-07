# 原生候選與 WinUI 3 設定維護

2026-10-07 使用者確認：設定頁保留 WinUI 3，候選移回 DLL 內的 Win32 視窗，降低資源與首次啟動成本。沿用 Windows／善解操作 profile，Windows 模式的直列、橫列都以 ↓ 逐項選字。

## 來源

- Microsoft [AI-assisted WinUI migration](https://learn.microsoft.com/en-us/windows/apps/develop/ai-assisted/migrate/)。
- Microsoft [win-dev-skills](https://github.com/microsoft/win-dev-skills)，本次讀取 commit `2e8c902e9ba675e4c57550e066fc7e48b64c49b0` 的 `winui-design`、`winui-dev-workflow`、`winui-setup`；以 WinApp CLI 0.7.1 查範例及專案實際 API。
- UI 品質流程採用本機 impeccable skill。視覺方向已由使用者指定，不另換主題或用網站視覺樣式模仿原生控制項。

## Direction contract

THESIS：設定用 Windows 11 的操作慣例，候選讓位給正在編輯的文件；只有開設定才啟動 WinUI 程式。

OWN-WORLD：Windows accent 與淺／深中性色，繁體中文採 Microsoft JhengHei UI。設定使用 NavigationView、ComboBox、ToggleSwitch、NumberBox、InfoBar；候選由 Direct2D／DirectWrite 繪製，保留既有密度与單框。

STORY：載入 profile → 修改草稿 → 儲存 → 通知已啟用的善解立即套用。鍵盤配置與操作政策分開；需要換引擎的設定先提交目前組字。候選不取得鍵盤焦點，滑鼠結果經 TSF edit session 提交。

FIRST VIEWPORT：設定 840×680 DIP，208 DIP 側欄，內容 20 DIP 留白、原生表單標題與 12 DIP 群組間隔，底部固定儲存／取消。候選依實際文字測量與顯示器工作區 sizing，螢幕底部不足時向上移。依使用者圈選移除底部注音按鈕與頁次提示。

FORM：使用者指定的 Windows 原生方向；code-led，沒有影像 mockup 或概念抽籤。表單可捲動，候選支援直列、橫列與格狀。

FINISH：unreviewed and undocumented is unfinished; this build ends with the finish review, the verdict, DESIGN.md, and every shipping raster carrying its provenance.

## 架構

TSF DLL 是 C++，負責 composition、edit session、按鍵與 TSF UI element。候選是同程序 Win32 popup：Direct2D software DC target 與 DirectWrite 量測／繪字；開啟輸入法時初始化隱藏視窗，不啟動候選子程序。WinUI 設定是獨立 x64 程序，不將 WinUI runtime 載入文件 App。

`src/candidate_host.*` 直接持有候選視窗與文字 layouts，`candidate_palette.*` 取得系統 accent 與高對比色；舊 mapping/event、候選協定與 XAML 候選已刪除。更新與隱藏在 TSF 執行緒同步完成，沒有跨程序 dispatcher 的過期更新。候選文字不經網路、剪貼簿或磁碟檔傳遞。長詞受每格 clipping 限制，不會畫入相鄰候選。

滑鼠 callback 帶畫面 sequence；收到 callback 與非同步 edit session 執行時分別核對版本。Windows 的系統輸入法清單與系統匣由 TSF／Windows 提供，不重畫成自有 WinUI shell。

## 打包與資料

維持既有 ZIP／管理員 regsvr32 安裝流程。WinUI host 是 .NET 10 與 Windows App SDK self-contained unpackaged exe，這是為了可由已註冊的 TSF DLL 直接啟動；沒有另加 MSIX 身分或改寫使用者的語言設定。只引用 WinUI component package，不打包 AI／模型推論 SDK。

DLL 與 UI 都放在 `versions/<DLL+UI hash>/`；模型仍共用根目錄 `data/`。`ui-path.txt` 指向目前設定程式。更新不覆寫正在使用的 UI 二進位；已開啟 App 的舊 TIP 仍搭配自己的 UI。

個人 profile schema 2 延伸 schema 1 的 INI。個人詞庫保存成內容雜湊命名的不可變檔，先完成詞庫寫入，再原子替換指向它的 INI；失敗不會把半份詞庫當作已保存草稿。既有 `dictionary.tsv` 可直接載入。

WinUI 載入預設保留個人詞庫草稿；命令列套用不含 dictionary 的範本也保留目前詞庫指向。解除安裝取消註冊及刪除 profile.ini，個人詞庫／學習檔保留。

學習透過同一個使用者目錄衍生的 named mutex 串行化：每次引擎操作先重新讀取學習，再寫回。清除學習同樣取得 mutex，更新 epoch，其他引擎下次操作會丟棄尚未提交的舊學習紀錄。沒有更改上游學習評分公式。

## 驗證入口

```powershell
pwsh -File scripts/verify.ps1
pwsh -File scripts/verify.ps1 -NativeOnly -Interaction shanjie
pwsh -File scripts/verify-interaction.ps1
pwsh -File scripts/verify-ui.ps1
pwsh -File scripts/package.ps1
```

設定驗證建立畫面外、不 activate 的真實 WinUI 視窗，保存 XAML render PNG 到 `build/ui-review-final/`；沒有變更真實個人 INI。原生候選 harness 驗證同程序、NOACTIVATE、單框、滑鼠 sequence、隱藏／重開，透過真實 WM_PRINTCLIENT 繪製保存 BMP 到 `build/native-review/x64` 與 `x86`。模式圖示與提示檢查用 `verify-mode.ps1`；量測用 `measure-engine.ps1`。這不等同已完成 Chrome、Office、工作列、讀屏、高對比與多螢幕日常驗收。測量界線與結果見 `PERFORMANCE.md`。
