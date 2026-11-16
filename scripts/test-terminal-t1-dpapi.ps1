param(
    [Parameter(Mandatory)][ValidateSet('Protect','Unprotect','Echo')][string]$Mode,
    [ValidateSet('local','direct','manual')][string]$Case = 'local',
    [Parameter(Mandatory)][string]$ProbeRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$timer = [Diagnostics.Stopwatch]::StartNew()
$result = [ordered]@{
    stage = $Mode
    ok = $false
    error_code = $null
    matched = $null
    logon_type = $null
    session_zero = $null
    profile_hive_loaded = $null
    context_error_code = $null
    elapsed_ms = $null
}
$status = 3
$sample = $null
$plain = $null

try {
    $utf8 = [Text.UTF8Encoding]::new($false, $true)
    [Console]::OutputEncoding = $utf8
    if ($Mode -eq 'Echo') {
        [Console]::InputEncoding = $utf8
        $request = [Console]::ReadLine() | ConvertFrom-Json
        if ($request.id -cne 'stdio-test' -or $request.marker -isnot [string] -or $request.marker.Length -gt 40) {
            throw 'Invalid echo fixture.'
        }
        [Console]::WriteLine((@{ stage = 'echo'; id = $request.id; marker = $request.marker } | ConvertTo-Json -Compress))
        $result.matched = $null -eq [Console]::ReadLine()
        $result.ok = $result.matched
        if ($result.ok) { $status = 0 }
    } else {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
using System.Security.Principal;

public static class TerminalT1Dpapi
{
    [StructLayout(LayoutKind.Sequential)]
    struct Blob { public int Size; public IntPtr Data; }

    [DllImport("crypt32.dll", SetLastError = true)]
    static extern bool CryptProtectData(ref Blob input, IntPtr description, IntPtr entropy,
        IntPtr reserved, IntPtr prompt, uint flags, out Blob output);

    [DllImport("crypt32.dll", SetLastError = true)]
    static extern bool CryptUnprotectData(ref Blob input, IntPtr description, IntPtr entropy,
        IntPtr reserved, IntPtr prompt, uint flags, out Blob output);

    [DllImport("kernel32.dll")]
    static extern IntPtr LocalFree(IntPtr memory);

    public static byte[] Transform(bool protect, byte[] bytes)
    {
        const uint uiForbidden = 1;
        var pin = GCHandle.Alloc(bytes, GCHandleType.Pinned);
        Blob output = new Blob();
        try
        {
            var input = new Blob { Size = bytes.Length, Data = pin.AddrOfPinnedObject() };
            var nil = IntPtr.Zero;
            bool ok = protect
                ? CryptProtectData(ref input, nil, nil, nil, nil, uiForbidden, out output)
                : CryptUnprotectData(ref input, nil, nil, nil, nil, uiForbidden, out output);
            if (!ok) throw new Win32Exception(Marshal.GetLastWin32Error());
            if (output.Data == nil || output.Size <= 0 || output.Size > 4096)
                throw new InvalidOperationException("Unexpected DPAPI output.");
            byte[] copy = new byte[output.Size];
            Marshal.Copy(output.Data, copy, 0, copy.Length);
            return copy;
        }
        finally
        {
            if (output.Data != IntPtr.Zero)
            {
                for (int i = 0; i < output.Size; i++) Marshal.WriteByte(output.Data, i, 0);
                LocalFree(output.Data);
            }
            pin.Free();
        }
    }

    [StructLayout(LayoutKind.Sequential)]
    struct Luid { public uint Low; public int High; }

    [StructLayout(LayoutKind.Sequential)]
    struct TokenStatistics
    {
        public Luid Token, Authentication;
        public long Expiration;
        public int Kind, Impersonation;
        public uint Charged, Available, Groups, Privileges;
        public Luid Modified;
    }

    [StructLayout(LayoutKind.Sequential)]
    struct UnicodeString { public ushort Length, Maximum; public IntPtr Buffer; }

    [StructLayout(LayoutKind.Sequential)]
    struct LogonHeader
    {
        public uint Size;
        public Luid LogonId;
        public UnicodeString User, Domain, Package;
        public uint Kind, Session;
    }

    [DllImport("advapi32.dll", SetLastError = true)]
    static extern bool GetTokenInformation(IntPtr token, int type, out TokenStatistics data,
        int size, out int written);

    [DllImport("secur32.dll")]
    static extern uint LsaGetLogonSessionData(ref Luid id, out IntPtr data);

    [DllImport("secur32.dll")]
    static extern uint LsaFreeReturnBuffer(IntPtr data);

    [DllImport("advapi32.dll")]
    static extern uint LsaNtStatusToWinError(uint status);

    public static uint[] ReadLogonContext()
    {
        using (var identity = WindowsIdentity.GetCurrent(TokenAccessLevels.Query))
        {
            const int tokenStatistics = 10;
            TokenStatistics stats;
            int written;
            if (!GetTokenInformation(identity.Token, tokenStatistics, out stats,
                Marshal.SizeOf(typeof(TokenStatistics)), out written))
                throw new Win32Exception(Marshal.GetLastWin32Error());
            IntPtr data;
            uint status = LsaGetLogonSessionData(ref stats.Authentication, out data);
            if (status != 0) throw new Win32Exception((int)LsaNtStatusToWinError(status));
            if (data == IntPtr.Zero)
                throw new InvalidOperationException("Missing logon data.");
            try
            {
                if (Marshal.ReadInt32(data) < Marshal.SizeOf(typeof(LogonHeader)))
                    throw new InvalidOperationException("Short logon data.");
                var header = (LogonHeader)Marshal.PtrToStructure(data, typeof(LogonHeader));
                return new uint[] { header.Kind, header.Session };
            }
            finally { LsaFreeReturnBuffer(data); }
        }
    }
}
'@
        try {
            $context = [TerminalT1Dpapi]::ReadLogonContext()
            $result.logon_type = $context[0]
            $result.session_zero = $context[1] -eq 0
            $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
            try { $result.profile_hive_loaded = Test-Path -LiteralPath "Registry::HKEY_USERS\$($identity.User.Value)" }
            finally { $identity.Dispose() }
        } catch {
            $result.context_error_code = $_.Exception.GetBaseException().HResult
            if ($_.Exception.GetBaseException() -is [ComponentModel.Win32Exception]) {
                $result.context_error_code = $_.Exception.GetBaseException().NativeErrorCode
            }
        }

        if ((Split-Path -Leaf $ProbeRoot) -cnotmatch '^terminal-t1-0a-[a-z0-9-]+$' -or
            -not (Test-Path -LiteralPath $ProbeRoot -PathType Container)) {
            throw 'Expected a dedicated probe directory.'
        }
        $directory = Join-Path $ProbeRoot $Case
        $blob = Join-Path $directory 'probe.blob'
        $digest = Join-Path $directory 'probe.sha256'
        $sha = [Security.Cryptography.SHA256]::Create()
        try {
            if ($Mode -eq 'Protect') {
                $sample = [byte[]]::new(32)
                $rng = [Security.Cryptography.RandomNumberGenerator]::Create()
                try { $rng.GetBytes($sample) }
                finally { $rng.Dispose() }
                $sealed = [TerminalT1Dpapi]::Transform($true, $sample)
                New-Item -ItemType Directory -Path $directory | Out-Null
                [IO.File]::WriteAllBytes($blob, $sealed)
                [IO.File]::WriteAllBytes($digest, $sha.ComputeHash($sample))
                $result.ok = $true
            } else {
                $blobSize = (Get-Item -LiteralPath $blob).Length
                if ($blobSize -lt 1 -or $blobSize -gt 4096 -or (Get-Item -LiteralPath $digest).Length -ne 32) {
                    throw 'Invalid bounded probe.'
                }
                $plain = [TerminalT1Dpapi]::Transform($false, [IO.File]::ReadAllBytes($blob))
                $result.matched = [BitConverter]::ToString($sha.ComputeHash($plain)) -ceq
                    [BitConverter]::ToString([IO.File]::ReadAllBytes($digest))
                $result.ok = $result.matched
            }
            if ($result.ok) { $status = 0 }
        } finally { $sha.Dispose() }
    }
} catch {
    $cause = $_.Exception.GetBaseException()
    if ($cause -is [ComponentModel.Win32Exception]) {
        $result.error_code = $cause.NativeErrorCode
        $status = 2
    } else {
        $result.error_code = $cause.HResult
    }
} finally {
    foreach ($bytes in @($sample, $plain)) {
        if ($null -ne $bytes) { [Array]::Clear($bytes, 0, $bytes.Length) }
    }
    $result.elapsed_ms = $timer.ElapsedMilliseconds
    [Console]::WriteLine(($result | ConvertTo-Json -Compress))
}
exit $status
