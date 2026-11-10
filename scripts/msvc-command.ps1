# Emit a batch setup line. The explicit opt-in supports a portable MSVC/SDK
# already loaded in this process, without installing or changing system settings.
function Get-MsvcSetup([ValidateSet('x64','x86')][string]$Architecture) {
    if ($env:SHANJIE_USE_CURRENT_TOOLCHAIN -eq '1') {
        $compiler = Get-Command cl.exe -ErrorAction Stop
        $null = Get-Command rc.exe -ErrorAction Stop
        if ((Split-Path (Split-Path $compiler.Source -Parent) -Leaf) -ne $Architecture -or !$env:INCLUDE -or !$env:LIB) {
            throw "Load an $Architecture MSVC/SDK environment before using the current toolchain."
        }
        return 'rem Using the explicitly selected current MSVC/SDK environment'
    }
    $vs = & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (!$vs) { throw 'Install Visual Studio Build Tools or explicitly select a current MSVC/SDK environment.' }
    return 'call "'+(Join-Path $vs 'VC/Auxiliary/Build/vcvarsall.bat')+'" '+$Architecture+' >nul'
}

# Identifies the compiler so a toolchain update invalidates cached harnesses.
function Get-MsvcIdentity {
    if ($env:SHANJIE_USE_CURRENT_TOOLCHAIN -eq '1') {
        $compiler = (Get-Command cl.exe -ErrorAction Stop).Source
        return "$compiler $((Get-Item $compiler).VersionInfo.FileVersion)"
    }
    & "${env:ProgramFiles(x86)}/Microsoft Visual Studio/Installer/vswhere.exe" -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
}

# Builds a test harness once per distinct input and reuses it afterwards. The key covers the
# compile command (with this run's own folders removed), the toolchain, every file under tests/,
# the core headers and each library the command links. Leaves $LASTEXITCODE like cmd.exe does.
function Invoke-HarnessBuild([string]$Command, [string]$Script, [string]$Output, [string]$Log) {
    $root = Split-Path $PSScriptRoot -Parent
    $Output = [IO.Path]::GetFullPath($Output); $Script = [IO.Path]::GetFullPath($Script)
    # Callers mix separators ("$root\x/$name"), so compare on one spelling.
    $normalized = $Command.Replace('\', '/')
    foreach ($folder in @((Split-Path $Output -Parent), (Split-Path $Script -Parent)) | Sort-Object Length -Descending) {
        $normalized = $normalized.Replace($folder.Replace('\', '/'), '<own>')
    }
    $inputs = [Collections.Generic.List[string]]::new()
    $inputs.Add($normalized); $inputs.Add("toolchain=$(Get-MsvcIdentity)")
    $files = @(Get-ChildItem "$root/tests" -Recurse -File | Where-Object Extension -in '.cpp', '.h', '.hpp', '.inl') +
        @(Get-ChildItem "$root/upstream/shanjie/core/include" -Recurse -File) +
        @([regex]::Matches($Command, '"([^"]+\.lib)"') | ForEach-Object { $_.Groups[1].Value } | Where-Object { Test-Path -LiteralPath $_ } | Get-Item)
    foreach ($file in $files | Sort-Object FullName -Unique) {
        $inputs.Add("$([IO.Path]::GetRelativePath($root, $file.FullName))=$((Get-FileHash -LiteralPath $file.FullName).Hash)")
    }
    $key = [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($inputs -join "`n"))).Substring(0, 32).ToLowerInvariant()
    $cache = Join-Path $root "build/harness-cache/$key-$([IO.Path]::GetFileName($Output))"
    if (Test-Path -LiteralPath $cache) {
        New-Item -ItemType Directory (Split-Path $Output -Parent) -Force | Out-Null
        Copy-Item -LiteralPath $cache $Output -Force
        if ($Log) { Set-Content $Log "Reused cached harness $key" -Encoding utf8NoBOM }
        $global:LASTEXITCODE = 0; return
    }
    Set-Content $Script $Command -Encoding utf8NoBOM
    if ($Log) { & cmd.exe /d /c $Script *> $Log } else { & cmd.exe /d /c $Script }
    if ($LASTEXITCODE) { return }
    New-Item -ItemType Directory (Split-Path $cache -Parent) -Force | Out-Null
    Copy-Item -LiteralPath $Output "$cache.tmp" -Force; Move-Item -LiteralPath "$cache.tmp" $cache -Force
    $global:LASTEXITCODE = 0
}
