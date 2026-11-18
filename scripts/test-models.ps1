#Requires -Version 7
# Public fixtures for isolated native verification; never writes a real user profile.
function Initialize-TestModels([string]$ProfileRoot) {
    $project=Split-Path $PSScriptRoot -Parent
    $resolved=[IO.Path]::GetFullPath($ProfileRoot)
    $buildPrefix=[IO.Path]::GetFullPath("$project/build")+[IO.Path]::DirectorySeparatorChar
    if(!$resolved.StartsWith($buildPrefix,[StringComparison]::OrdinalIgnoreCase)){throw 'Model fixture profile must remain inside project/build.'}
    $id='model-v6-classes-v4';$fixture="$project/build/model-variants/$id"
    if(!(Test-Path -LiteralPath "$fixture/model.json")) {
        $catalog=Get-Content "$project/MODEL-CATALOG.json" -Raw|ConvertFrom-Json
        $spec=$catalog.models|Where-Object id -eq $id
        foreach($part in @(@($spec.model_release,'bigram.sjlm','bigram-v6.sjlm',$spec.model_size,$spec.model_sha256),@($spec.classes_release,'classes.sjc','classes-v4.sjc',$spec.classes_size,$spec.classes_sha256))) {
            $cache="$project/build/model/$($part[2])"
            if(!(Test-Path $cache) -or (Get-FileHash -LiteralPath $cache -Algorithm SHA256).Hash.ToLowerInvariant() -ne $part[4]) {
                & "$project/scripts/download-asset.ps1" -Url "https://github.com/Nanako0129/shanjie/releases/download/$($part[0])/$($part[1])" -Destination $cache -Size $part[3] -Sha256 $part[4]
            }
        }
        $temporary="$project/build/model-variants/fixture-profile"
        $work='fixture-'+[guid]::NewGuid().ToString('N');$stage="$temporary/Shanjie/Models/.staging/$work"
        New-Item -ItemType Directory $stage -Force|Out-Null
        Copy-Item "$project/build/model/bigram-v6.sjlm" "$stage/bigram.sjlm"
        Copy-Item "$project/build/model/classes-v4.sjc" "$stage/classes.sjc"
        $spec|ConvertTo-Json|Set-Content "$stage/model.json" -Encoding utf8NoBOM
        $start=[Diagnostics.ProcessStartInfo]::new("$project/dist/tools/shanjie-manager.exe")
        $start.UseShellExecute=$false;$start.CreateNoWindow=$true;$start.RedirectStandardOutput=$true;$start.RedirectStandardError=$true
        $start.Environment['LOCALAPPDATA']=$temporary;$start.Environment['SHANJIE_TEST_PROFILE']='1'
        $start.ArgumentList.Add('model-prepare');$start.ArgumentList.Add($work)
        $process=[Diagnostics.Process]::Start($start);$stdout=$process.StandardOutput.ReadToEndAsync();$stderr=$process.StandardError.ReadToEndAsync()
        if(!$process.WaitForExit(60000) -or !$stdout.Wait(5000) -or !$stderr.Wait(5000)){throw 'Public fixture mapping did not finish'}
        if($process.ExitCode){throw $stderr.Result};$process.Dispose()
        Copy-Item "$temporary/Shanjie/Models/$id" $fixture -Recurse
    }
    $target=Join-Path $resolved "Shanjie/Models/$id"
    if(!(Test-Path -LiteralPath "$target/model.json")){New-Item -ItemType Directory (Split-Path $target -Parent) -Force|Out-Null;Copy-Item -LiteralPath $fixture -Destination $target -Recurse}
}
