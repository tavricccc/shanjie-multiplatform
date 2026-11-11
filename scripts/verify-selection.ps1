# Which verification a set of changed paths needs (the table in AGENTS.md「驗證：做多少」).
# Pure: no git, no processes. Any path no rule recognises runs the full verify.ps1.

# Ordered: the first matching rule decides a path. Patterns are regexes on forward-slash paths.
$script:VerificationRules = @(
    @{ Name = 'docs';        Pattern = '^(docs/.*|[^/]*\.md|LICENSES/.*|\.github/(ISSUE_TEMPLATE/.*|pull_request_template\.md))$'; Steps = @() }
    @{ Name = 'selection';   Pattern = '^(scripts/verify-(changed|selection)\.ps1|tests/verify_selection\.ps1)$'; Steps = @('selection') }
    @{ Name = 'release';     Pattern = '^(scripts/(release|test_release)\.py|\.github/workflows/release\.yml)$'; Steps = @('release') }
    @{ Name = 'ci';          Pattern = '^\.github/workflows/ci\.yml$'; Steps = @('release', 'rust') }
    @{ Name = 'ui';          Pattern = '^ui/'; Steps = @('rust', 'ui') }
    @{ Name = 'add-word';    Pattern = '^(windows/src/(word_helper|word_popup|selection_text)\.rs|windows/src/tsf/add_word\.rs|tests/(add_word_popup|popup_lifecycle)\.cpp|scripts/verify-add-word\.ps1)$'; Steps = @('rust', 'regressions:add-word', 'regressions:popup_lifecycle') }
    @{ Name = 'candidate';   Pattern = '^(windows/src/(candidate|glass|appearance|font|bridge)\.rs|windows/src/tsf/candidates\.rs|tests/candidate_host_smoke\.cpp)$'; Steps = @('rust', 'ui-native', 'regressions:tsf_compat') }
    @{ Name = 'english';     Pattern = '^(windows/src/mixed_regression\.rs|tests/english_keys\.cpp|scripts/verify-english-keys\.ps1)$'; Steps = @('rust', 'mixed', 'regressions:english-keys') }
    @{ Name = 'submission';  Pattern = '^(tests/submission\.cpp|scripts/verify-submission\.ps1)$'; Steps = @('rust', 'regressions:submission') }
    @{ Name = 'installer';   Pattern = '^windows/src/installer(\.rs|/)'; Steps = @('rust') }
    @{ Name = 'diagnostics'; Pattern = '^windows/src/runtime_log\.rs$'; Steps = @('rust') }
    # Key paths and core requests: upstream core suite, the Windows key regressions and latency.
    @{ Name = 'platform';    Pattern = '^(platform/|scripts/prepare-core\.ps1$)'; Steps = @('rust', 'core', 'mixed', 'regressions:english-keys', 'regressions:submission', 'measure') }
)

function Select-VerificationPlan([string[]]$Paths) {
    $steps = [Collections.Generic.List[string]]::new()
    $reasons = [Collections.Generic.List[object]]::new()
    foreach ($raw in @($Paths | Where-Object { $_ } | ForEach-Object { $_.Trim().Replace('\', '/') } | Sort-Object -Unique)) {
        $rule = $script:VerificationRules | Where-Object { $raw -match $_.Pattern } | Select-Object -First 1
        if (!$rule) { $rule = @{ Name = 'unlisted (full)'; Steps = @('rust', 'full') } }
        $reasons.Add([pscustomobject]@{ Path = $raw; Rule = $rule.Name; Steps = @($rule.Steps) })
        foreach ($step in $rule.Steps) { if (!$steps.Contains($step)) { $steps.Add($step) } }
    }
    # verify.ps1 already builds and runs the core suite, mixed regression and all regression suites.
    # It does not run verify-ui.ps1, the release tests or the latency measurement, so those stay.
    if ($steps.Contains('full')) {
        foreach ($covered in @($steps | Where-Object { $_ -in 'core', 'mixed', 'ui-native' -or $_ -like 'regressions:*' })) { [void]$steps.Remove($covered) }
    }
    $suites = @($steps | Where-Object { $_ -like 'regressions:*' } | ForEach-Object { $_.Substring(12) })
    $needsBuild = [bool]@($steps | Where-Object { $_ -in 'ui', 'ui-native', 'mixed', 'measure' -or $_ -like 'regressions:*' }).Count
    [pscustomobject]@{
        Steps = @($steps | Where-Object { $_ -notlike 'regressions:*' })
        Suites = $suites
        Build = $needsBuild -and !$steps.Contains('full')
        Reasons = $reasons.ToArray()
    }
}
