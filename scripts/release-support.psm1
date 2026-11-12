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

# One copy recipe for development packages and frozen RELEASE inputs. Never follow
# links into a user's installation or omit a linked input from its fingerprint.
function Copy-ReleaseFiles([string]$Source,[string]$Destination) {
    $item=Get-Item -LiteralPath $Source -Force -ErrorAction Stop
    if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw "Linked release input is not allowed: $Source"}
    if($item.PSIsContainer){
        $null=New-Item -ItemType Directory -Path $Destination -Force
        foreach($child in Get-ChildItem -LiteralPath $Source -Force){
            if($child.Name -eq '.deleted_files'){continue}
            Copy-ReleaseFiles $child.FullName (Join-Path $Destination $child.Name)
        }
    }else{
        $null=New-Item -ItemType Directory -Path (Split-Path $Destination -Parent) -Force
        Copy-Item -LiteralPath $Source -Destination $Destination -Force
        (Get-Item -LiteralPath $Destination).IsReadOnly=$false
    }
}

function Copy-ReleasePackageInputs([string]$ProjectRoot,[string]$RuntimeRoot,[string]$Destination) {
    foreach($arch in 'x64','x86'){Copy-ReleaseFiles "$RuntimeRoot/$arch/Shanjie.dll" "$Destination/$arch/Shanjie.dll"}
    foreach($name in 'tools','ui','data','LICENSES'){Copy-ReleaseFiles "$RuntimeRoot/$name" "$Destination/$name"}
    Copy-ReleaseFiles "$RuntimeRoot/setup/安裝善解.exe" "$Destination/安裝善解.exe"
    foreach($name in 'UPSTREAM.json','README.md','Changelog.md','profiles','UPSTREAM-CHANGES.md','docs','MODEL-SOURCES.json','MODEL-CATALOG.json','安裝說明.txt'){
        Copy-ReleaseFiles "$ProjectRoot/$name" "$Destination/$name"
    }
    foreach($name in 'install.ps1','uninstall.ps1','install.cmd','uninstall.cmd','profiles.ps1','apply-profile.ps1','apply-windows.cmd','apply-shanjie.cmd','settings.cmd','重新選擇鍵盤.cmd'){
        Copy-ReleaseFiles "$ProjectRoot/scripts/$name" "$Destination/scripts/$name"
    }
}

function Assert-ReleaseFileMap($Expected,$Actual,[string]$Context) {
    if(!$Expected -or $Expected.Count -ne $Actual.Count){throw "$Context file set changed."}
    foreach($entry in $Actual.GetEnumerator()){
        if(!$Expected.ContainsKey($entry.Key) -or $Expected[$entry.Key] -cne $entry.Value){throw "$Context hash mismatch: $($entry.Key)"}
    }
}

function Get-ReleaseSnapshot([string]$ProjectRoot,[string]$SnapshotId,[string]$ManifestHash,[switch]$VerifyFiles) {
    if($SnapshotId -notmatch '^[0-9a-f]{32}$' -or $ManifestHash -notmatch '^[0-9a-f]{64}$'){throw 'A sealed release snapshot identifier and SHA-256 are required.'}
    $path=Join-Path $ProjectRoot "build/release-snapshots/$SnapshotId"
    $manifest=Join-Path $path 'snapshot.json'
    if((Get-FileHash -LiteralPath $manifest).Hash.ToLowerInvariant() -cne $ManifestHash){throw 'Release snapshot manifest changed.'}
    $record=Get-Content -LiteralPath $manifest -Raw|ConvertFrom-Json -AsHashtable
    if($record.schema -ne 1 -or $record.id -cne $SnapshotId){throw 'Invalid release snapshot manifest.'}
    if($VerifyFiles){
        # Copy rejects links; checking again prevents replacement with a junction.
        foreach($item in Get-ChildItem -LiteralPath $path -Recurse -Force){if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw "Linked snapshot input: $($item.FullName)"}}
        $files=Get-RelativeFileEntries -Root $path -Paths @('payload','packaging','native') -RequirePaths
        Assert-ReleaseFileMap $record.files $files 'Release snapshot'
    }
    [pscustomobject]@{id=$SnapshotId;sha256=$ManifestHash;root=$path;runtime_root=(Join-Path $path 'payload');source=$record.source;files=$record.files}
}

