#Requires -Version 7
param([Parameter(Mandatory)][string]$CorePath)
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$file = Join-Path $CorePath 'src/engine.rs'
$content = (Get-Content $file -Raw).Replace("`r`n","`n")
function Replace-LearningContract([string]$old, [string]$new) {
    if ([regex]::Matches($script:content, [regex]::Escape($old)).Count -ne 1) { throw "Upstream selection-learning contract changed: $old" }
    $script:content = $script:content.Replace($old, $new)
}
Replace-LearningContract '    learner: Learner,' "    learner: Learner,`n    windows_selection_learning: bool,`n    windows_selection_base: Option<Vec<Record>>,`n    windows_learning_prefix: Option<WindowsLearningPrefix>,`n    windows_partial_commit: bool,"
Replace-LearningContract '            learner: Learner::default(),' "            learner: Learner::default(),`n            windows_selection_learning: false,`n            windows_selection_base: None,`n            windows_learning_prefix: None,`n            windows_partial_commit: false,"
Replace-LearningContract 'struct Fixed {' "#[derive(Clone)]`nstruct Fixed {"
Replace-LearningContract 'struct PathWord {' "#[derive(Clone)]`nstruct PathWord {"
Replace-LearningContract '    fn choose(&mut self, idx: usize) -> Result<(), EngineError> {' '    fn choose_without_selection_learning(&mut self, idx: usize) -> Result<(), EngineError> {'
Replace-LearningContract '    fn apply_pred(&mut self, mut p: Pred) -> Result<Output, EngineError> {' '    fn apply_pred_without_selection_learning(&mut self, mut p: Pred) -> Result<Output, EngineError> {'
Replace-LearningContract '    fn learn_commit(&mut self, display: &str) {' '    fn learn_commit_original(&mut self, display: &str) {'
Replace-LearningContract '    fn persist(&mut self, touched: &[Record]) {' "    fn persist(&mut self, touched: &[Record]) {`n        if self.windows_selection_base.is_some() { return; }"
Replace-LearningContract '    pub fn key(&mut self, key: Key) -> Result<Output, EngineError> {' "    pub fn key(&mut self, key: Key) -> Result<Output, EngineError> {`n        if self.restore_windows_selection() { self.refresh()?; }"
Replace-LearningContract '    pub fn insert_windows_literal(&mut self, text: &str) -> Result<Output, EngineError> {' "    pub fn insert_windows_literal(&mut self, text: &str) -> Result<Output, EngineError> {`n        if self.restore_windows_selection() { self.refresh()?; }"
Replace-LearningContract '    fn clear_all(&mut self) {' "    fn clear_all(&mut self) {`n        self.restore_windows_selection();`n        self.windows_learning_prefix = None;"
Replace-LearningContract '    pub fn set_learning(&mut self, on: bool) {' "    pub fn set_learning(&mut self, on: bool) {`n        if !on { self.restore_windows_selection(); self.windows_learning_prefix = None; }"
Replace-LearningContract '        self.learner = Learner::from_records(records);' "        self.windows_selection_base = None;`n        self.learner = Learner::from_records(records);"
Replace-LearningContract '    pub fn learning_clear(&mut self) -> Result<(), StoreError> {' "    pub fn learning_clear(&mut self) -> Result<(), StoreError> {`n        self.windows_selection_base = None;`n        self.windows_learning_prefix = None;"
Replace-LearningContract '        let commit = if mode == ResetMode::Commit { digits + &self.shown().0 } else { String::new() };' "        if mode == ResetMode::Commit && self.windows_selection_learning { self.learn_commit(&self.display.clone()); }`n        let commit = if mode == ResetMode::Commit { digits + &self.shown().0 } else { String::new() };"
# Keep the submitted head's reading, chosen text and context until the remaining tail is committed.
Replace-LearningContract "        self.refresh()?;`n        let commit = self.take_commit();`n        self.left = left + &commit;" "        self.refresh()?;`n        let mut prefix = self.windows_learning_prefix.take();`n        let paused = self.paused;`n        if self.windows_selection_learning && self.learning && !paused {`n            prefix.get_or_insert_with(|| WindowsLearningPrefix { left: left.clone(), ..Default::default() }).append(self, &self.display);`n        }`n        self.windows_partial_commit = true;`n        let commit = self.take_commit();`n        self.windows_partial_commit = false;`n        self.left = left + &commit;"
Replace-LearningContract "        self.cursor = 0;`n        self.refresh()?;`n        Ok(Some(self.view(true, commit)))" "        self.cursor = 0;`n        self.paused = paused;`n        self.windows_learning_prefix = prefix;`n        self.refresh()?;`n        self.preview_windows_selection();`n        Ok(Some(self.view(true, commit)))"
$content += "`n" + (Get-Content "$root/platform/selection_learning_windows.rs" -Raw)
Set-Content $file $content -Encoding utf8NoBOM
