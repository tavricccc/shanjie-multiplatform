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