function New-ReleaseSnapshot([string]$ProjectRoot,[string]$SnapshotId,$Identity) {
    if($SnapshotId -notmatch '^[0-9a-f]{32}$'){throw 'Invalid release snapshot identifier.'}
    $path=Join-Path $ProjectRoot "build/release-snapshots/$SnapshotId"
    if(Test-Path -LiteralPath $path){throw 'Release snapshots cannot be overwritten or reused for another run.'}
    $current=Get-ReleaseIdentity $ProjectRoot
    if(!$current.clean -or !$Identity.clean -or $current.commit -cne $Identity.commit -or $current.version -cne $Identity.version){throw 'A release snapshot requires the unchanged, clean release source.'}
    $inputs=@('dist/x64/Shanjie.dll','dist/x86/Shanjie.dll','dist/tools','dist/ui','dist/data','dist/LICENSES','dist/setup',
        'UPSTREAM.json','README.md','Changelog.md','profiles','UPSTREAM-CHANGES.md','docs','MODEL-SOURCES.json','MODEL-CATALOG.json','安裝說明.txt')
    $inputs+=@('install.ps1','uninstall.ps1','install.cmd','uninstall.cmd','profiles.ps1','apply-profile.ps1','apply-windows.cmd','apply-shanjie.cmd','settings.cmd','重新選擇鍵盤.cmd'|ForEach-Object{"scripts/$_"})
    foreach($triple in 'x86_64-pc-windows-msvc','i686-pc-windows-msvc'){$inputs+="windows/target/$triple/release/shanjie_windows.lib"}
    $before=Get-RelativeFileEntries -Root $ProjectRoot -Paths $inputs -RequirePaths
    Copy-ReleasePackageInputs $ProjectRoot "$ProjectRoot/dist" "$path/payload"
    Copy-ReleaseFiles "$ProjectRoot/dist/setup" "$path/packaging"
    foreach($arch in 'x64','x86'){
        $triple=if($arch -eq 'x64'){'x86_64-pc-windows-msvc'}else{'i686-pc-windows-msvc'}
        Copy-ReleaseFiles "$ProjectRoot/windows/target/$triple/release/shanjie_windows.lib" "$path/native/$arch/shanjie_windows.lib"
    }
    $after=Get-RelativeFileEntries -Root $ProjectRoot -Paths $inputs -RequirePaths
    Assert-ReleaseFileMap $before $after 'Release inputs during snapshot creation'
    # Also compare every copied file to its original; a concurrent build may have
    # changed an input and changed it back while the snapshot was being copied.
    $files=Get-RelativeFileEntries -Root $path -Paths @('payload','packaging','native') -RequirePaths
    foreach($entry in $files.GetEnumerator()){
        $key=$entry.Key
        $source=if($key -match '^packaging/(.+)$'){"dist/setup/$($Matches[1])"}
            elseif($key -match '^native/(x64|x86)/(.+)$'){"windows/target/$(if($Matches[1] -eq 'x64'){'x86_64'}else{'i686'})-pc-windows-msvc/release/$($Matches[2])"}
            elseif($key -eq 'payload/安裝善解.exe'){'dist/setup/安裝善解.exe'}
            elseif($key -match '^payload/((x64|x86|tools|ui|data|LICENSES)/.+)$'){"dist/$($Matches[1])"}
            else{$key.Substring('payload/'.Length)}
        if(!$before.ContainsKey($source) -or $before[$source] -cne $entry.Value){throw "Snapshot copy differs from build input: $key"}
    }
    $current=Get-ReleaseIdentity $ProjectRoot
    if(!$current.clean -or $current.commit -cne $Identity.commit -or $current.version -cne $Identity.version){throw 'Release source changed while sealing its snapshot.'}
    $record=[ordered]@{schema=1;id=$SnapshotId;source=$Identity;files=$files}
    $manifest=Join-Path $path 'snapshot.json'
    [IO.File]::WriteAllText($manifest,($record|ConvertTo-Json -Depth 8),[Text.UTF8Encoding]::new($false))
    foreach($file in Get-ChildItem -LiteralPath $path -File -Recurse -Force){$file.IsReadOnly=$true}
    Get-ReleaseSnapshot $ProjectRoot $SnapshotId (Get-FileHash -LiteralPath $manifest).Hash.ToLowerInvariant() -VerifyFiles
}

