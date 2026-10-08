#Requires -Version 7
function Read-ProfileIni([string]$Content) {
    $document = [ordered]@{}; $section = ''
    foreach ($row in $Content -split '\r?\n') {
        $line = $row.Trim()
        if (!$line -or $line.StartsWith(';') -or $line.StartsWith('#')) { continue }
        if ($line -match '^\[(.+)\]$') { $section = $Matches[1]; if (!$document.Contains($section)) { $document[$section] = [ordered]@{} }; continue }
        $equals = $line.IndexOf('=')
        if (!$section -or $equals -le 0) { throw '設定檔格式有誤，請確認 [分類] 與 key=value。' }
        $document[$section][$line.Substring(0,$equals).Trim()] = $line.Substring($equals+1).Trim()
    }
    return $document
}
function Write-ProfileIni($Document) {
    $lines = foreach ($section in $Document.Keys) {
        "[$section]"
        foreach ($key in $Document[$section].Keys) { "$key=$($Document[$section][$key])" }
        ''
    }
    return ($lines -join "`r`n") + "`r`n"
}
function Preserve-ProfileData($Preset,$Previous) {
    $schema = if ($Previous.Contains('profile') -and $Previous.profile.Contains('schema')) { [int]$Previous.profile.schema } else { 1 }
    if ($schema -lt 3 -and (!$Previous.Contains('engine') -or !$Previous.engine.Contains('model'))) { $Preset.engine.model = '1' }
    $keys = @{
        engine = @('model','ranking','demote')
        input = @('dictionary','keyboard','learning','context','chinese','full_width')
        candidates = @('big5')
        appearance = @('theme','font_size','mode_indicator')
    }
    foreach ($section in $keys.Keys) {
        foreach ($key in $keys[$section]) {
            if ($Previous.Contains($section) -and $Previous[$section].Contains($key)) { $Preset[$section][$key] = $Previous[$section][$key] }
        }
    }
    return $Preset
}
