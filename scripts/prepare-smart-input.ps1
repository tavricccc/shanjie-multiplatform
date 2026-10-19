#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$file = Join-Path $CorePath 'src/engine.rs'
$content = (Get-Content $file -Raw).Replace("`r`n","`n")
function Replace-SmartContract([string]$old,[string]$new) {
    if ([regex]::Matches($script:content,[regex]::Escape($old)).Count -ne 1) { throw "Upstream smart-input contract changed: $old" }
    $script:content = $script:content.Replace($old,$new)
}
Replace-SmartContract '    abbr: bool,' "    abbr: bool,`n    mixed_input: bool,`n    mixed_raw: String,`n    mixed_latin: bool,`n    mixed_display_char: Option<char>,"
Replace-SmartContract '            abbr: false,' "            abbr: false,`n            mixed_input: false,`n            mixed_raw: String::new(),`n            mixed_latin: false,`n            mixed_display_char: None,"
Replace-SmartContract '    pub fn key(&mut self, k: Key) -> Result<Output, EngineError> {' '    fn key_original(&mut self, k: Key) -> Result<Output, EngineError> {'
Replace-SmartContract '        self.shadow.push(ch);' '        self.shadow.push(self.mixed_display_char.unwrap_or(ch));'
Replace-SmartContract '        words.binary_search_by(|x| (**x).cmp(w)).is_ok()' '        let lower=w.to_ascii_lowercase(); words.binary_search_by(|x| (**x).cmp(lower.as_str())).is_ok()'
Replace-SmartContract '        let i = words.partition_point(|w| **w < *prefix);' '        let lower=prefix.to_ascii_lowercase(); let prefix=lower.as_str(); let i = words.partition_point(|w| **w < *prefix);'
Replace-SmartContract 'self.shadow.iter().all(char::is_ascii_lowercase)' 'self.shadow.iter().all(char::is_ascii_alphabetic)'
Replace-SmartContract '        self.left.clear();' "        self.mixed_raw.clear();`n        self.mixed_latin = false;`n        self.left.clear();"
Replace-SmartContract "        } else {`n            self.pending()`n        };" "        } else if self.mixed_input && !self.mixed_raw.is_empty() {`n            self.mixed_raw.clone()`n        } else {`n            self.pending()`n        };"
Replace-SmartContract '        text.push_str(&unfinished);' '        text.push_str(&unfinished); text.push_str(&self.pending_literal);'
Replace-SmartContract '        Output { handled, commit, preedit, cursor_utf16, candidates, selected, columns, first, total, vertical, english_run_utf16_start, english_run_utf16_len }' '        self.mixed_view(Output { handled, commit, preedit, cursor_utf16, candidates, selected, columns, first, total, vertical, english_run_utf16_start, english_run_utf16_len })'
Replace-SmartContract '    pub fn pick(&mut self, index: usize) -> Result<Option<Output>, EngineError> {' "    pub fn pick(&mut self, index: usize) -> Result<Option<Output>, EngineError> {`n        if self.is_mixed_hint() { return Ok(None); }"
$content += "`n" + (Get-Content "$root/platform/smart_input_windows.rs" -Raw)
$content += "`n" + (Get-Content "$root/platform/composition_windows.rs" -Raw)
Replace-SmartContract '    mixed_input: bool,' "    mixed_input: bool,`n    mixed_tolerance: bool,`n    enter_to_cursor: bool,`n    space_inputs_zhuyin: bool,`n    windows_raw: String,`n    pending_literal: String,`n    typed_literals: HashMap<usize,String>,"
Replace-SmartContract '            mixed_input: false,' "            mixed_input: false,`n            mixed_tolerance: false,`n            enter_to_cursor: false,`n            space_inputs_zhuyin: true,`n            windows_raw: String::new(),`n            pending_literal: String::new(),`n            typed_literals: HashMap::new(),"
Replace-SmartContract '        self.left.clear();' "        self.windows_raw.clear();`n        self.pending_literal.clear();`n        self.typed_literals.clear();`n        self.left.clear();"
Replace-SmartContract '        self.syls.insert(c, reading);' "        self.typed_literals = std::mem::take(&mut self.typed_literals).into_iter().map(|(i,text)| (if i >= c { i+1 } else { i },text)).collect();`n        self.syls.insert(c, reading);"
Replace-SmartContract '        self.syls.remove(i);' "        self.typed_literals = std::mem::take(&mut self.typed_literals).into_iter().filter(|(index,_)| *index != i).map(|(index,text)| (if index > i { index-1 } else { index },text)).collect();`n        self.syls.remove(i);"
Replace-SmartContract '        self.syls.splice(start..end, reading);' "        self.typed_literals = std::mem::take(&mut self.typed_literals).into_iter().filter(|(i,_)| *i < start || *i >= end).map(|(i,text)| (if i >= end { i - (end-start) + m } else { i },text)).collect();`n        self.syls.splice(start..end, reading);"
Replace-SmartContract '    fn choose(&mut self, idx: usize) -> Result<(), EngineError> {' "    fn choose(&mut self, idx: usize) -> Result<(), EngineError> {`n        if self.choose_windows_literal(idx)? { return Ok(()); }"
Replace-SmartContract '    fn forget_highlighted(&mut self) -> Result<(), EngineError> {' "    fn forget_highlighted(&mut self) -> Result<(), EngineError> {`n        if self.cands.as_ref().is_some_and(|c| c.literals.contains_key(&c.sel)) { return Ok(()); }"
Replace-SmartContract '    list: Vec<(String, usize)>,' "    list: Vec<(String, usize)>,`n    literals: HashMap<usize,WindowsLiteralCandidate>,"
Replace-SmartContract 'Cands { list, sel: 0, expanded: false, top: 0, vertical, first: 0, english: false, zh: None, rows: Vec::new(), dalt: None }' 'Cands { list, literals: HashMap::new(), sel: 0, expanded: false, top: 0, vertical, first: 0, english: false, zh: None, rows: Vec::new(), dalt: None }'
Set-Content $file $content -Encoding utf8NoBOM
& "$PSScriptRoot/prepare-english.ps1" | Out-Null
Copy-Item "$root/build/english-words.txt" "$CorePath/src/english-words.txt" -Force
Add-Content "$CorePath/src/ffi.rs" ("`n" + (Get-Content "$root/platform/smart_input_windows_ffi.rs" -Raw))
