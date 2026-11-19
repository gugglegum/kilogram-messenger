# Current ordinary observations only. Never writes policy, registry, tasks or account settings.
. "$PSScriptRoot/test-terminal-t1-3e-lab-common.ps1"
function Initialize-LabNative {
    if(!('KilogramLabNative' -as [type])){Add-Type -Path "$PSScriptRoot/test-terminal-t1-3e-lab-native.cs"}
}
function Lab-JsonHash($Value) {
    $bytes=[Text.Encoding]::UTF8.GetBytes((ConvertTo-Json -InputObject $Value -Depth 18 -Compress))
    if($bytes.Length -gt 2097152){throw 'LAB_OBSERVATION_SIZE'}
    $h=[Security.Cryptography.SHA256]::Create()
    try{[BitConverter]::ToString($h.ComputeHash($bytes)).Replace('-','').ToLowerInvariant()}finally{$h.Dispose()}
}
function Get-LabPolicyInputs {
    $roots=@('HKLM:/SOFTWARE/Policies/Microsoft/WindowsFirewall','HKLM:/SOFTWARE/Microsoft/PolicyManager/current/device/Firewall',
        'HKLM:/SYSTEM/CurrentControlSet/Services/SharedAccess/Parameters/FirewallPolicy/Mdm',
        'HKLM:/SYSTEM/CurrentControlSet/Services/SharedAccess/Parameters/FirewallPolicy/FirewallRules',
        'HKLM:/SYSTEM/CurrentControlSet/Services/SharedAccess/Parameters/FirewallPolicy/StandardProfile',
        'HKLM:/SYSTEM/CurrentControlSet/Services/SharedAccess/Parameters/FirewallPolicy/DomainProfile',
        'HKLM:/SYSTEM/CurrentControlSet/Services/SharedAccess/Parameters/FirewallPolicy/PublicProfile')
    $rows=@();$count=0
    foreach($root in $roots){
        if(!(Test-Path -LiteralPath $root)){$rows+=,[ordered]@{path=$root;absent=$true};continue}
        $keys=@((Get-Item -LiteralPath $root))+@(Get-ChildItem -LiteralPath $root -Recurse -ErrorAction Stop)
        if($keys.Count -gt 2048){throw 'LAB_POLICY_INPUT_BOUND'}
        foreach($key in @($keys|Sort-Object Name)){
            $names=[string[]]$key.GetValueNames();[Array]::Sort($names,[StringComparer]::OrdinalIgnoreCase)
            $values=@(foreach($name in $names){
                $count++;if($count -gt 16384){throw 'LAB_POLICY_INPUT_BOUND'}
                [ordered]@{name=$name;kind=[string]$key.GetValueKind($name);value=$key.GetValue($name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)}
            })
            $rows+=,[ordered]@{path=$key.Name;values=$values}
        }
    }
    @{sha256=(Lab-JsonHash $rows);keys=$rows.Count;values=$count}
}
function Get-LabNativeNetwork($Cfg) {
    Initialize-LabNative
    $bindings=@([KilogramLabNative]::Networks())
    $all=@([Net.NetworkInformation.NetworkInterface]::GetAllNetworkInterfaces())
    $network=@(@(foreach($b in $bindings){
        $interfaces=@($all|Where-Object {([guid]$_.Id) -eq [guid]$b.adapter -and $_.OperationalStatus -eq 'Up'})
        if($interfaces.Count -ne 1){throw 'LAB_NATIVE_NETWORK_UNCERTAIN'}
        $ifc=$interfaces[0];$index=$ifc.GetIPProperties().GetIPv4Properties().Index
        [ordered]@{index=$index;alias=$ifc.Name;adapter=([guid]$ifc.Id).ToString('D');network=$b.network;
            category=$(switch($b.category){0{'Public'};1{'Private'};2{'DomainAuthenticated'};default{throw 'LAB_NATIVE_NETWORK_UNCERTAIN'}})}
    })|Sort-Object index,network)
    $own=@($all|Where-Object {([guid]$_.Id) -eq [guid]$Cfg.interface_guid -and $_.OperationalStatus -eq 'Up'})
    if($own.Count -ne 1){throw 'LAB_NATIVE_NETWORK_UNCERTAIN'}
    $ifc=$own[0];$props=$ifc.GetIPProperties()
    $addresses=@($props.UnicastAddresses|Where-Object {$_.Address.AddressFamily -eq 'InterNetwork'}|ForEach-Object {"$($_.Address)/$($_.IPv4Mask)"}|Sort-Object)
    $matched=@($network|Where-Object { $_.index -eq $Cfg.interface_index -and $_.alias -ceq $Cfg.interface_alias -and $_.category -ceq 'Private' })
    if($matched.Count -ne 1 -or @($network|Where-Object {$_.category -ceq 'Private' -and $_.index -ne $Cfg.interface_index}).Count -or
       $props.GetIPv4Properties().Index -ne $Cfg.interface_index -or $ifc.Name -cne $Cfg.interface_alias -or "$($Cfg.ip)/255.255.255.0" -cnotin $addresses){throw 'LAB_NATIVE_NETWORK_DRIFT'}
    $raw=@(& "$env:windir/System32/netsh.exe" int ipv4 show dynamicport udp)
    $numbers=@([regex]::Matches(($raw -join "`n"),'\d+')|ForEach-Object {[int]$_.Value})
    if($LASTEXITCODE -ne 0 -or ($numbers -join ',') -cne '49152,16384'){throw 'LAB_NATIVE_UDP_DRIFT'}
    @{network=$network;adapter=@{index=$Cfg.interface_index;name=$ifc.Name;guid=([guid]$ifc.Id).ToString('D');addresses=$addresses};udp=@(49152,65535)}
}
function Convert-LabComSingletonAddress([string]$Value){
    # Persisted COM observations use x-x for the same one-address set CIM emits
    # as x. Only a complete canonical IPv4 singleton range is equivalent here.
    $m=[regex]::Match($Value,'\A((?:0|[1-9][0-9]{0,2})(?:\.(?:0|[1-9][0-9]{0,2})){3})-\1\z')
    if($m.Success -and !@($m.Groups[1].Value.Split('.')|Where-Object {[int]$_ -gt 255}).Count){return $m.Groups[1].Value}
    return $Value
}
function Convert-LabComRule($Rule,[string]$InstanceName) {
    # Only the default unauthenticated/unpackaged rule subset is supported for own rules.
    if([int]$Rule.SecureFlags -ne 0 -or $Rule.LocalUserOwner){throw 'LAB_COM_RULE_SECURITY_UNSUPPORTED'}
    $view=[ordered]@{Name=$InstanceName;DisplayName=[string]$Rule.Name;Description=[string]$Rule.Description;Group=[string]$Rule.Grouping;
        Enabled=([bool]$Rule.Enabled).ToString();Profile=$(switch([int]$Rule.Profiles){2{'Private'};default{throw 'LAB_COM_RULE_PROFILE_UNSUPPORTED'}});
        Direction=$(switch([int]$Rule.Direction){1{'Inbound'};2{'Outbound'};default{throw 'LAB_COM_RULE_DIRECTION_UNSUPPORTED'}});
        Action=$(switch([int]$Rule.Action){0{'Block'};1{'Allow'};default{throw 'LAB_COM_RULE_ACTION_UNSUPPORTED'}});
        EdgeTraversalPolicy=$(switch([int]$Rule.EdgeTraversalOptions){0{'Block'};1{'Allow'};default{throw 'LAB_COM_RULE_EDGE_UNSUPPORTED'}});
        Program=[Environment]::ExpandEnvironmentVariables([string]$Rule.ApplicationName);
        Protocol=$(switch([int]$Rule.Protocol){6{'TCP'};17{'UDP'};256{'Any'};default{throw 'LAB_COM_PROTOCOL_UNSUPPORTED'}});
        InterfaceType=$(switch([string]$Rule.InterfaceTypes){'All'{'Any'};'Wireless'{'Wireless'};'LAN'{'Wired'};default{throw 'LAB_COM_INTERFACE_UNSUPPORTED'}});
        Authentication='NotRequired';Encryption='NotRequired';OverrideBlockRules='False';DynamicTransport='Any'}
    foreach($pair in @(@('LocalPort','LocalPorts'),@('RemotePort','RemotePorts'),@('LocalAddress','LocalAddresses'),@('RemoteAddress','RemoteAddresses'),
       @('Service','ServiceName'),@('Package','LocalAppPackageId'),@('LocalUser','LocalUserAuthorizedList'),@('RemoteUser','RemoteUserAuthorizedList'),@('RemoteMachine','RemoteMachineAuthorizedList'))){
        $value=[string]$Rule.($pair[1]);if(!$value -or $value -ceq '*'){$value='Any'}
        if($pair[0] -cin @('LocalAddress','RemoteAddress')){$value=(Convert-LabComSingletonAddress $value) -replace '/(32|255\.255\.255\.255)$',''}
        $view[$pair[0]]=$value
    }
    if(!$view.Program){$view.Program='Any'}
    $interfaces=@($Rule.Interfaces|Where-Object {$_}|ForEach-Object {[string]$_}|Sort-Object)
    $view.InterfaceAlias=if($interfaces.Count){$interfaces -join ','}else{'Any'}
    return $view
}
function Get-LabOrdinaryState($Cfg,$Want,[switch]$AllowMissing) {
    $fw=New-Object -ComObject HNetCfg.FwPolicy2
    if([int]$fw.LocalPolicyModifyState -ne 0){throw 'LAB_COM_POLICY_UNCERTAIN'}
    $profiles=@(foreach($pair in @(@('Domain',1),@('Private',2),@('Public',4))){
        [ordered]@{name=$pair[0];enabled=([bool]$fw.FirewallEnabled($pair[1])).ToString();
            inbound=$(switch([int]$fw.DefaultInboundAction($pair[1])){0{'Block'};1{'Allow'};default{throw 'LAB_COM_PROFILE_UNCERTAIN'}});
            outbound=$(switch([int]$fw.DefaultOutboundAction($pair[1])){0{'Block'};1{'Allow'};default{throw 'LAB_COM_PROFILE_UNCERTAIN'}});
            block_all=[bool]$fw.BlockAllInboundTraffic($pair[1]);disabled=@($fw.ExcludedInterfaces($pair[1])|Where-Object {$_}|ForEach-Object {[string]$_}|Sort-Object)}
    })
    $key=Get-Item -LiteralPath 'HKLM:/SYSTEM/CurrentControlSet/Services/SharedAccess/Parameters/FirewallPolicy/FirewallRules'
    $all=@($fw.Rules)
    $unexpected=@($all|Where-Object {($_.Name -like 'Kilogram-Lab-*' -or $_.Grouping -like 'Kilogram-Lab-*') -and $_.Name -cnotin @($Want.rules|ForEach-Object DisplayName)})
    if($unexpected.Count){throw 'LAB_COM_UNEXPECTED_OWN_RULE'}
    $rules=@(foreach($expected in $Want.rules){
        $matching=@($all|Where-Object {$_.Name -ceq $expected.DisplayName})
        $raw=$key.GetValue($expected.Name,$null,[Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
        if(!$matching.Count -and $null -eq $raw -and $AllowMissing){[ordered]@{name=$expected.Name;absent=$true};continue}
        if($matching.Count -ne 1 -or $null -eq $raw -or $raw -isnot [string]){throw 'LAB_COM_OWN_RULE_DRIFT'}
        [ordered]@{name=$expected.Name;view=(Convert-LabComRule $matching[0] $expected.Name);persistent_sha256=(Lab-JsonHash $raw)}
    })
    @{backend='com-native-v1';profiles=$profiles;rules=$rules;policy_inputs=(Get-LabPolicyInputs);native_network=(Get-LabNativeNetwork $Cfg)}
}
function Get-LabComRuleComparison($Cfg,$Want,$Ordinary,$Relevant){
    # Own fixed rules only. Do not include unrelated policy or a full Audit.
    if(@($Want.rules).Count -gt 2 -or @($Ordinary.rules).Count -gt 2 -or @($Relevant.rules).Count -gt 4){throw 'LAB_COM_COMPARISON_BOUND'}
    $rows=@(foreach($r in $Ordinary.rules){
        $expected=@($Want.rules|Where-Object Name -ceq $r.name)
        $native=@($Relevant.rules|Where-Object {$_.view.Name -ceq $r.name})
        $diff=@()
        if($expected.Count -eq 1){
            $actualObject=if($r.view -is [Collections.IDictionary]){[pscustomobject]$r.view}else{$r.view}
            $expectedObject=if($expected[0] -is [Collections.IDictionary]){[pscustomobject]$expected[0]}else{$expected[0]}
            $keys=@(@($actualObject.PSObject.Properties.Name)+@($expectedObject.PSObject.Properties.Name)|Sort-Object -Unique)
            if($keys.Count -gt 40){throw 'LAB_COM_COMPARISON_BOUND'}
            $diff=@(foreach($key in $keys){
                $a=$actualObject.PSObject.Properties[$key];$e=$expectedObject.PSObject.Properties[$key]
                if(!$a -or !$e -or !(Lab-Equal $a.Value $e.Value)){
                    @{field=$key;actual_present=[bool]$a;expected_present=[bool]$e;actual=$(if($a){$a.Value}else{$null});expected=$(if($e){$e.Value}else{$null})}
                }
            })
        }
        @{name=$r.name;expected_count=$expected.Count;expected=$expected;ordinary=$r.view;native=$native;differences=$diff}
    })
    @{host=$Cfg.host;side=$Cfg.side;stage=(Get-LabStage);utc=[DateTime]::UtcNow.ToString('o');rules=$rows;
        expected_rules=@($Want.rules).Count;ordinary_rules=@($Ordinary.rules).Count;scope='own rule views only; no unrelated policy or credentials'}
}
function Save-LabComRuleComparison($Cfg,$Want,$Ordinary,$Relevant,[string]$Directory){
    $leaf=Split-Path -Leaf $Directory
    $root=Join-Path ([Environment]::GetFolderPath('CommonApplicationData')) 'Kilogram-Lab/SetupPackages'
    if($leaf -cnotmatch '^setup-[0-9a-f]{32}$' -or [IO.Path]::GetFullPath($Directory) -ine (Join-Path $root $leaf)){throw 'LAB_COM_DIAGNOSTIC_SCOPE'}
    Lab-Plain $Directory
    Lab-Write (Join-Path $Directory 'com-rule-comparison.json') (Get-LabComRuleComparison $Cfg $Want $Ordinary $Relevant) -MaxBytes 2097152
}
function Assert-LabOrdinaryAgainstCim($Cfg,$Want,$Ordinary,$Relevant,[string]$EvidenceDirectory) {
    if($Ordinary.backend -cne 'com-native-v1' -or (@($Ordinary.profiles|ForEach-Object name|Sort-Object) -join ',') -cne 'Domain,Private,Public' -or
       $Ordinary.rules.Count -ne $Want.rules.Count){throw 'LAB_COM_PROJECTION_SCHEMA'}
    foreach($p in $Ordinary.profiles){
        $native=@($Relevant.profiles|Where-Object {$_.store -ceq 'ActiveStore' -and $_.name -ceq $p.name})
        if($native.Count -ne 1 -or $native[0].allow_local -cne 'True' -or
           $native[0].enabled -cne $p.enabled -or $native[0].inbound -cne $p.inbound -or $native[0].outbound -cne $p.outbound -or
           (($native[0].allow_inbound -ceq 'True') -eq $p.block_all) -or !(Lab-Equal @($native[0].disabled|Where-Object {$_}) $p.disabled)){throw 'LAB_COM_CIM_PROFILE_DIFFERS'}
    }
    foreach($r in $Ordinary.rules){
        $expected=@($Want.rules|Where-Object Name -ceq $r.name)
        if($expected.Count -ne 1 -or !(Lab-Equal $r.view $expected[0])){
            if($EvidenceDirectory){
                try{Save-LabComRuleComparison $Cfg $Want $Ordinary $Relevant $EvidenceDirectory}
                catch{[Console]::Error.WriteLine('LAB_COM_DIAGNOSTIC_UNAVAILABLE:'+ $_.Exception.Message)}
            }
            throw 'LAB_COM_SPEC_DIFFERS'
        }
        foreach($store in @('PersistentStore','ActiveStore')){
            $native=@($Relevant.rules|Where-Object {$_.store -ceq $store -and $_.view.Name -ceq $r.name})
            if($native.Count -ne 1 -or !(Lab-Equal $r.view $native[0].view)){throw 'LAB_COM_CIM_RULE_DIFFERS'}
        }
        $effective=@(Get-NetFirewallRule -PolicyStore ActiveStore -TracePolicyStore -Name $r.name)
        if($effective.Count -ne 1 -or [string]$effective[0].PolicyStoreSourceType -cne 'Local'){throw 'LAB_COM_POLICY_SOURCE_UNSUPPORTED'}
    }
    $nativeNetwork=@($Ordinary.native_network.network|ForEach-Object {[ordered]@{index=$_.index;alias=$_.alias;category=$_.category}})
    if(!(Lab-Equal $nativeNetwork $Relevant.network)){throw 'LAB_COM_CIM_NETWORK_DIFFERS'}
    if([guid]$Ordinary.native_network.adapter.guid -ne [guid]$Relevant.adapter.guid -or $Ordinary.native_network.adapter.name -cne $Relevant.adapter.name -or $Relevant.adapter.status -cne 'Up'){throw 'LAB_COM_CIM_NETWORK_DIFFERS'}
    $expectedAddress=@($Relevant.adapter.addresses|ForEach-Object {$_ -replace '/24$','/255.255.255.0'})
    if(!(Lab-Equal $expectedAddress $Ordinary.native_network.adapter.addresses)){throw 'LAB_COM_CIM_NETWORK_DIFFERS'}
}
