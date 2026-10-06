#Requires -Version 7
param([Parameter(Mandatory)][ValidateSet('install','uninstall')][string]$Action)
$ErrorActionPreference = 'Stop'
$admin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (!$admin) {
    $arguments = '-NoProfile -ExecutionPolicy Bypass -File "{0}" -Action {1}' -f $PSCommandPath, $Action
    $process = Start-Process (Join-Path $PSHOME 'pwsh.exe') -Verb RunAs -ArgumentList $arguments -Wait -PassThru
    exit $process.ExitCode
}
try {
    & (Join-Path $PSScriptRoot "$Action.ps1")
} catch {
    Write-Host $_ -ForegroundColor Red
    Read-Host '按 Enter 關閉'
    exit 1
}
Read-Host '按 Enter 關閉'
