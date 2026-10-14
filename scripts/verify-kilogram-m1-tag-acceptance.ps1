[CmdletBinding()]
param(
    [string]$Tag = 'M1.0.0',
    [switch]$SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$workspace = [System.IO.Path]::GetFullPath((Split-Path -Parent $PSScriptRoot))
$expectedTag = 'M1.0.0'
$expectedTagObject = '15e74f7a786070c247d4d1a0f023adde8b250922'
$expectedCommit = '60b20b364c8df2324a0a287b4bcb6c056ae12c39'
$expectedTree = '6e3183e6a4cf754a4aa3f7788c5e8be4e70cf9f3'
$expectedCandidateBlob = 'c24389d673006deb28b09c66e4accfcc00e600f7'
$expectedAcceptanceRfcBlob = '38b791db2a96437e6fe10debd2375210fd2c22fe'
$expectedDecisionBlob = 'bae5c73ff79bf4477e7d030268cca532f83ccaed'

function Assert-Exact {
    param(
        [Parameter(Mandatory = $true)] [string]$Actual,
        [Parameter(Mandatory = $true)] [string]$Expected,
        [Parameter(Mandatory = $true)] [string]$Label
    )

    if ($Actual -cne $Expected) {
        throw "$Label mismatch: expected '$Expected', got '$Actual'"
    }
}

function Assert-Rejected {
    param(
        [Parameter(Mandatory = $true)] [scriptblock]$Action,
        [Parameter(Mandatory = $true)] [string]$Label
    )

    try {
        & $Action
    }
    catch {
        Write-Output "$Label=rejected"
        return
    }
    throw "$Label was unexpectedly accepted"
}

function Invoke-Git {
    param(
        [Parameter(Mandatory = $true)] [string[]]$Arguments,
        [int[]]$AllowedExitCodes = @(0)
    )

    $output = @(& git -C $workspace @Arguments 2>&1)
    $exitCode = $LASTEXITCODE
    if ($exitCode -notin $AllowedExitCodes) {
        throw "git failed ($exitCode): git $($Arguments -join ' ')`n$($output -join [Environment]::NewLine)"
    }
    return [pscustomobject]@{
        ExitCode = $exitCode
        Output = @($output | ForEach-Object { [string]$_ })
    }
}

if ($SelfTest) {
    Assert-Exact 'a' 'a' 'synthetic identity'
    Assert-Rejected {
        Assert-Exact 'a' 'b' 'synthetic identity'
    } 'changed_identity'
    Assert-Rejected {
        Assert-Exact 'M1.0.1' $expectedTag 'synthetic tag'
    } 'changed_tag'
    Write-Output 'm1_tag_acceptance_self_test=passed'
    Write-Output 'network_executed=false'
    Write-Output 'working_tree_mutated=false'
    exit 0
}

Assert-Exact $Tag $expectedTag 'accepted tag name'
$tagRef = "refs/tags/$Tag"
$tagType = (Invoke-Git @('cat-file', '-t', $tagRef)).Output[0].Trim()
Assert-Exact $tagType 'tag' 'accepted tag object type'
$tagObject = (Invoke-Git @('rev-parse', $tagRef)).Output[0].Trim()
Assert-Exact $tagObject $expectedTagObject 'accepted annotated tag object'
$commit = (Invoke-Git @('rev-parse', "$tagRef`^{commit}")).Output[0].Trim()
Assert-Exact $commit $expectedCommit 'accepted commit'
$tree = (Invoke-Git @('rev-parse', "$tagRef`^{tree}")).Output[0].Trim()
Assert-Exact $tree $expectedTree 'accepted source tree'
$candidateBlob = (Invoke-Git @('rev-parse', "${Tag}:M1-CANDIDATE.json")).Output[0].Trim()
Assert-Exact $candidateBlob $expectedCandidateBlob 'accepted candidate record blob'
$acceptanceRfcBlob = (Invoke-Git @(
    'rev-parse', "${Tag}:docs/RFC-0123-m1-technical-poc-acceptance.md"
)).Output[0].Trim()
Assert-Exact $acceptanceRfcBlob $expectedAcceptanceRfcBlob 'accepted RFC blob'
$decisionBlob = (Invoke-Git @('rev-parse', "${Tag}:ai-docs/decisions.md")).Output[0].Trim()
Assert-Exact $decisionBlob $expectedDecisionBlob 'accepted decision log blob'
$ancestor = Invoke-Git @('merge-base', '--is-ancestor', $expectedCommit, 'HEAD') @(0, 1)
if ($ancestor.ExitCode -ne 0) {
    throw 'The accepted M1.0.0 commit is not an ancestor of current HEAD'
}

Write-Output 'm1_tag_acceptance_status=verified'
Write-Output "accepted_tag=$expectedTag"
Write-Output "accepted_tag_object=$expectedTagObject"
Write-Output "accepted_commit=$expectedCommit"
Write-Output "accepted_tree=$expectedTree"
Write-Output "accepted_candidate_blob=$expectedCandidateBlob"
Write-Output "accepted_rfc_blob=$expectedAcceptanceRfcBlob"
Write-Output "accepted_decision_blob=$expectedDecisionBlob"
Write-Output 'accepted_snapshot_ancestor_of_head=true'
Write-Output 'historical_candidate_reclassified=false'
Write-Output 'network_executed=false'
Write-Output 'working_tree_mutated=false'
