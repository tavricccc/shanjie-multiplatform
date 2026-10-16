# Public-project outputs may be replaced; personal data is never passed here.
function Preserve-Output([string]$Path) {
    if (!(Test-Path -LiteralPath $Path)) { return }
    $project = [IO.Path]::GetFullPath((Split-Path $PSScriptRoot -Parent))
    $resolved = [IO.Path]::GetFullPath($Path)
    if (!$resolved.StartsWith($project + [IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)) { throw 'Output cleanup must stay inside the project.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
