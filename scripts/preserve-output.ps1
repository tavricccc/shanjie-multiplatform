# Replaced build outputs remain recoverable; personal data is never passed here.
function Preserve-Output([string]$Path) {
    if (!(Test-Path -LiteralPath $Path)) { return }
    $project = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
    $resolved = [IO.Path]::GetFullPath($Path)
    if (!$resolved.StartsWith($project + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Output cleanup must stay inside the project.' }
    $parent = Split-Path $resolved -Parent
    $archive = Join-Path $parent '.deleted_files'
    if ((Get-Item -LiteralPath $resolved -Force).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Refuse to archive a linked output.' }
    New-Item -ItemType Directory -Path $archive -Force | Out-Null
    $destination = Join-Path $archive ("{0}-{1}-{2}" -f (Split-Path $resolved -Leaf),(Get-Date -Format 'yyyyMMdd-HHmmss'),[guid]::NewGuid().ToString('N').Substring(0,8))
    Move-Item -LiteralPath $resolved -Destination $destination
    Write-Host "Previous output moved to $destination"
}
