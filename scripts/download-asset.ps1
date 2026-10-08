#Requires -Version 7
param([Parameter(Mandatory)][string]$Url, [Parameter(Mandatory)][string]$Destination,
    [Parameter(Mandatory)][long]$Size, [Parameter(Mandatory)][string]$Sha256)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$cache = [IO.Path]::GetFullPath((Join-Path $root 'build/model'))
$destinationPath = [IO.Path]::GetFullPath($Destination)
if ([IO.Path]::GetDirectoryName($destinationPath) -ne $cache) { throw 'Unexpected model cache path.' }
$parts = "$destinationPath.parts"
New-Item -ItemType Directory -Path $parts -Force | Out-Null
$chunkSize = 4MB
$count = [int][Math]::Ceiling($Size / $chunkSize)
# Bounded ranges avoid restarting an 80 MB release download on a slow CDN.
$results = 0..($count-1) | ForEach-Object -Parallel {
    $ErrorActionPreference = 'Stop'; $index = $_
    $start = $index * $using:chunkSize; $end = [Math]::Min($using:Size - 1,$start + $using:chunkSize - 1)
    $part = Join-Path $using:parts "$index.part"
    try {
        if (!(Test-Path -LiteralPath $part) -or (Get-Item -LiteralPath $part).Length -ne $end-$start+1) {
            curl.exe --fail --location --silent --show-error --range "$start-$end" --connect-timeout 20 --max-time 180 --retry 2 --retry-all-errors --output $part "$using:Url"
            if ($LASTEXITCODE) { throw "Download failed: part $index" }
        }
        if ((Get-Item -LiteralPath $part).Length -ne $end-$start+1) { throw "Unexpected asset range length: part $index" }
        [pscustomobject]@{ Index=$index; Error=$null }
    } catch { [pscustomobject]@{ Index=$index; Error=$_.Exception.Message } }
} -ThrottleLimit 4
$failed = @($results | Where-Object Error)
if ($failed.Count) { throw ($failed.Error -join "`n") }
$temporary = "$destinationPath.download"
$output = [IO.File]::Create($temporary)
try {
    for ($index=0; $index -lt $count; $index++) {
        $partStream = [IO.File]::OpenRead((Join-Path $parts "$index.part"))
        try { $partStream.CopyTo($output) } finally { $partStream.Dispose() }
    }
} finally { $output.Dispose() }
# Check the official whole-file hash, not just individual range sizes.
if ((Get-FileHash -LiteralPath $temporary).Hash.ToLowerInvariant() -ne $Sha256) {
    Remove-Item -LiteralPath $parts -Recurse -Force
    throw 'Release asset SHA-256 mismatch.'
}
Move-Item -LiteralPath $temporary -Destination $destinationPath -Force
Remove-Item -LiteralPath $parts -Recurse -Force
