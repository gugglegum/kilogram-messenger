[CmdletBinding()]
param(
    [string] $ServiceUrl,
    [string] $ExpectedStoreKey,
    [string] $StoreStartupLog,
    [ValidateRange(1, 60)] [int] $TimeoutSeconds = 15,
    [switch] $SelfTest
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-MailboxServiceUri {
    param([Parameter(Mandatory)] [string] $Url)

    $uri = $null
    if (-not [Uri]::TryCreate($Url, [UriKind]::Absolute, [ref]$uri) -or
        $uri.Scheme -ne 'https' -or
        [string]::IsNullOrWhiteSpace($uri.DnsSafeHost) -or
        $uri.UserInfo.Length -ne 0 -or
        -not [string]::IsNullOrEmpty($uri.Query) -or
        -not [string]::IsNullOrEmpty($uri.Fragment)) {
        throw 'ServiceUrl must be an absolute HTTPS URL without credentials, query or fragment.'
    }
    return $uri
}

function Read-BoundedStartupLog {
    param([Parameter(Mandatory)] [string] $Path)

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "mailbox store startup log is missing: $Path"
    }
    $item = Get-Item -LiteralPath $Path -Force
    if (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw 'mailbox store startup log must not be a reparse point.'
    }
    if ($item.Length -gt 65536) {
        throw 'mailbox store startup log exceeds the 64 KiB evidence limit.'
    }
    return (Get-Content -LiteralPath $item.FullName -Raw).Replace("`r`n", "`n")
}

function Assert-StartupLog {
    param(
        [Parameter(Mandatory)] [string] $Text,
        [Parameter(Mandatory)] [string] $StoreKey
    )

    if ($StoreKey -cnotmatch '^[0-9a-f]{64}$') {
        throw 'ExpectedStoreKey must be exactly 64 lowercase hexadecimal characters.'
    }
    foreach ($line in @(
        'transport_security=reverse-proxy-https-required',
        'storage_format=opaque-redb-v1',
        'blind_mailbox_transport=reverse-proxy-https-required'
    )) {
        if (-not [regex]::IsMatch($Text, "(?m)^$([regex]::Escape($line))$")) {
            throw "mailbox startup log is missing exact marker: $line"
        }
    }
    $keyMatches = [regex]::Matches($Text, '(?m)^blind_mailbox_store_key=([^\r\n]+)$')
    if ($keyMatches.Count -ne 1 -or $keyMatches[0].Groups[1].Value -cne $StoreKey) {
        throw 'mailbox startup log must contain exactly one matching pinned store key.'
    }
    foreach ($forbidden in @('account_id=', 'device_id=', 'conversation_id=', 'event_id=', 'message=')) {
        if ($Text.IndexOf($forbidden, [StringComparison]::OrdinalIgnoreCase) -ge 0) {
            throw "mailbox startup log exposes forbidden application metadata: $forbidden"
        }
    }
}

function Test-TrustedHealthEndpoint {
    param(
        [Parameter(Mandatory)] [Uri] $BaseUri,
        [Parameter(Mandatory)] [int] $Timeout
    )

    Add-Type -AssemblyName System.Net.Http
    $healthUri = [Uri]::new($BaseUri.AbsoluteUri.TrimEnd('/') + '/healthz')
    $handler = [Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $false
    $client = [Net.Http.HttpClient]::new($handler)
    $client.Timeout = [TimeSpan]::FromSeconds($Timeout)
    try {
        $response = $client.GetAsync($healthUri).GetAwaiter().GetResult()
        try {
            if ([int]$response.StatusCode -ne 200) {
                throw "mailbox health endpoint returned HTTP $([int]$response.StatusCode)"
            }
            $body = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
            if ($body.Length -gt 16) {
                throw 'mailbox health response exceeds the 16-byte limit.'
            }
            $text = [Text.Encoding]::UTF8.GetString($body)
            if ($text -cne "ok`n") {
                throw 'mailbox health response must be exactly ok followed by LF.'
            }
        }
        finally {
            $response.Dispose()
        }
    }
    finally {
        $client.Dispose()
        $handler.Dispose()
    }
    Write-Output "mailbox_health_url=$healthUri"
    Write-Output 'mailbox_tls=trusted-default-windows-store'
    Write-Output 'mailbox_redirects=disabled'
    Write-Output 'mailbox_health=ok'
}

function Test-PreflightInputs {
    param(
        [Parameter(Mandatory)] [string] $Url,
        [Parameter(Mandatory)] [string] $StoreKey,
        [Parameter(Mandatory)] [string] $LogPath,
        [switch] $SkipNetwork
    )

    $uri = Assert-MailboxServiceUri $Url
    $log = Read-BoundedStartupLog $LogPath
    Assert-StartupLog $log $StoreKey
    Write-Output "mailbox_service_url=$($uri.AbsoluteUri.TrimEnd('/'))"
    Write-Output "mailbox_store_key=$StoreKey"
    Write-Output 'mailbox_startup_log=verified'
    if (-not $SkipNetwork) {
        Test-TrustedHealthEndpoint $uri $TimeoutSeconds
    }
}

if ($SelfTest) {
    $root = Join-Path ([IO.Path]::GetTempPath()) ('kilogram-mailbox-preflight-' + [Guid]::NewGuid().ToString('N'))
    try {
        New-Item -ItemType Directory -Path $root | Out-Null
        $key = '61' * 32
        $log = Join-Path $root 'store.log'
        Set-Content -LiteralPath $log -Encoding utf8 -Value @(
            'status=listening',
            'listen_address=127.0.0.1:8787',
            'transport_security=reverse-proxy-https-required',
            'storage_format=opaque-redb-v1',
            "blind_mailbox_store_key=$key",
            'blind_mailbox_transport=reverse-proxy-https-required'
        )
        $null = Test-PreflightInputs 'https://mailbox.example.test' $key $log -SkipNetwork

        $wrongKeyRejected = $false
        try {
            $null = Test-PreflightInputs 'https://mailbox.example.test' ('62' * 32) $log -SkipNetwork
        }
        catch {
            $wrongKeyRejected = $true
        }
        if (-not $wrongKeyRejected) {
            throw 'self-test accepted a startup log with a different pinned store key.'
        }

        $httpRejected = $false
        try {
            $null = Test-PreflightInputs 'http://192.0.2.1:8787' $key $log -SkipNetwork
        }
        catch {
            $httpRejected = $true
        }
        if (-not $httpRejected) {
            throw 'self-test accepted a remote cleartext mailbox URL.'
        }

        Write-Output 'mailbox_store_preflight_self_test=passed'
        Write-Output 'pinned_store_key_mismatch=rejected'
        Write-Output 'remote_cleartext_http=rejected'
        exit 0
    }
    finally {
        if (Test-Path -LiteralPath $root) {
            Remove-Item -LiteralPath $root -Recurse -Force
        }
    }
}

foreach ($value in @(
    @{ Name = 'ServiceUrl'; Value = $ServiceUrl },
    @{ Name = 'ExpectedStoreKey'; Value = $ExpectedStoreKey },
    @{ Name = 'StoreStartupLog'; Value = $StoreStartupLog }
)) {
    if ([string]::IsNullOrWhiteSpace([string]$value.Value)) {
        throw "-$($value.Name) is required unless -SelfTest is used."
    }
}

Test-PreflightInputs $ServiceUrl $ExpectedStoreKey $StoreStartupLog
Write-Output 'status=mailbox-store-preflight-passed'
