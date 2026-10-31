# SOURCE FOR REVIEW ONLY: no execution authorized by the preparation task.
# Numeric validation only; not production Parent/job/claim/private end-to-end.
Set-StrictMode -Version Latest
$ErrorActionPreference='Stop'
$source='E:\AI-Projects\kilogram-p2p-messenger\scripts\invoke-migration-s53-cold-preservation.ps1'
$fixedShell='C:\Users\Paul\.cache\codex-runtimes\codex-primary-runtime\dependencies\native\powershell\pwsh.exe'
function Assert-Regression([bool]$Condition,[string]$Label) {
    if(-not $Condition){throw ('Numeric regression failed: '+$Label)}
}
function Text-Sha([string]$Text) {
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($Text))).ToLowerInvariant()
}
Assert-Regression ($IsWindows -and [Environment]::ProcessPath -ceq $fixedShell) 'fixed shell'
$raw=[IO.File]::ReadAllBytes($source)
Assert-Regression ($raw.Length -eq 71513 -and
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($raw)).ToLowerInvariant() -ceq '8cb43e3e6dfba31b21f014ea4f82be1451417a15424ed046d87d2d455d6f3557') 'source pin'
$text=[Text.UTF8Encoding]::new($false,$true).GetString($raw)
$tokens=$null;$errors=$null
$ast=[Management.Automation.Language.Parser]::ParseInput($text,[ref]$tokens,[ref]$errors)
Assert-Regression ($errors.Count -eq 0) 'source syntax'
$validate=@($ast.FindAll({param($n)
    $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq 'Validate-Report'
},$true))
Assert-Regression ($validate.Count -eq 1) 'validator count'
$loops=@($validate[0].Body.FindAll({param($n) $n -is [Management.Automation.Language.ForEachStatementAst]},$true))
$selected=[ordered]@{
    output=@($loops | Where-Object {$_.Condition.Extent.Text -match "'ancestry'"})
    parent_guards=@($loops | Where-Object {$_.Condition.Extent.Text -match "'open_attempts'"})
    stdio=@($validate[0].Body.FindAll({param($n)
        $n -is [Management.Automation.Language.CommandAst] -and $n.GetCommandName() -ceq 'Need' -and
        $n.Extent.Text.EndsWith("'public_stdio'")
    },$true))
}
$expected='{"output":"e79d2bb519cf5cd414ad2d182c021185e5c2d2da9917e85758e3c3da55b7009b","parent_guards":"8c21185c510da1497d2b27b125fe0e04a71a77a27157b31ace2f182ec5abce7d","stdio":"c42e4869072c3b206ee0eaf4045ba511d759e76c7db2ca95fa99f634468d96a6","Stop-Cold":"b4a6fe904b608a4558d0c89069bfaa93cac90465c56093b3611113107d7e457f","Need":"a691ff0a69d20cbe34308cf841c116d0e3094db698e5ae15fbd8613feb2d8815","Value":"cc1b141ade661e203e485e8bdc77db7a8209e51ccdf17f7210fcb33154a1615f"}' | ConvertFrom-Json -AsHashtable
$blocks=[ordered]@{};$members=[ordered]@{}
foreach($name in @('output','parent_guards','stdio')) {
    Assert-Regression ($selected[$name].Count -eq 1) ('statement count '+$name)
    $node=$selected[$name][0]
    Assert-Regression ((Text-Sha $node.Extent.Text) -ceq $expected[$name]) ('statement pin '+$name)
    $blocks[$name]=[scriptblock]::Create($node.Extent.Text)
    if($name -ne 'stdio') {
        $members[$name]=@($node.Condition.FindAll({param($n)
            $n -is [Management.Automation.Language.StringConstantExpressionAst]
        },$true) | ForEach-Object {$_.Value})
        Assert-Regression ($members[$name].Count -eq 6) ('member count '+$name)
    }
}
foreach($name in @('Stop-Cold','Need','Value')) {
    $nodes=@($ast.FindAll({param($n)
        $n -is [Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -ceq $name
    },$true))
    Assert-Regression ($nodes.Count -eq 1 -and (Text-Sha $nodes[0].Extent.Text) -ceq $expected[$name]) ('dependency pin '+$name)
    # Define ONLY these three actual source dependencies, never the launcher script.
    . ([scriptblock]::Create($nodes[0].Extent.Text))
}
$script:accepted=0;$script:refused=0
function Check-Case([scriptblock]$Validation,[object]$Object,[bool]$Accept,[string]$Code,[string]$Label) {
    $caught=$null
    try { & $Validation } catch { $caught=$_.Exception }
    if($Accept) {
        Assert-Regression ($null -eq $caught) ('accept '+$Label)
        $script:accepted++
    } else {
        Assert-Regression ($caught -is [InvalidOperationException] -and
            $caught.HResult -eq -2146233079 -and $caught.Data['cold_launch_code'] -ceq $Code) ('exact refusal '+$Label)
        $script:refused++
    }
}
# The candidate domain is deliberately independent of the extracted predicate.
$inRange=@([int]::MinValue,[int]-1,[int]0,[int]1,[int]::MaxValue,
    [long][int]::MinValue,[long]-1,[long]0,[long]1,[long][int]::MaxValue)
$badCounters=@([long]-2147483649,[long]2147483648,[long]::MinValue,[long]::MaxValue,
    '0','2147483647',[double]0,[double]1.5,$true,$false,$null)
foreach($group in @('output','parent_guards')) {
    $code=if($group -eq 'output'){'public_output'}else{'public_guard'}
    foreach($member in $members[$group]) {
        foreach($candidate in $inRange) {
            $map=[ordered]@{};foreach($key in $members[$group]){$map[$key]=[int]0};$map[$member]=$candidate
            $obj=[pscustomobject]@{$group=$map}
            Check-Case $blocks[$group] $obj $true $code ($group+'.'+$member)
        }
        foreach($candidate in $badCounters) {
            $map=[ordered]@{};foreach($key in $members[$group]){$map[$key]=[int]0};$map[$member]=$candidate
            $obj=[pscustomobject]@{$group=$map}
            Check-Case $blocks[$group] $obj $false $code ($group+'.'+$member)
        }
    }
}
$stdioGood=@([int]0,[int]4096,[long]0,[long]4096)
$stdioBad=@([int]-1,[int]4097,[long]-1,[long]4097,[long]::MinValue,[long]::MaxValue,
    '0','4096',[double]0,[double]4096,$true,$false,$null)
foreach($member in @('stdout_chars','stderr_chars')) {
    foreach($candidate in $stdioGood) {
        $obj=[pscustomobject]@{stdout_chars=[int]0;stderr_chars=[int]0};$obj.$member=$candidate
        Check-Case $blocks.stdio $obj $true 'public_stdio' $member
    }
    foreach($candidate in $stdioBad) {
        $obj=[pscustomobject]@{stdout_chars=[int]0;stderr_chars=[int]0};$obj.$member=$candidate
        Check-Case $blocks.stdio $obj $false 'public_stdio' $member
    }
}
# Roundtrip ONLY in-memory public-shaped counter objects using the production decoder flags.
# Both signed endpoints cover every output/guard member; stdio covers both endpoints.
$jsonInt64Observed=0
foreach($edge in @([int]::MinValue,[int]::MaxValue)) {
    $obj=[ordered]@{output=[ordered]@{};parent_guards=[ordered]@{};stdout_chars=0;stderr_chars=4096}
    foreach($group in @('output','parent_guards')){foreach($member in $members[$group]){$obj[$group][$member]=$edge}}
    $roundtrip=ConvertFrom-Json -InputObject (ConvertTo-Json -InputObject $obj -Depth 12 -Compress) -Depth 12 -DateKind String
    foreach($group in @('output','parent_guards')) {
        foreach($member in $members[$group]) {
            Assert-Regression ($roundtrip.$group.$member -is [long]) ('roundtrip Int64 '+$group+'.'+$member)
            $jsonInt64Observed++
        }
        $code=if($group -eq 'output'){'public_output'}else{'public_guard'}
        Check-Case $blocks[$group] $roundtrip $true $code ('roundtrip '+$group)
    }
    foreach($member in @('stdout_chars','stderr_chars')) {
        Assert-Regression ($roundtrip.$member -is [long]) ('roundtrip Int64 '+$member)
        $jsonInt64Observed++
    }
    Check-Case $blocks.stdio $roundtrip $true 'public_stdio' 'roundtrip stdio'
}
Assert-Regression ($accepted -eq 134 -and $refused -eq 158 -and $jsonInt64Observed -eq 28) 'coverage totals'
[ordered]@{classification='cold-a-numeric-regression';scope='actual-ast-numeric-statements-only'
    source_sha256='8cb43e3e6dfba31b21f014ea4f82be1451417a15424ed046d87d2d455d6f3557';accepted=$accepted;refused=$refused;json_int64_observed=$jsonInt64Observed
    launcher_entrypoint_invoked=$false;private_access=$false;process_spawn=$false;native_helpers=$false} | ConvertTo-Json -Compress
