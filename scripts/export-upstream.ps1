# Export committed bytes, independent of Git's Windows checkout newline conversion.
function Export-Upstream([string]$Destination, [string[]]$Paths) {
    $project = Split-Path $PSScriptRoot -Parent
    $upstream = Join-Path $project 'upstream/shanjie'
    $pin = (Get-Content "$project/UPSTREAM.json" -Raw | ConvertFrom-Json).commit
    if ((git -C $upstream rev-parse HEAD) -ne $pin) { throw 'Upstream is not at the pinned commit.' }
    $destinationPath = [IO.Path]::GetFullPath($Destination)
    if (!$destinationPath.StartsWith([IO.Path]::GetFullPath("$project/build/") ,[StringComparison]::OrdinalIgnoreCase)) { throw 'Export must stay under build/.' }
    New-Item -ItemType Directory -Path $destinationPath -Force | Out-Null
    $archive = Join-Path $destinationPath 'upstream.zip'
    & git -c core.autocrlf=false -c core.eol=lf -C $upstream archive --format=zip -o $archive $pin -- @Paths
    if ($LASTEXITCODE) { throw 'Pinned upstream export failed.' }
    Expand-Archive -LiteralPath $archive -DestinationPath $destinationPath -Force
}
