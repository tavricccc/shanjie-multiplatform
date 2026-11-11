#Requires -Version 7
param([ValidateSet('x64','x86')][string]$Architecture='x64')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
$target=if($Architecture -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
$cacheCase='mixed_regression::sessions_on_one_model_share_the_digit_tables'
# This assertion inspects process-wide cache identity. Other tests use enough
# lexicons to evict its entries while it opens the two sessions.
foreach($phase in 'input','cache-identity') {
    $arguments=if($phase -eq 'input'){@('--ignored','mixed_regression','--skip',$cacheCase)}else{@('--ignored','--exact',$cacheCase)}
    Invoke-Timed -Name "Mixed-input regression $phase ($Architecture)" -Category 'rust.mixed-regression' -Details @{target=$target;phase=$phase} -Action {
        & cargo test --manifest-path "$root/windows/Cargo.toml" --lib --target $target -- @arguments
        if($LASTEXITCODE){throw "Mixed-input regression $phase failed for $Architecture."}
    }
}
