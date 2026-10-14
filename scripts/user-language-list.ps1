# Embedded in the native installer. Windows PowerShell 5.1 is included in Windows.
& {
    $ErrorActionPreference='Stop'
    $WarningPreference='SilentlyContinue'
    [Console]::OutputEncoding=[Text.UTF8Encoding]::new($false)
    function Get-State($Languages,$Override) {
        $items=@(foreach($language in $Languages){
            [ordered]@{tag=$language.LanguageTag;tips=@($language.InputMethodTips);spelling=[bool]$language.Spellchecking;handwriting=[bool]$language.Handwriting}
        })
        [ordered]@{languages=$items;defaultTip=if($Override){[string]$Override}else{$null}}
    }
    function Get-LiveState {Get-State (Get-WinUserLanguageList) ((Get-WinDefaultInputMethodOverride).InputMethodTip)}
    function Test-OwnTip([string]$Tip) {$Tip -match '^0404:\{8912696E-8A31-4DF3-A7BC-0261F3B6A147\}\{(?:AC59A613-F813-4DAD-BA97-D221A87A6ACE|338145B4-047A-4564-A0DE-223DC22C1AE1)\}$'}
    function Get-OtherState($State,$BaselineTags) {
        [ordered]@{languages=@(foreach($language in $State.languages){
            $otherTips=@($language.tips | Where-Object {!(Test-OwnTip $_)})
            if($language.tag -eq 'zh-Hant-TW' -and $language.tag -notin $BaselineTags -and !$otherTips.Count){continue}
            [ordered]@{tag=$language.tag;tips=$otherTips;spelling=$language.spelling;handwriting=$language.handwriting}
        });defaultTip=if(Test-OwnTip $State.defaultTip){$null}else{$State.defaultTip}}
    }
    function Convert-Languages($State) {
        $items=New-Object 'System.Collections.Generic.List[Microsoft.InternationalSettings.Commands.WinUserLanguage]'
        foreach($language in $State.languages){
            $created=(New-WinUserLanguageList -Language $language.tag)[0]
            $created.InputMethodTips.Clear()
            foreach($tip in $language.tips){$created.InputMethodTips.Add([string]$tip)}
            $created.Spellchecking=[bool]$language.spelling;$created.Handwriting=[bool]$language.handwriting
            $items.Add($created)
        }
        ,$items
    }
    try {
        Import-Module International
        $request=Get-Content -LiteralPath $env:SHANJIE_LANGUAGE_REQUEST -Raw -Encoding UTF8 | ConvertFrom-Json
        if($request.action -eq 'read'){$state=Get-LiveState}
        elseif($request.action -eq 'preview'){
            # Construct real International cmdlet objects, without writing user settings.
            $objects=Convert-Languages $request.desired
            $state=Get-State $objects $request.desired.defaultTip
        }
        elseif($request.action -eq 'apply' -or $request.action -eq 'restore'){
            if($env:SHANJIE_TEST_PROFILE -eq '1'){throw 'Isolated verification cannot change the live language list'}
            $current=Get-LiveState
            $tags=@($request.before.languages.tag)
            $oldOther=(Get-OtherState $request.before $tags | ConvertTo-Json -Depth 6 -Compress)
            $currentOther=(Get-OtherState $current $tags | ConvertTo-Json -Depth 6 -Compress)
            $alternate=if($request.alternate){Get-OtherState $request.alternate $tags | ConvertTo-Json -Depth 6 -Compress}else{$oldOther}
            if($oldOther -cne $currentOther -and $alternate -cne $currentOther){throw '其他語言或輸入設定已變更，未覆寫這些設定。'}
            if(($current | ConvertTo-Json -Depth 6 -Compress) -cne ($request.desired | ConvertTo-Json -Depth 6 -Compress)){
                $objects=Convert-Languages $request.desired
                Set-WinUserLanguageList -LanguageList $objects -Force
                $override=(Get-WinDefaultInputMethodOverride).InputMethodTip
                if($override -cne $request.desired.defaultTip){
                    if($request.desired.defaultTip){Set-WinDefaultInputMethodOverride -InputTip $request.desired.defaultTip}
                    else{Set-WinDefaultInputMethodOverride}
                }
            }
            $state=Get-LiveState
        }
        else{throw 'Unknown language-list action'}
        [IO.File]::WriteAllText($request.output,($state | ConvertTo-Json -Depth 6),[Text.UTF8Encoding]::new($false))
        exit 0
    } catch {
        [Console]::Error.WriteLine($_.Exception.Message)
        exit 1
    }
}
