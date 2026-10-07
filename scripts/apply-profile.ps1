#Requires -Version 7
param([Parameter(Mandatory)][string]$ProfileFile)
$ErrorActionPreference = 'Stop'
$source = (Resolve-Path -LiteralPath $ProfileFile).Path
$target = Join-Path $env:LOCALAPPDATA 'Shanjie'
New-Item -ItemType Directory -Path $target -Force | Out-Null
$destination = Join-Path $target 'profile.ini'
$content = [IO.File]::ReadAllText($source)
# A preset changes interaction settings, while retaining the user's saved dictionary.
if (Test-Path -LiteralPath $destination) {
    $previous = [IO.File]::ReadAllText($destination)
    $inputSection = [regex]::Match($previous, '(?ms)^\[input\]\r?\n(.*?)(?=^\[|\z)').Groups[1].Value
    $dictionary = [regex]::Match($inputSection, '(?m)^dictionary=(.+)\r?$').Groups[1].Value.Trim()
    $newInput = [regex]::Match($content, '(?ms)^\[input\]\r?\n(.*?)(?=^\[|\z)').Groups[1].Value
    if ($dictionary -and $newInput -notmatch '(?m)^dictionary=') {
        $content = $content.Replace('[input]', "[input]`ndictionary=$dictionary")
    }
}
[IO.File]::WriteAllText("$destination.tmp", $content, [Text.UTF8Encoding]::new($false))
[IO.File]::Move("$destination.tmp", $destination, $true)
Write-Host "已套用：$source"
Write-Host '切換到其他輸入法，再切回善解以載入設定。已開啟的應用程式若仍使用舊設定，請重新開啟該程式。'
