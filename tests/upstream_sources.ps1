#Requires -Version 7
param([Parameter(Mandatory)][string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$git=(Get-Command git -CommandType Application | Select-Object -First 1).Source
$null=New-Item -ItemType Directory -Path $ResultRoot -Force
$seed=Join-Path $ResultRoot 'source.git'
& $git clone --bare --shared "$root/upstream/shanjie" $seed *> (Join-Path $ResultRoot 'seed.log')
if($LASTEXITCODE){throw 'Could not prepare the local upstream fixture.'}
$commit=(git -C "$root/upstream/shanjie" rev-parse HEAD).Trim()
$sourceBase=([uri]$seed).AbsoluteUri -replace '\.git$',''
foreach($scenario in 'official','mirror','unavailable') {
    $fixture=Join-Path $ResultRoot $scenario
    $null=New-Item -ItemType Directory -Path "$fixture/scripts" -Force
    Copy-Item "$root/scripts/sync-upstream.ps1" "$fixture/scripts/sync-upstream.ps1"
    @{repository='https://github.com/Nanako0129/shanjie';commit=$commit}|ConvertTo-Json|Set-Content "$fixture/UPSTREAM.json"
    "[submodule `"upstream/shanjie`"]`n`tpath = upstream/shanjie`n`turl = https://github.com/Nanako0129/shanjie.git"|Set-Content "$fixture/.gitmodules"
    git -C $fixture init --quiet
    git -C $fixture add .gitmodules UPSTREAM.json
    git -C $fixture update-index --add --cacheinfo "160000,$commit,upstream/shanjie"
    if($LASTEXITCODE){throw 'Could not prepare the fixture gitlink.'}
    $official=if($scenario -eq 'official'){$sourceBase}else{([uri](Join-Path $fixture 'missing-official')).AbsoluteUri}
    $mirror=if($scenario -eq 'mirror'){$sourceBase}else{([uri](Join-Path $fixture 'missing-mirror')).AbsoluteUri}
    $start=[Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
    $start.UseShellExecute=$false;$start.CreateNoWindow=$true
    $start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
    foreach($argument in @('-NoProfile','-File',"$fixture/scripts/sync-upstream.ps1")){$start.ArgumentList.Add($argument)}
    # Child-only URL rewrites keep all three cases offline and do not change user git configuration.
    $start.Environment['GIT_CONFIG_COUNT']='3'
    $start.Environment['GIT_CONFIG_KEY_0']='protocol.file.allow';$start.Environment['GIT_CONFIG_VALUE_0']='always'
    $start.Environment['GIT_CONFIG_KEY_1']="url.$official.insteadOf";$start.Environment['GIT_CONFIG_VALUE_1']='https://github.com/Nanako0129/shanjie'
    $start.Environment['GIT_CONFIG_KEY_2']="url.$mirror.insteadOf";$start.Environment['GIT_CONFIG_VALUE_2']='https://github.com/tavricccc/shanjie-upstream-backup'
    $process=[Diagnostics.Process]::Start($start)
    $stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $output=$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText((Join-Path $fixture 'result.log'),$output)
    $expectedSuccess=$scenario -ne 'unavailable'
    if(($process.ExitCode -eq 0) -ne $expectedSuccess){throw "Wrong upstream outcome for $scenario`: $output"}
    if($expectedSuccess -and (git -C "$fixture/upstream/shanjie" rev-parse HEAD).Trim() -ne $commit){throw 'Fallback changed the pinned commit.'}
    $process.Dispose()
    Write-Host "PASS upstream checkout: $scenario"
}
