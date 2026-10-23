#Requires -Version 7
param([Parameter(Mandatory)][string]$OutputRoot,[string]$BasePackage,[string]$BuildId='0.12.1-diag.20261023.3-a9767ab')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$BasePackage){
 $installedRoot=Join-Path $env:ProgramFiles 'Shanjie'
 $version=(Get-Content "$installedRoot/installed.json" -Raw|ConvertFrom-Json).version
 if($version -notmatch '^[a-f0-9]{16}-[a-f0-9]+$'){throw 'Unexpected installed version identifier.'}
 $BasePackage=Join-Path "$installedRoot/versions" $version
}
$BasePackage=[IO.Path]::GetFullPath($BasePackage)
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot)
if(Test-Path -LiteralPath $OutputRoot){throw 'Use a new output directory; existing artifacts are never overwritten.'}
$basePrefix=$BasePackage.TrimEnd('\','/')+[IO.Path]::DirectorySeparatorChar
$entries=@()
foreach($line in Get-Content "$BasePackage/SHA256SUMS.txt"){
 if($line -notmatch '^([a-fA-F0-9]{64})  (.+)$'){throw 'Malformed base package manifest.'}
 $digest=$Matches[1];$relative=$Matches[2];$part=$relative -split '[\\/]'
 if($part[0] -notin @('data','docs','LICENSES','profiles','scripts','tools','ui','x64','x86','Changelog.md','MODEL-CATALOG.json','MODEL-SOURCES.json','README.md','UPSTREAM-CHANGES.md','UPSTREAM.json','安裝善解.exe','安裝說明.txt')){throw "Non-public root in base manifest: $relative"}
 $path=[IO.Path]::GetFullPath((Join-Path $BasePackage $relative))
 if(!$path.StartsWith($basePrefix,[StringComparison]::OrdinalIgnoreCase)){throw 'Base manifest escapes its directory.'}
 if((Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $digest){throw "Base package file failed verification: $relative"}
 $entries+=$relative
}
if((Get-Content "$BasePackage/README.md" -Raw) -notmatch '0\.12\.1'){throw 'Expected a 0.12.1 public base package.'}
foreach($arch in @('x86_64','i686')){
 if(!(Test-Path "$root/windows/target/$arch-pc-windows-msvc/release/shanjie_windows.dll")){throw "Missing diagnostic DLL: $arch"}
}
$original=Join-Path $OutputRoot 'original-0.12.1'
$diagnostic=Join-Path $OutputRoot 'diagnostic-0.12.1'
New-Item -ItemType Directory -Path $original,$diagnostic -Force|Out-Null
foreach($relative in $entries){
 foreach($destination in @($original,$diagnostic)){
  $path=Join-Path $destination $relative
  New-Item -ItemType Directory -Path (Split-Path $path) -Force|Out-Null
  Copy-Item -LiteralPath (Join-Path $BasePackage $relative) -Destination $path
 }
}
Copy-Item "$BasePackage/SHA256SUMS.txt" $original
$release=Join-Path $root 'windows/target/x86_64-pc-windows-msvc/release'
Copy-Item "$release/shanjie_windows.dll" "$diagnostic/x64/Shanjie.dll" -Force
Copy-Item "$root/windows/target/i686-pc-windows-msvc/release/shanjie_windows.dll" "$diagnostic/x86/Shanjie.dll" -Force
Copy-Item "$release/shanjie_windows.dll" "$diagnostic/ui/shanjie_windows.dll" -Force
foreach($name in @('shanjie-core.exe','shanjie-manager.exe')){Copy-Item "$release/$name" "$diagnostic/tools/$name" -Force}
Copy-Item "$release/shanjie-setup.exe" "$diagnostic/安裝善解.exe" -Force
Copy-Item "$release/shanjie-diagnostics.exe" "$diagnostic/診斷控制台.exe"
$sources=@('windows/Cargo.toml','windows/build.rs','windows/src/input_diagnostics.rs','windows/src/bin/shanjie-diagnostics.rs','windows/src/tsf/diagnostics.rs','windows/src/tsf/keys.rs','windows/src/tsf/add_word.rs','windows/src/native.rs','windows/src/tsf/modifiers.rs','windows/src/tsf/service.rs','windows/src/tsf/mod.rs','windows/src/lib.rs')
$provenance=[ordered]@{build=$BuildId;source_head=(& git -C $root rev-parse HEAD);base_version='0.12.1';upstream=(Get-Content "$root/UPSTREAM.json" -Raw|ConvertFrom-Json).commit;feature='input-diagnostics';public_base=$BasePackage;source_files=@()}
foreach($source in $sources){$provenance.source_files+=@{file=$source;sha256=(Get-FileHash "$root/$source" -Algorithm SHA256).Hash.ToLowerInvariant()}}
$provenance|ConvertTo-Json -Depth 5|Set-Content "$diagnostic/DIAGNOSTICS-BUILD.json" -Encoding utf8NoBOM
$readme=@'
善解 0.12.1 輸入診斷包（第三版：加詞浮層）

這是診斷版，沒有套用 Shift／Excel／加詞浮層的修法。一般正式建置不啟用紀錄功能。
診斷識別：0.12.1-diag.20261023.3-a9767ab。控制台標題應顯示「第三版」。
這版補記雙 Ctrl 手勢是否成立、讀取選取內容、位置、延後顯示及建立浮層的結果；不保存選取的文字。
第二版實機已確認 AyuGram 的雙 Ctrl 成立，但未能出現浮層。這次只需定位後續退出階段，不必重測 Shift／123。
第二版無法記錄這些退出原因，請安裝本版並確認實際載入後再補測。舊報告保留。

開始前
1. 先把整包解壓縮，保留另外提供的「Shanjie-Windows-0.12.1-Before-Diagnostics.zip」作為原安裝版本的還原包。
2. 存好工作，執行「安裝善解.exe」。這會按一般 0.12.1 更新流程要求 UAC、註冊診斷 DLL，並依既有安裝程式清理舊版本；控制台本身不會安裝或變更系統設定。
3. 存檔後關閉並重新開啟 AyuGram／Excel。執行「診斷控制台.exe」，按「檢查診斷版與載入狀態」。若 explorer.exe 仍載入舊 DLL，請存好工作後登出、重新登入，再測 Win+R。不要強制結束系統程式。

加詞浮層測試（不必錄影，四個程式可在同一次紀錄完成）
1. 先在目標程式準備已提交的兩到四個中文字；Win+R 不要執行命令，AyuGram 不要送出訊息。Excel 用空白測試活頁簿，按 F2 進入編輯後反白文字，不是只選取儲存格。
2. 控制台按「開始：加詞浮層測試」。
3. 到 AyuGram 反白已提交的中文字，放開其他按鍵後連按兩下左 Ctrl，速度照平常觸發加詞。可重試一次，兩組間隔一秒。
4. 依序在 Win+R、Excel 編輯欄位重複；最後到記事本做正常對照。
5. 有浮層時按 Esc 取消，不按「加入」或 Enter，不保存詞條。回控制台按「停止紀錄」。
6. 告訴 agent 各程式是否出現浮層；無須提供聊天內容或詞條全文。

原本的「右 Shift 測試」及「Excel 首鍵測試」仍保留，本輪不用執行。

結果與隱私
- 每次最多 5 分鐘；手動停止、關閉控制台或逾時，都會停止收集。若控制台意外結束，DLL 也會在原期限停止。
- 「加詞浮層」只記錄 AyuGram.exe／explorer.exe／EXCEL.EXE／notepad.exe；「右 Shift」及「Excel」維持原本白名單。請只操作測試欄位。
- 按鍵名稱只辨識 Shift／Control／Alt／Win、PROCESS_KEY（輸入法處理代理訊息）、1／2／3、方向鍵、F2、Enter／Escape／Tab／Backspace；其餘按鍵統一標為 OTHER，不記錄掃描碼數值或字元。只補記是否有掃描碼、按下／放開及修飾鍵狀態。
- 診斷不額外讀取聊天／工作表全文；原本加詞功能仍會讀取反白文字供浮層使用，但紀錄只保存長度與位置，不保存文字、候選詞、剪貼簿、視窗標題、個人詞庫或學習資料。另保存程序名稱、PID、DLL 路徑與雜湊、手勢成立及各階段結果碼。
- 每個程序的各輸入執行緒最多保存 2 MiB。紀錄只留本機，不會上傳。
- 按「開啟紀錄資料夾」，可看到每次測試的獨立資料夾，內有 session.json、events-*.jsonl、summary.json。若事件數為零，請先檢查是否仍載入舊 DLL；不要把零事件當作測試通過。
- 同一台電腦交給 agent 分析時，只需說已測完；其他電腦只提供該次 reports 子資料夾，不要提供整個 Shanjie 個人資料目錄。
- 位置：%LOCALAPPDATA%\Shanjie\InputDiagnostics\reports\<測試識別碼>

還原
停止紀錄並關閉控制台，解壓縮「Shanjie-Windows-0.12.1-Before-Diagnostics.zip」，執行其中的「安裝善解.exe」覆蓋安裝，再重新開啟測試程式；需要時登出再登入。這是換回原安裝版本的公開程式檔案，不需要解除安裝或移除個人資料。
詞庫、學習與已下載模型沿用一般更新流程，不包含在此包與還原包內。
'@
Set-Content "$diagnostic/請先閱讀-診斷使用方式.txt" $readme -Encoding utf8BOM
Get-ChildItem $diagnostic -Recurse -File|Where-Object Name -ne 'SHA256SUMS.txt'|Sort-Object FullName|ForEach-Object{
 '{0}  {1}' -f (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant(),[IO.Path]::GetRelativePath($diagnostic,$_.FullName)
}|Set-Content "$diagnostic/SHA256SUMS.txt" -Encoding utf8NoBOM
& "$diagnostic/tools/shanjie-manager.exe" verify-package $diagnostic
if($LASTEXITCODE){throw 'Diagnostic package failed verification.'}
Add-Type -AssemblyName System.IO.Compression.FileSystem
foreach($item in @(@($diagnostic,'Shanjie-Windows-0.12.1-Diagnostics.zip'),@($original,'Shanjie-Windows-0.12.1-Before-Diagnostics.zip'))){
 $zip=Join-Path $OutputRoot $item[1]
 [IO.Compression.ZipFile]::CreateFromDirectory($item[0],$zip,[IO.Compression.CompressionLevel]::Optimal,$false)
 $digest=(Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
 Set-Content "$zip.sha256" "$digest  $($item[1])" -Encoding ascii
}
Write-Output $OutputRoot
