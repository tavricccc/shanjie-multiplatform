#Requires -Version 7
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$manifest = [IO.File]::ReadAllText((Join-Path $root 'MODEL-SOURCES.json')) | ConvertFrom-Json
New-Item -ItemType Directory -Path "$root/build/model","$root/dist/data" -Force | Out-Null
foreach ($model in $manifest.models) {
    $source = Join-Path $root "build/model/$($model.source)"
    if (!(Test-Path -LiteralPath $source) -or (Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant() -ne $model.sha256) {
        curl.exe --fail --location --silent --show-error --continue-at - --connect-timeout 20 --speed-time 30 --speed-limit 1024 --max-time 240 --retry 3 --retry-delay 2 --retry-all-errors --retry-max-time 300 --output "$source.download" "$($manifest.repository)/releases/download/$($model.release)/bigram.sjlm"
        if ($LASTEXITCODE) { throw "Model download failed: $($model.release)" }
        if ((Get-FileHash -LiteralPath "$source.download").Hash.ToLowerInvariant() -ne $model.sha256) { throw "Model SHA-256 mismatch: $($model.release)" }
        Move-Item -LiteralPath "$source.download" -Destination $source -Force
    }
    cargo run --release --manifest-path "$root/build/core/Cargo.toml" --bin map-model -- $source "$root/dist/data/$($model.mapped)"
    if ($LASTEXITCODE) { throw "Mapped model equivalence verification failed: $($model.release)" }
}
