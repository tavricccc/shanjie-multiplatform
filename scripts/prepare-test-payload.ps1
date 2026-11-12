#Requires -Version 7
param([Parameter(Mandatory)][ValidateSet('x64','x86')][string]$Architecture,[Parameter(Mandatory)][string]$PayloadRoot,[string]$RuntimeRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(!$RuntimeRoot){$RuntimeRoot=Join-Path $root 'dist'}
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$null=Initialize-TestTimings -ProjectRoot $root -Name 'test-payload'
$buildRoot=[IO.Path]::GetFullPath((Join-Path $root 'build/cmake'))
$payload=[IO.Path]::GetFullPath($PayloadRoot)
if (!$payload.StartsWith($buildRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) {
    throw "Refusing to prepare a fixture outside build/cmake: $payload"
}
$dll=Join-Path $RuntimeRoot "$Architecture/Shanjie.dll"
foreach($path in @($dll,(Join-Path $RuntimeRoot 'tools/shanjie-core.exe'),(Join-Path $RuntimeRoot 'data'))) {
    if (!(Test-Path -LiteralPath $path)) { throw "Runtime payload input is missing: $path" }
}
Invoke-Timed -Name "Prepare isolated CTest runtime payload ($Architecture)" -Category 'test.prepare.payload' -Details @{architecture=$Architecture;root=$payload} -Action {
    $null=New-Item -ItemType Directory -Path $payload -Force
    $archDir=Join-Path $payload $Architecture
    $null=New-Item -ItemType Directory -Path $archDir -Force
    $destination=Join-Path $archDir 'Shanjie.dll'
    $sourceHash=(Get-FileHash -LiteralPath $dll -Algorithm SHA256).Hash.ToLowerInvariant()
    $cacheHit=$false
    if(Test-Path -LiteralPath $destination){
        $destinationItem=Get-Item -LiteralPath $destination
        if($destinationItem.Length -eq (Get-Item -LiteralPath $dll).Length){$cacheHit=(Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant() -eq $sourceHash}
    }
    if(!$cacheHit){Copy-Item -LiteralPath $dll -Destination $destination -Force}
    Copy-TestTools -Source "$RuntimeRoot/tools" -Destination "$payload/tools"
    foreach($name in 'data') {
        $link=Join-Path $payload $name
        $target=[IO.Path]::GetFullPath((Join-Path $RuntimeRoot $name))
        $item=Get-Item -LiteralPath $link -Force -ErrorAction SilentlyContinue
        if ($item) {
            if (!($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or [IO.Path]::GetFullPath([string]$item.Target) -ne $target) {
                throw "Refusing to replace a non-fixture or unexpected payload path: $link"
            }
        } else { New-Item -ItemType Junction -Path $link -Target $target | Out-Null }
    }
    $installed=Join-Path $payload 'installed.json'
    if (!(Test-Path -LiteralPath $installed) -or (Get-Content -LiteralPath $installed -Raw).Trim() -ne '{"version":"test","keyboards":3}') {
        Set-Content -LiteralPath $installed '{"version":"test","keyboards":3}' -Encoding utf8NoBOM
        $cacheHit=$false
    }
    Write-Host "Runtime payload DLL cache $(if($cacheHit){'hit'}else{'miss'}): $payload/$Architecture."
}
Write-Host "Isolated CTest payload ready: $payload (read-only data junction and private tool copies)."