function Assert-ReleasePayload($Snapshot,[string]$PayloadRoot,[string[]]$Paths) {
    foreach($path in $Paths){
        $item=Get-Item -LiteralPath (Join-Path $PayloadRoot $path) -Force -ErrorAction Stop
        foreach($entry in @($item)+@(if($item.PSIsContainer){Get-ChildItem -LiteralPath $item.FullName -Recurse -Force})){
            if($entry.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Runtime executable copies cannot contain links.'}
        }
    }
    $expected=Get-RelativeFileEntries -Root $Snapshot.runtime_root -Paths $Paths -RequirePaths
    $actual=Get-RelativeFileEntries -Root $PayloadRoot -Paths $Paths -RequirePaths
    Assert-ReleaseFileMap $expected $actual 'Verified runtime/package copy'
}

# Runtime fixture executables are private copies (IPC identity); data stays a
# read-only junction to the seal. A different junction target is never accepted.
function Assert-ReleaseRuntimeFixture($Snapshot,[string]$PayloadRoot,[string]$Architecture) {
    Assert-ReleasePayload $Snapshot $PayloadRoot @("$Architecture/Shanjie.dll",'tools')
    $data=Get-Item -LiteralPath (Join-Path $PayloadRoot 'data') -Force -ErrorAction Stop
    $expected=[IO.Path]::GetFullPath((Join-Path $Snapshot.runtime_root 'data'))
    if(!($data.Attributes -band [IO.FileAttributes]::ReparsePoint) -or [IO.Path]::GetFullPath([string]$data.Target) -cne $expected){throw 'Release runtime data must refer to the sealed snapshot.'}
}

function Get-ReleaseArtifacts([string]$ProjectRoot,[string]$CandidateId,[object[]]$CtestReports,[string]$PackagingPermitPath) {
    if($CandidateId -notmatch '^[0-9a-f]{32}$'){throw 'Invalid release candidate identifier.'}
    $candidate="build/release-candidates/$CandidateId"
    $paths=@("build/release-snapshots/$CandidateId","$candidate/Shanjie-Windows-Setup.exe","$candidate/candidate.json","$candidate/package")
    if($PackagingPermitPath){$paths += [IO.Path]::GetRelativePath($ProjectRoot,$PackagingPermitPath)}
    foreach($report in $CtestReports){$paths += [IO.Path]::GetRelativePath($ProjectRoot,$report.junit)}
    Get-RelativeFileEntries -Root $ProjectRoot -Paths $paths -RequirePaths
}

# Used before packing and again by promotion. The pre-package inventory excludes
# only release.package; all x64/x86 tests and parameter assertions stay mandatory.
function Assert-ReleaseTestCoverage([string]$ProjectRoot,$Record,[string]$EvidencePath,[switch]$CiArtifact,[switch]$BeforePackage) {
    $expected=[Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    $caseRows=@()
    foreach($preset in 'windows-x64','windows-x86'){
        $inventoryText=& ctest --test-dir (Join-Path $ProjectRoot "build/cmake/$preset") -C Release --show-only=json-v1
        if($LASTEXITCODE){throw "Cannot verify the current $preset release test inventory."}
        $inventory=($inventoryText|Out-String)|ConvertFrom-Json
        $architectureCount=0
        foreach($test in $inventory.tests){
            $labels=@(($test.properties|Where-Object name -eq 'LABELS').value|ForEach-Object{$_ -split ';'})
            $required=if($preset -eq 'windows-x64'){'full' -in $labels -or 'release' -in $labels}else{'release' -in $labels}
            if($CiArtifact -and $record.ci_safe -and 'ui' -in $labels -and 'ci-safe' -notin $labels){$required=$false}
            if($BeforePackage -and $test.name -eq 'release.package'){$required=$false}
            if($required){
                $architectureCount++
                [void]$expected.Add("${preset}::$($test.name)")
                $caseRows += [pscustomobject]@{name=$test.name;preset=$preset;selected=$true;reason='Required release acceptance'}
            }
        }
        if(!$architectureCount){throw "Current $preset RELEASE inventory is empty."}
    }
    if(!$expected.Count){throw 'Current RELEASE inventory is empty.'}
    $receiptDirectory=(Split-Path $EvidencePath -Parent)+[IO.Path]::DirectorySeparatorChar
    foreach($testReport in $record.ctest){
        if($testReport.snapshot_id -cne $record.release.candidate_id -or $testReport.snapshot_sha256 -cne $record.release.snapshot_sha256){throw 'CTest evidence belongs to another release snapshot.'}
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

}

function Assert-ReleaseAcceptance([string]$ProjectRoot,[string]$ReportPath,[switch]$CiArtifact) {
    $ProjectRoot=[IO.Path]::GetFullPath($ProjectRoot)
    $ReportPath=[IO.Path]::GetFullPath($ReportPath)
    $reportsRoot=[IO.Path]::GetFullPath((Join-Path $ProjectRoot 'build/reports'))+[IO.Path]::DirectorySeparatorChar
    if(!$ReportPath.StartsWith($reportsRoot,[StringComparison]::OrdinalIgnoreCase)){throw 'Release receipt must belong to this worktree under build/reports.'}
    $record=Get-Content -LiteralPath $ReportPath -Raw|ConvertFrom-Json -AsHashtable
    $identity=Get-ReleaseIdentity $ProjectRoot
    if(!$identity.clean){throw 'Formal release requires a clean Git worktree.'}
    if($record.schema -ne 4 -or $record.level -ne 'RELEASE' -or $record.status -ne 'passed' -or !$record.release){throw 'A completed RELEASE receipt is required; status alone is insufficient.'}
    if(!$CiArtifact -and $record.ci_safe){throw 'CI-safe or skipped interactive acceptance cannot authorize a local formal release.'}
    foreach($source in @($record.release.source_start,$record.release.source_end)){
        if(!$source.clean -or $source.commit -ne $identity.commit -or $source.version -ne $identity.version){throw 'Release receipt is dirty, stale, or belongs to another commit/version.'}
    }
    if($record.commit -ne $identity.commit -or $record.version -ne $identity.version -or $record.worktree_dirty){throw 'Release receipt source identity does not match HEAD.'}
    if(@(Compare-Object @('x64','x86') @($record.release.architectures)).Count){throw 'Both x64 and x86 release acceptance are required.'}

    Assert-ReleaseTestCoverage $ProjectRoot $record $ReportPath -CiArtifact:$CiArtifact

    $candidateId=$record.release.candidate_id
    $snapshot=Get-ReleaseSnapshot $ProjectRoot $candidateId $record.release.snapshot_sha256 -VerifyFiles
    if($snapshot.source.commit -cne $identity.commit -or $snapshot.source.version -cne $identity.version -or !$snapshot.source.clean){throw 'Snapshot belongs to another release source.'}
    $permit=Assert-ReleasePackagingPermit $ProjectRoot $record.release.packaging_permit $candidateId $snapshot.sha256
    $current=Get-ReleaseArtifacts $ProjectRoot $candidateId $record.ctest $record.release.packaging_permit
    $saved=$record.release.artifacts
    if(!$saved -or $saved.Count -ne $current.Count){throw 'Release build artifacts differ from the verified set.'}
    foreach($entry in $current.GetEnumerator()){
        if(!$saved.Contains($entry.Key) -or $saved[$entry.Key] -cne $entry.Value){throw "Release artifact changed after acceptance: $($entry.Key)"}
    }
    $candidateRoot=Join-Path $ProjectRoot "build/release-candidates/$candidateId"
    $candidate=Get-Content -LiteralPath (Join-Path $candidateRoot 'candidate.json') -Raw|ConvertFrom-Json
    if($candidate.snapshot_id -cne $candidateId -or $candidate.snapshot_sha256 -cne $snapshot.sha256 -or $candidate.packaging_permit_sha256 -cne $permit.sha256){throw 'Candidate was not packed from the dual-architecture accepted snapshot.'}
    if(!$candidate.integrity_verified -or $candidate.source.commit -ne $identity.commit -or $candidate.source.version -ne $identity.version -or !$candidate.source.clean){throw 'Candidate package was not integrity-verified from the accepted clean source.'}
    Assert-ReleasePackageCopy $snapshot (Join-Path $candidateRoot 'package')
    $installer=Join-Path $candidateRoot 'Shanjie-Windows-Setup.exe'
    $key=[IO.Path]::GetRelativePath($ProjectRoot,$installer).Replace('\','/')
    $candidateHash=($current.GetEnumerator()|Where-Object{ $_.Key.Replace('\','/') -eq $key }|Select-Object -First 1).Value
    if($candidate.installer_sha256 -cne $candidateHash){throw 'Final candidate installer hash differs from its verified metadata.'}
    [pscustomobject]@{identity=$identity;report_path=$ReportPath;report_sha256=(Get-FileHash -LiteralPath $ReportPath).Hash.ToLowerInvariant();candidate_id=$candidateId;installer=$installer;sha256=$candidateHash;local_release_verified=(!$CiArtifact)}
}

function Assert-ReleasePackageCopy($Snapshot,[string]$PackageRoot) {
    foreach($item in Get-ChildItem -LiteralPath $PackageRoot -Recurse -Force){if($item.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Candidate package cannot contain links.'}}
    $expected=Get-RelativeFileEntries -Root $Snapshot.runtime_root -Paths @('.') -RequirePaths
    $actual=Get-RelativeFileEntries -Root $PackageRoot -Paths @('.') -RequirePaths
    [void]$actual.Remove('SHA256SUMS.txt')
    Assert-ReleaseFileMap $expected $actual 'Candidate payload'
}

function New-ReleasePackagingPermit([string]$ProjectRoot,[string]$Path,$Record) {
    if($Record.level -ne 'RELEASE' -or $Record.status -ne 'tests-passed'){throw 'Both architectures must complete before packaging.'}
    Assert-ReleaseTestCoverage $ProjectRoot $Record $Path -BeforePackage -CiArtifact:([bool]$Record.ci_safe)
    $snapshot=Get-ReleaseSnapshot $ProjectRoot $Record.release.candidate_id $Record.release.snapshot_sha256 -VerifyFiles
    $identity=Get-ReleaseIdentity $ProjectRoot
    if(!$identity.clean -or $snapshot.source.commit -cne $identity.commit -or $snapshot.source.version -cne $identity.version){throw 'Release source changed before packaging.'}
    if(Test-Path -LiteralPath $Path){throw 'Packaging permits cannot be overwritten.'}
    [IO.File]::WriteAllText($Path,($Record|ConvertTo-Json -Depth 18),[Text.UTF8Encoding]::new($false))
    (Get-Item -LiteralPath $Path).IsReadOnly=$true
    Assert-ReleasePackagingPermit $ProjectRoot $Path $snapshot.id $snapshot.sha256
}

function Assert-ReleasePackagingPermit([string]$ProjectRoot,[string]$Path,[string]$SnapshotId,[string]$ManifestHash) {
    if(!$Path){throw 'Packaging requires completed dual-architecture test evidence.'}
    $Path=[IO.Path]::GetFullPath($Path)
    $reportsRoot=[IO.Path]::GetFullPath((Join-Path $ProjectRoot 'build/reports'))+[IO.Path]::DirectorySeparatorChar
    if(!$Path.StartsWith($reportsRoot,[StringComparison]::OrdinalIgnoreCase)){throw 'Packaging evidence must belong to this worktree.'}
    $record=Get-Content -LiteralPath $Path -Raw|ConvertFrom-Json -AsHashtable
    if($record.schema -ne 4 -or $record.level -ne 'RELEASE' -or $record.status -ne 'tests-passed' -or $record.release.candidate_id -cne $SnapshotId -or $record.release.snapshot_sha256 -cne $ManifestHash){throw 'Packaging permit does not match the tested release snapshot.'}
    if(@(Compare-Object @('x64','x86') @($record.release.architectures)).Count){throw 'Packaging requires both architectures.'}
    $snapshot=Get-ReleaseSnapshot $ProjectRoot $SnapshotId $ManifestHash -VerifyFiles
    $identity=Get-ReleaseIdentity $ProjectRoot
    if(!$identity.clean -or !$snapshot.source.clean -or $snapshot.source.commit -cne $identity.commit -or $snapshot.source.version -cne $identity.version){throw 'Release snapshot source changed before packaging.'}
    Assert-ReleaseTestCoverage $ProjectRoot $record $Path -BeforePackage -CiArtifact:([bool]$record.ci_safe)
    [pscustomobject]@{path=$Path;sha256=(Get-FileHash -LiteralPath $Path).Hash.ToLowerInvariant();snapshot=$snapshot}
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

Export-ModuleMember -Function Assert-ReleaseRuntimeFixture,Copy-ReleaseFiles,Copy-ReleasePackageInputs,New-ReleaseSnapshot,Get-ReleaseSnapshot,Assert-ReleasePayload,Assert-ReleasePackageCopy,New-ReleasePackagingPermit,Assert-ReleasePackagingPermit,Get-ReleaseIdentity,Get-ReleaseArtifacts,Assert-ReleaseAcceptance,Complete-ReleaseArtifact
