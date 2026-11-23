#Requires -Version 7
# Real upstream CLI and prepared shipping core, sharing the pinned judge and inputs.
param([string]$ResultRoot)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
. "$PSScriptRoot/export-upstream.ps1"
Import-Module "$PSScriptRoot/test-support.psm1"
. "$PSScriptRoot/preserve-output.ps1"
$elapsed=[Diagnostics.Stopwatch]::StartNew();$phases=[ordered]@{}
if(!$ResultRoot){$ResultRoot=Join-Path $root ('build/rowstats-'+[guid]::NewGuid().ToString('N'))}
New-Item -ItemType Directory -Force $ResultRoot | Out-Null
$pin=(Get-Content "$root/UPSTREAM.json" -Raw | ConvertFrom-Json).commit
$manifest=Get-Content "$root/MODEL-SOURCES.json" -Raw | ConvertFrom-Json
$models=@{}
foreach($asset in $manifest.builtin.assets){
    $file=Join-Path $root "build/model/$($asset.release)-$($asset.filename)"
    if(!(Test-Path -LiteralPath $file) -or (Get-FileHash -LiteralPath $file).Hash.ToLowerInvariant() -ne $asset.sha256){throw "Prepare and validate model first: $($asset.filename)"}
    $models[$asset.filename]=@{path=$file;sha256=$asset.sha256}
}
$phases.model_validation_seconds=$elapsed.Elapsed.TotalSeconds
$executables=@{};$sources=@{}
# MSVC's linker still has path-length limits. Keep source/target trees short;
# retained logs and first differences remain in the CTest result directory.
foreach($arm in 'upstream','overlay'){
    $inputs=@('UPSTREAM.json','MODEL-SOURCES.json','scripts/verify-rowstats.ps1','scripts/export-upstream.ps1')
    if($arm -eq 'overlay'){$inputs+=@('build/core/src','build/core/Cargo.toml')}
    $key=Get-ContentFingerprint -Root $root -Paths $inputs -Values @($arm)
    $tree=Join-Path $root "build/rowstats-$arm-$($key.Substring(0,12))"
    $sourcePaths=@('core','cli','data','eval','Cargo.toml','Cargo.lock')
    $sourceWatch=[Diagnostics.Stopwatch]::StartNew()
    $sourceHit=Test-ContentCache -Root $tree -ManifestPath "$tree/.source-cache.json" -Key $key -Paths $sourcePaths
    if(!$sourceHit){
    if(Test-Path -LiteralPath $tree){Preserve-Output $tree}
    Export-Upstream $tree @('core','cli','Cargo.lock','data','eval')
    # Keep workspace-only host/MSI crates outside this isolated CLI build.
    "[workspace]`nmembers = [`"core`", `"cli`"]`nresolver = `"2`"`n[profile.release]`ndebug = false" | Set-Content "$tree/Cargo.toml" -Encoding utf8NoBOM
    # Integration tests also build every binary target. The unrelated replay CLI
    # uses Unix-only file APIs; this fixture tests eval and its unchanged judges.
    $cliToml=(Get-Content "$tree/cli/Cargo.toml" -Raw).Replace('[package]',"[package]`nautobins = false") -replace '(?ms)^\[\[bin\]\]\s*name = "shanjie-replay"\s*path = "src/bin/replay.rs"\s*',''
    Set-Content "$tree/cli/Cargo.toml" $cliToml -Encoding utf8NoBOM
    if($arm -eq 'overlay'){
        Get-ChildItem "$root/build/core/src" | Copy-Item -Destination "$tree/core/src" -Recurse -Force
        Copy-Item "$root/build/core/Cargo.toml" "$tree/core/Cargo.toml" -Force
        # Prepared core is standalone for product Cargo; join this test workspace.
        $toml=(Get-Content "$tree/core/Cargo.toml" -Raw) -replace '(?m)^\[workspace\]\s*',''
        Set-Content "$tree/core/Cargo.toml" $toml -Encoding utf8NoBOM
    }
    New-Item -ItemType Directory -Force "$tree/data/lm" | Out-Null
    foreach($name in $models.Keys){Copy-Item -LiteralPath $models[$name].path -Destination "$tree/data/lm/$name" -Force}
    }
    $sourceSeconds=$sourceWatch.Elapsed.TotalSeconds
    $buildWatch=[Diagnostics.Stopwatch]::StartNew()
    & cargo test --release --manifest-path "$tree/Cargo.toml" -p cli --test judge_v2 --test golden --no-run *>&1 | Tee-Object "$ResultRoot/build-$arm.log" | Write-Host
    if($LASTEXITCODE){throw "CLI build failed: $arm"}
    $buildSeconds=$buildWatch.Elapsed.TotalSeconds
    $goldenWatch=[Diagnostics.Stopwatch]::StartNew()
    & cargo test --release --manifest-path "$tree/Cargo.toml" -p cli --test judge_v2 --test golden -- --test-threads=1 *>&1 | Tee-Object "$ResultRoot/golden-$arm.log" | Write-Host
    if($LASTEXITCODE){throw "Pinned CLI judge/golden regression failed: $arm"}
    $phases[$arm]=@{source_cache_hit=$sourceHit;source_prepare_seconds=$sourceSeconds;compile_wall_seconds=$buildSeconds;judge_golden_wall_seconds=$goldenWatch.Elapsed.TotalSeconds}
    Save-ContentCache -Root $tree -ManifestPath "$tree/.source-cache.json" -Key $key -Paths $sourcePaths
    $sources[$arm]=Get-ContentFingerprint -Root $tree -Paths @('core','cli','Cargo.toml','Cargo.lock')
    $executables[$arm]=@{path="$tree/target/release/shanjie-eval.exe";sha256=(Get-FileHash "$tree/target/release/shanjie-eval.exe").Hash.ToLowerInvariant()}
}
function Compare-RowBytes([string]$Expected,[string]$Actual,[string]$Difference){
    $a=[IO.File]::ReadAllBytes($Expected);$b=[IO.File]::ReadAllBytes($Actual)
    $i=0;while($i -lt [Math]::Min($a.Length,$b.Length) -and $a[$i] -eq $b[$i]){$i++}
    if($i -eq $a.Length -and $i -eq $b.Length){return $true}
    @{offset=$i;expected_length=$a.Length;actual_length=$b.Length;expected=$(if($i -lt $a.Length){$a[$i]}else{$null});actual=$(if($i -lt $b.Length){$b[$i]}else{$null})} |
        ConvertTo-Json | Set-Content -LiteralPath $Difference -Encoding utf8NoBOM
    return $false
}
$runs=@()
$diffWatch=[Diagnostics.Stopwatch]::StartNew()
foreach($packs in 'acg,it','acg,it,beauty,meme'){
    $configuration=if($packs -eq 'acg,it'){'upstream-flags'}else{'fork-shipping-flags'}
    foreach($profile in 'chat','formal'){
        foreach($set in 'dev302','user-typing','user-reported'){
            $label="$configuration-$set-$profile";$outputs=@{}
            foreach($arm in 'upstream','overlay'){
                $tree=Join-Path $workspace $arm
                $out=Join-Path $ResultRoot "$label-$arm.rowstats"
                $arguments=@('--lm','data/lm/bigram.sjlm','--profile',$profile,'--context','--packs',$packs,'--tinylm','data/lm/tinylm.sjtl','--threads','4','--rowstats',$out)
                if($set -eq 'dev302'){$arguments+=@('--dev','302')}else{$arguments+=@('--rows',"eval/dev/$set.txt")}
                if($profile -eq 'chat'){$arguments+=@('--colloquial-core','data/lm/colloquial.sjlm')}
                Push-Location $tree
                $runWatch=[Diagnostics.Stopwatch]::StartNew()
                try {& $executables[$arm].path @arguments *>&1 | Tee-Object "$ResultRoot/$label-$arm.log" | Write-Host;if($LASTEXITCODE){throw "CLI failed: $label/$arm"}}finally{Pop-Location}
                $outputs[$arm]=$out
                $runs+=@{configuration=$configuration;profile=$profile;set=$set;arm=$arm;arguments=$arguments;wall_seconds=$runWatch.Elapsed.TotalSeconds;sha256=(Get-FileHash $out).Hash.ToLowerInvariant()}
            }
            if(!(Compare-RowBytes $outputs.upstream $outputs.overlay "$ResultRoot/$label.first-difference.json")){throw "Rowstats differ: $label (first byte saved)"}
        }
    }
}
$phases.rowstats_wall_seconds=$diffWatch.Elapsed.TotalSeconds
$negativeWatch=[Diagnostics.Stopwatch]::StartNew()
# A one-byte mutation must exercise the same comparator, not just its hash check.
$negative=[IO.File]::ReadAllBytes($outputs.upstream);$negative[0]=$negative[0] -bxor 1
[IO.File]::WriteAllBytes("$ResultRoot/negative.rowstats",$negative)
if(Compare-RowBytes $outputs.upstream "$ResultRoot/negative.rowstats" "$ResultRoot/negative.first-difference.json"){throw 'One-byte negative control was accepted'}
$phases.negative_control_seconds=$negativeWatch.Elapsed.TotalSeconds
$phases.total_wall_seconds=$elapsed.Elapsed.TotalSeconds
@{upstream=$pin;windows=(git -C $root rev-parse HEAD);sources=$sources;executables=$executables;models=$models;architecture='x64';os=[Environment]::OSVersion.VersionString;learning='off';english='not exercised: reading-based CLI';judge='pinned upstream cli/src, unchanged';negative_control='one byte rejected';test_results_reused=$false;phases=$phases;runs=$runs} |
    ConvertTo-Json -Depth 8 | Set-Content "$ResultRoot/result.json" -Encoding utf8NoBOM
Write-Host "PASS 12 byte-identical CLI comparisons, upstream/fork pack configurations and negative control: $ResultRoot"
