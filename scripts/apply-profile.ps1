#Requires -Version 7
param([Parameter(Mandatory)][string]$ProfileFile)
$ErrorActionPreference = 'Stop'
$source = (Resolve-Path -LiteralPath $ProfileFile).Path
$target = Join-Path $env:LOCALAPPDATA 'Shanjie'
New-Item -ItemType Directory -Path $target -Force | Out-Null
Copy-Item -LiteralPath $source -Destination (Join-Path $target 'profile.ini') -Force
Write-Host "已套用：$source"
Write-Host '切換到其他輸入法，再切回善解以載入設定。已開啟的應用程式若仍使用舊設定，請重新開啟該程式。'
