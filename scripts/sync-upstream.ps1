#Requires -Version 7
# Keep the pinned checkout intact; only network failures switch to the mirror.
param([string]$ProjectRoot = (Split-Path $PSScriptRoot -Parent))
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath($ProjectRoot)
$pin = Get-Content -LiteralPath "$root/UPSTREAM.json" -Raw | ConvertFrom-Json
$checkout = Join-Path $root 'upstream/shanjie'
if ((Test-Path -LiteralPath "$checkout/.git") -and (git -C $checkout status --porcelain)) {
    throw 'Upstream checkout must be clean before synchronization.'
}
foreach ($repository in @('https://github.com/Nanako0129/shanjie', 'https://github.com/tavricccc/shanjie-upstream-backup')) {
    if (!(Test-Path -LiteralPath "$checkout/.git")) {
        git -C $root -c "submodule.upstream/shanjie.url=$repository.git" submodule update --init --no-fetch -- upstream/shanjie
        if (!(Test-Path -LiteralPath "$checkout/.git")) { continue }
    }
    git -C $checkout cat-file -e "$($pin.commit)^{commit}" 2>$null
    if ($LASTEXITCODE) {
        git -C $checkout fetch --no-tags "$repository.git" $pin.commit
        if ($LASTEXITCODE) { continue }
    }
    git -C $checkout checkout --detach $pin.commit
    if ($LASTEXITCODE) { throw 'Could not check out the pinned upstream commit.' }
    return
}
throw 'Neither upstream nor the mirror could provide the pinned commit.'
