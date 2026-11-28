#Requires -Version 7
param(
    [Parameter(Mandatory)][ValidateSet('Restore','Save')][string]$Operation,
    [Parameter(Mandatory)][ValidateSet('x64','x86')][string]$Architecture,
    [Parameter(Mandatory)][string]$Context,
    [string]$Root=(Split-Path $PSScriptRoot -Parent)
)
$ErrorActionPreference='Stop'
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$watch=[Diagnostics.Stopwatch]::StartNew()
$manifest=Join-Path $Root "build/cargo-inputs/$Architecture.json"
if($Operation -eq 'Restore'){
    if(!(Test-Path -LiteralPath $manifest)){Write-Host '[CARGO INPUT CACHE] MISS: no input metadata';return}
    $saved=Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json -AsHashtable
    if($saved.schema -ne 1 -or $saved.context -cne $Context){Write-Host '[CARGO INPUT CACHE] MISS: compilation context changed';return}
}
$paths=@('windows/src','windows/build.rs','windows/Cargo.toml','windows/Cargo.lock',
    'shared/src','shared/Cargo.toml','platform','build/core/src','build/core/Cargo.toml',
    '.cargo/config.toml','.cargo/windows-release-x64.toml','profiles/windows.ini','profiles/shanjie.ini',
    'ui/Themes/Sj.xaml','assets/mode/chinese.png','assets/mode/english.png','data/model-v6-classes-v4.json')
$inputs=Get-RelativeFileEntries -Root $Root -Paths $paths
if($Operation -eq 'Restore'){
    if($inputs.Count -ne $saved.inputs.Count){Write-Host '[CARGO INPUT CACHE] MISS: input inventory changed';return}
    foreach($entry in $inputs.GetEnumerator()){
        if($saved.inputs[$entry.Key] -cne $entry.Value){Write-Host "[CARGO INPUT CACHE] MISS: input changed: $($entry.Key)";return}
    }
}
$artifacts=Get-RelativeFileEntries -Root $Root -Paths @('windows/target','shared/target',"build/native-$Architecture")
if($Operation -eq 'Restore'){
    if($artifacts.Count -ne $saved.artifacts.Count){throw 'Restored Cargo artifact inventory differs from its content receipt.'}
    foreach($entry in $artifacts.GetEnumerator()){
        if($saved.artifacts[$entry.Key] -cne $entry.Value){throw "Restored Cargo artifact hash differs: $($entry.Key)"}
    }
    foreach($path in $inputs.Keys){
        [IO.File]::SetLastWriteTimeUtc((Join-Path $Root $path),[DateTime]::new([long]$saved.times[$path],[DateTimeKind]::Utc))
    }
    Write-Host "[CARGO INPUT CACHE] HIT: verified $($inputs.Count) inputs and $($artifacts.Count) artifacts; restored source timestamps; $([Math]::Round($watch.Elapsed.TotalSeconds,3))s"
    return
}
$times=[ordered]@{}
foreach($path in $inputs.Keys){$times[$path]=(Get-Item -LiteralPath (Join-Path $Root $path)).LastWriteTimeUtc.Ticks}
$record=[ordered]@{schema=1;context=$Context;inputs=$inputs;artifacts=$artifacts;times=$times}
$null=New-Item -ItemType Directory -Path (Split-Path $manifest -Parent) -Force
[IO.File]::WriteAllText($manifest,($record|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
Write-Host "[CARGO INPUT CACHE] SAVED: $($inputs.Count) inputs and $($artifacts.Count) artifacts; $([Math]::Round($watch.Elapsed.TotalSeconds,3))s"
