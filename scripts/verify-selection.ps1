# Compatibility import for upstream callers. The canonical pure policy now lives in
# test-plan.psm1; Select-VerificationPlan reports CTest Full/Labels/Skip/Reasons.
Import-Module "$PSScriptRoot/test-plan.psm1" -ErrorAction Stop
