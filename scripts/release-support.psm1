Import-Module "$PSScriptRoot/test-support.psm1" -ErrorAction Stop
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop
. "$PSScriptRoot/preserve-output.ps1"

function Get-ReleaseIdentity([string]$ProjectRoot) {
    $version=(Get-Content -LiteralPath "$ProjectRoot/RELEASE-CHANNEL.json" -Raw|ConvertFrom-Json).version
    $cargo=[regex]::Match([IO.File]::ReadAllText("$ProjectRoot/windows/Cargo.toml"),'(?m)^version = "([^"]+)"').Groups[1].Value
    $lock=[regex]::Match([IO.File]::ReadAllText("$ProjectRoot/windows/Cargo.lock"),'name = "shanjie-windows"\s+version = "([^"]+)"').Groups[1].Value
    [xml]$ui=Get-Content -LiteralPath "$ProjectRoot/ui/Shanjie.UI.csproj" -Raw
    if(!$version -or $cargo -ne $version -or $lock -ne $version -or $ui.Project.PropertyGroup.Version -ne $version){throw 'Release versions in Cargo, Cargo.lock, WinUI and RELEASE-CHANNEL.json must match.'}
    $commit=& git -C $ProjectRoot rev-parse --verify HEAD
    if($LASTEXITCODE){throw 'Cannot identify the release commit.'}
    $dirty=@(& git -C $ProjectRoot status --porcelain --untracked-files=normal)
    if($LASTEXITCODE){throw 'Cannot check the release worktree.'}
    [pscustomobject]@{commit=$commit;version=$version;clean=(!$dirty.Count)}
}

function Get-ReleaseArtifacts([string]$ProjectRoot,[string]$CandidateId,[object[]]$CtestReports) {
    if($CandidateId -notmatch '^[0-9a-f]{32}$'){throw 'Invalid release candidate identifier.'}
    $candidate="build/release-candidates/$CandidateId"
    $paths=@('dist/x64/Shanjie.dll','dist/x86/Shanjie.dll','dist/tools','dist/ui','dist/data','dist/LICENSES','dist/setup',"$candidate/Shanjie-Windows-Setup.exe","$candidate/candidate.json")
    foreach($report in $CtestReports){$paths += [IO.Path]::GetRelativePath($ProjectRoot,$report.junit)}
    Get-RelativeFileEntries -Root $ProjectRoot -Paths $paths -RequirePaths
}

