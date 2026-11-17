param([Parameter(Mandatory)][string]$Mode)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Console]::OutputEncoding = New-Object Text.UTF8Encoding($false)

function Send($Row) { [Console]::WriteLine(($Row | ConvertTo-Json -Compress -Depth 7)) }
function Reply($Id, $Value) { Send @{v=1;type='result';id=$Id;ok=$true;value=$Value} }

$protected = $Mode -cnotin @('echo','disconnect')
if ($protected) {
    $private = (@('blindword') * 24) -join ' '
    [Console]::Error.WriteLine($private)
    if ($Mode -eq 'early') { [Console]::WriteLine($private); exit 1 }
}
Send @{v=1;type='ready'}
$count = 0
while ($null -ne ($line = [Console]::ReadLine())) {
    $command = ConvertFrom-Json $line
    if ($protected) {
        switch ($command.op) {
            'create' {
                Reply $command.id @{state='recovery_available';account_id=$null;device_id=$null;contexts=$null}
            }
            'recovery_show' {
                $good = @{
                    v=1;type='result';id=$command.id;ok=$true
                    value=@{recovery_scope='root-key-only-authority-history-required';recovery_words=$private}
                }
                switch ($Mode) {
                    'malformed' { [Console]::WriteLine('{"' + $private); exit 1 }
                    'oversized' { [Console]::WriteLine('x' * 20000 + $private); exit 1 }
                    'truncated' { [Console]::Write(($good | ConvertTo-Json -Compress)); exit 1 }
                    'eof' { exit 1 }
                    'timeout' { Start-Sleep -Seconds 45; exit 1 }
                    'wrong-id' { $good.id = 'other' }
                    'wrong-scope' { $good.value.recovery_scope = 'wrong' }
                }
                Send $good
            }
            'recovery_ack' {
                if (!$command.args.saved) { exit 1 }
                Reply $command.id @{state='recovery_saved';account_id=$null;device_id=$null;contexts=$null}
            }
            'quit' { Reply $command.id @{closed=$true}; exit 0 }
            default { exit 1 }
        }
        continue
    }
    switch ($command.op) {
        'echo' {
            $count++
            Reply $command.id @{count=$count;text=$command.args.text}
        }
        'incoming' {
            Reply $command.id @{accepted=$true}
            Start-Sleep -Milliseconds 400
            Send @{v=1;type='event';event='incoming';event_id=('a' * 64);context='public-a';epoch='public-epoch';runtime_instance='public-runtime';text='first'}
            Send @{v=1;type='event';event='incoming';event_id=('b' * 64);context='public-b';epoch='public-epoch';runtime_instance='public-runtime';text='second'}
        }
        'hold' {
            Send @{v=1;type='event';event='incoming';event_id=('c' * 64);context='public-hold';epoch='public-epoch';runtime_instance='public-runtime';text='started'}
            Start-Sleep -Seconds 60
        }
        'quit' { Reply $command.id @{connected=$false}; exit 0 }
        default { exit 1 }
    }
}
exit 1
