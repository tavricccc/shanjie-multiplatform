# Build outputs are recoverable instead of permanently removed.
function Preserve-Output([string]$Path) {
    if (!(Test-Path -LiteralPath $Path)) { return }
    $item = Get-Item -LiteralPath $Path
    $parent = Split-Path $item.FullName -Parent
    $archive = Join-Path $parent '.deleted_files'
    New-Item -ItemType Directory -Path $archive -Force | Out-Null
    $target = Join-Path $archive ((Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [guid]::NewGuid().ToString('N').Substring(0,8) + '-' + $item.Name)
    if (!( [IO.Path]::GetFullPath($target).StartsWith([IO.Path]::GetFullPath($archive) + [IO.Path]::DirectorySeparatorChar))) { throw 'Unexpected archive path.' }
    Move-Item -LiteralPath $item.FullName -Destination $target
}
