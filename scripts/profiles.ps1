#Requires -Version 7
param([switch]$Remove)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System.Runtime.InteropServices;
public static class ShanjieProfiles {
    [DefaultDllImportSearchPaths(DllImportSearchPath.System32)]
    [DllImport("input.dll", CharSet = CharSet.Unicode)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool InstallLayoutOrTip(string profiles, uint flags);
}
'@
$tips = '0x0404:{8912696E-8A31-4DF3-A7BC-0261F3B6A147}{AC59A613-F813-4DAD-BA97-D221A87A6ACE};0x0404:{8912696E-8A31-4DF3-A7BC-0261F3B6A147}{338145B4-047A-4564-A0DE-223DC22C1AE1}'
$flags = if ($Remove) { 1 } else { 0 }
if (![ShanjieProfiles]::InstallLayoutOrTip($tips,$flags)) { throw 'Windows 無法更新善解輸入法清單。' }
