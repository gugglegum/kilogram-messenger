function Test-KilogramS5SingleTestLog([string]$Log,[string]$Selector) {
    $name='(?m)^test (?:[^\s]*::)?'+[regex]::Escape($Selector)+' \.\.\.'
    $passed='(?m)^test result: ok\. 1 passed; 0 failed; 0 ignored;'
    $completed='(?m)^status=passed\r?$'
    return [regex]::Matches($Log,$name).Count -eq 1 -and
        [regex]::Matches($Log,$passed).Count -eq 1 -and
        [regex]::Matches($Log,$completed).Count -eq 1
}