function Assert-ReleaseAcceptance([string]$ProjectRoot,[string]$ReportPath,[switch]$CiArtifact) {
    $ProjectRoot=[IO.Path]::GetFullPath($ProjectRoot)
    $ReportPath=[IO.Path]::GetFullPath($ReportPath)
    $reportsRoot=[IO.Path]::GetFullPath((Join-Path $ProjectRoot 'build/reports'))+[IO.Path]::DirectorySeparatorChar
    if(!$ReportPath.StartsWith($reportsRoot,[StringComparison]::OrdinalIgnoreCase)){throw 'Release receipt must belong to this worktree under build/reports.'}
    $record=Get-Content -LiteralPath $ReportPath -Raw|ConvertFrom-Json -AsHashtable
    $identity=Get-ReleaseIdentity $ProjectRoot
    if(!$identity.clean){throw 'Formal release requires a clean Git worktree.'}
    if($record.schema -ne 3 -or $record.level -ne 'RELEASE' -or $record.status -ne 'passed' -or !$record.release){throw 'A completed RELEASE receipt is required; status alone is insufficient.'}
    if(!$CiArtifact -and $record.ci_safe){throw 'CI-safe or skipped interactive acceptance cannot authorize a local formal release.'}
    foreach($source in @($record.release.source_start,$record.release.source_end)){
        if(!$source.clean -or $source.commit -ne $identity.commit -or $source.version -ne $identity.version){throw 'Release receipt is dirty, stale, or belongs to another commit/version.'}
    }
    if($record.commit -ne $identity.commit -or $record.version -ne $identity.version -or $record.worktree_dirty){throw 'Release receipt source identity does not match HEAD.'}
    if(@(Compare-Object @('x64','x86') @($record.release.architectures)).Count){throw 'Both x64 and x86 release acceptance are required.'}

    $expected=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $caseRows=@()
    foreach($preset in 'windows-x64','windows-x86'){
        $inventoryText=& ctest --test-dir (Join-Path $ProjectRoot "build/cmake/$preset") -C Release --show-only=json-v1
        if($LASTEXITCODE){throw "Cannot verify the current $preset release test inventory."}
        $inventory=($inventoryText|Out-String)|ConvertFrom-Json
        foreach($test in $inventory.tests){
            $labels=@(($test.properties|Where-Object name -eq 'LABELS').value|ForEach-Object{$_ -split ';'})
            $required=if($preset -eq 'windows-x64'){'full' -in $labels -or 'release' -in $labels}else{'release' -in $labels}
            if($CiArtifact -and $record.ci_safe -and 'ui' -in $labels -and 'ci-safe' -notin $labels){$required=$false}
            if($required){
                [void]$expected.Add("${preset}::$($test.name)")
                $caseRows += [pscustomobject]@{name=$test.name;preset=$preset;selected=$true;reason='Required release acceptance'}
            }
        }
    }
    if(!$expected.Count){throw 'Current RELEASE inventory is empty.'}
    $receiptDirectory=(Split-Path $ReportPath -Parent)+[IO.Path]::DirectorySeparatorChar
    foreach($testReport in $record.ctest){
        $junit=[IO.Path]::GetFullPath($testReport.junit)
        if(!$junit.StartsWith($receiptDirectory,[StringComparison]::OrdinalIgnoreCase)){throw 'JUnit files must belong to the release receipt directory.'}
    }
    $actual=@(Read-CtestResults $record.ctest)
    foreach($list in @(@{name='JUnit';rows=$actual},@{name='JSON results';rows=@($record.results)},@{name='selection';rows=@($record.selection.selected)})){
        $keys=@($list.rows|ForEach-Object{"$($_.preset)::$($_.name)"})
        if($keys.Count -ne $expected.Count -or !$expected.SetEquals([string[]]$keys)){throw "$($list.name) does not contain every current required RELEASE test exactly once."}
    }
    if(@($actual|Where-Object{$_.status -ne 'passed' -or !$_.attempts}).Count -or @($record.results|Where-Object status -ne 'passed').Count){throw 'Every required RELEASE test must have an executed, passing JUnit result.'}
    $planned=@(Get-HarnessCasePlan $caseRows)
    $observed=@($record.subcases|Where-Object selected)
    if($observed.Count -ne $planned.Count){throw 'Release harness combination coverage is incomplete.'}
    foreach($case in $planned){
        $match=@($observed|Where-Object{$_.name -ceq $case.name -and $_.preset -ceq $case.preset})
        if($match.Count -ne 1 -or $match[0].status -ne 'passed' -or !$match[0].attempts){throw "Release combination did not execute and pass: $($case.name)"}
        foreach($key in $case.parameters.Keys){if($match[0].parameters[$key] -cne $case.parameters[$key]){throw "Release combination parameters differ: $($case.name)/$key"}}
    }

    $candidateId=$record.release.candidate_id
    $current=Get-ReleaseArtifacts $ProjectRoot $candidateId $record.ctest
    $saved=$record.release.artifacts
    if(!$saved -or $saved.Count -ne $current.Count){throw 'Release build artifacts differ from the verified set.'}
    foreach($entry in $current.GetEnumerator()){
        if(!$saved.Contains($entry.Key) -or $saved[$entry.Key] -cne $entry.Value){throw "Release artifact changed after acceptance: $($entry.Key)"}
    }
    $candidateRoot=Join-Path $ProjectRoot "build/release-candidates/$candidateId"
    $candidate=Get-Content -LiteralPath (Join-Path $candidateRoot 'candidate.json') -Raw|ConvertFrom-Json
    if(!$candidate.integrity_verified -or $candidate.source.commit -ne $identity.commit -or $candidate.source.version -ne $identity.version -or !$candidate.source.clean){throw 'Candidate package was not integrity-verified from the accepted clean source.'}
    $installer=Join-Path $candidateRoot 'Shanjie-Windows-Setup.exe'
    $key=[IO.Path]::GetRelativePath($ProjectRoot,$installer).Replace('\','/')
    $candidateHash=($current.GetEnumerator()|Where-Object{ $_.Key.Replace('\','/') -eq $key }|Select-Object -First 1).Value
    if($candidate.installer_sha256 -cne $candidateHash){throw 'Final candidate installer hash differs from its verified metadata.'}
    [pscustomobject]@{identity=$identity;report_path=$ReportPath;report_sha256=(Get-FileHash -LiteralPath $ReportPath).Hash.ToLowerInvariant();candidate_id=$candidateId;installer=$installer;sha256=$candidateHash;local_release_verified=(!$CiArtifact)}
}

function Complete-ReleaseArtifact([string]$ProjectRoot,[string]$ReportPath,[switch]$CiArtifact) {
    if($CiArtifact -and $env:GITHUB_ACTIONS -ne 'true'){throw 'CI artifact export is available only inside GitHub Actions; it does not authorize a local formal release.'}
    $accepted=$null
    if($ReportPath){$accepted=Assert-ReleaseAcceptance -ProjectRoot $ProjectRoot -ReportPath $ReportPath -CiArtifact:$CiArtifact}
    else{
        $receipts=@(Get-ChildItem -LiteralPath (Join-Path $ProjectRoot 'build/reports') -Directory -Filter 'release-*' -ErrorAction SilentlyContinue | Sort-Object LastWriteTimeUtc -Descending)
        foreach($directory in $receipts){
            $path=Join-Path $directory.FullName 'report.json'
            if(!(Test-Path -LiteralPath $path)){continue}
            try{$accepted=Assert-ReleaseAcceptance -ProjectRoot $ProjectRoot -ReportPath $path -CiArtifact:$CiArtifact;break}
            catch{Write-Verbose "Receipt cannot be reused: $($_.Exception.Message)"}
        }
        if(!$accepted){return}
    }
    # Validate the external receipt once, then promote that exact candidate.
    # Copy errors stay outside receipt discovery and never trigger another RELEASE.
    $setup=Join-Path $ProjectRoot 'dist/Shanjie-Windows-Setup.exe'
    Preserve-Output $setup
    Copy-Item -LiteralPath $accepted.installer -Destination $setup -Force
    if((Get-FileHash -LiteralPath $setup).Hash.ToLowerInvariant() -cne $accepted.sha256){throw 'Promoted installer does not match the verified candidate.'}
    Set-Content -LiteralPath "$setup.sha256" -Value "$($accepted.sha256)  Shanjie-Windows-Setup.exe" -Encoding utf8NoBOM
    $marker=[ordered]@{schema=1;status=if($accepted.local_release_verified){'verified-local-release'}else{'github-actions-artifact'};local_release_verified=$accepted.local_release_verified;commit=$accepted.identity.commit;version=$accepted.identity.version;architectures=@('x64','x86');installer_sha256=$accepted.sha256;candidate_id=$accepted.candidate_id;report=$accepted.report_path;report_sha256=$accepted.report_sha256;created_utc=[DateTimeOffset]::UtcNow.ToString('O')}
    $markerPath=Join-Path $ProjectRoot 'dist/release.json'
    [IO.File]::WriteAllText("$markerPath.tmp",($marker|ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
    Move-Item -LiteralPath "$markerPath.tmp" -Destination $markerPath -Force
    Write-Host "Verified artifact: $setup ($($marker.status))"
    return $accepted
}

Export-ModuleMember -Function Get-ReleaseIdentity,Get-ReleaseArtifacts,Assert-ReleaseAcceptance,Complete-ReleaseArtifact
