#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$pin = '54a4a07289412efc54134092c0d945f895a71ed3'
$source = Join-Path $root 'build/easy_en.dict.yaml'
Invoke-WebRequest "https://raw.githubusercontent.com/BlindingDark/rime-easy-en/$pin/easy_en.dict.yaml" -OutFile $source
$words = foreach ($line in [IO.File]::ReadLines($source)) {
    if ($line -cmatch '^([a-z]{3,32})\t\1\t(\d+)$' -and [int]$Matches[2] -ge 990000) { $Matches[1] }
}
[IO.File]::WriteAllLines((Join-Path $root 'data/english-common.txt'), @($words | Sort-Object -Unique), [Text.UTF8Encoding]::new($false))
Get-FileHash $source,(Join-Path $root 'data/english-common.txt') -Algorithm SHA256
