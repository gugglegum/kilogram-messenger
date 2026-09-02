#![cfg_attr(not(windows), allow(dead_code))]

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::{Result, bail};
use tokio::sync::Notify;

use crate::recovery_plan::{RecoveryNetworkClass, RecoveryPowerSource};

const IANA_ETHERNET_CSMACD: u32 = 6;
const IANA_IEEE80211: u32 = 71;
const IANA_WWANPP: u32 = 243;
const IANA_WWANPP2: u32 = 244;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryPlatformContextSource {
    CallerSupplied,
    #[cfg(windows)]
    WindowsNative,
    #[cfg(not(windows))]
    UnsupportedPlatform,
}

impl RecoveryPlatformContextSource {
    fn as_str(self) -> &'static str {
        match self {
            Self::CallerSupplied => "caller-supplied",
            #[cfg(windows)]
            Self::WindowsNative => "windows-native",
            #[cfg(not(windows))]
            Self::UnsupportedPlatform => "unsupported-platform",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryProbeStatus {
    Available,
    Unavailable,
    CallerSupplied,
}

impl RecoveryProbeStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Unavailable => "unavailable",
            Self::CallerSupplied => "caller-supplied",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryNetworkCost {
    Unrestricted,
    Fixed,
    Variable,
    Unknown,
}

impl RecoveryNetworkCost {
    fn as_str(self) -> &'static str {
        match self {
            Self::Unrestricted => "unrestricted",
            Self::Fixed => "fixed",
            Self::Variable => "variable",
            Self::Unknown => "unknown",
        }
    }

    fn is_metered(self) -> Option<bool> {
        match self {
            Self::Unrestricted => Some(false),
            Self::Fixed | Self::Variable => Some(true),
            Self::Unknown => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryConnectivity {
    None,
    Local,
    Constrained,
    Internet,
    Unknown,
}

impl RecoveryConnectivity {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Local => "local",
            Self::Constrained => "constrained",
            Self::Internet => "internet",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryTruth {
    Yes,
    No,
    Unknown,
}

impl RecoveryTruth {
    fn from_option(value: Option<bool>) -> Self {
        match value {
            Some(true) => Self::Yes,
            Some(false) => Self::No,
            None => Self::Unknown,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Yes => "true",
            Self::No => "false",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryBatteryStatus {
    NotPresent,
    Discharging,
    Idle,
    Charging,
    Unknown,
}

impl RecoveryBatteryStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotPresent => "not-present",
            Self::Discharging => "discharging",
            Self::Idle => "idle",
            Self::Charging => "charging",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryPowerSupplyStatus {
    NotPresent,
    Inadequate,
    Adequate,
    Unknown,
}

impl RecoveryPowerSupplyStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::NotPresent => "not-present",
            Self::Inadequate => "inadequate",
            Self::Adequate => "adequate",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryEnergySaverStatus {
    Disabled,
    Off,
    On,
    Unknown,
}

impl RecoveryEnergySaverStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Off => "off",
            Self::On => "on",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RecoveryPlatformContext {
    source: RecoveryPlatformContextSource,
    network_probe_status: RecoveryProbeStatus,
    power_probe_status: RecoveryProbeStatus,
    interface_class: RecoveryNetworkClass,
    network_class: RecoveryNetworkClass,
    connectivity: RecoveryConnectivity,
    network_cost: RecoveryNetworkCost,
    roaming: RecoveryTruth,
    over_data_limit: RecoveryTruth,
    approaching_data_limit: RecoveryTruth,
    background_data_restricted: RecoveryTruth,
    power_source: RecoveryPowerSource,
    power_supply_status: RecoveryPowerSupplyStatus,
    battery_status: RecoveryBatteryStatus,
    energy_saver_status: RecoveryEnergySaverStatus,
    remaining_charge_percent: Option<u8>,
}

impl RecoveryPlatformContext {
    pub(crate) fn network_class(self) -> RecoveryNetworkClass {
        self.network_class
    }

    pub(crate) fn power_source(self) -> RecoveryPowerSource {
        self.power_source
    }

    pub(crate) fn refreshed(self) -> Self {
        match self.source {
            RecoveryPlatformContextSource::CallerSupplied => self,
            #[cfg(windows)]
            RecoveryPlatformContextSource::WindowsNative => system_recovery_platform_context(),
            #[cfg(not(windows))]
            RecoveryPlatformContextSource::UnsupportedPlatform => {
                system_recovery_platform_context()
            }
        }
    }

    fn caller_supplied(network: RecoveryNetworkClass, power: RecoveryPowerSource) -> Self {
        Self {
            source: RecoveryPlatformContextSource::CallerSupplied,
            network_probe_status: RecoveryProbeStatus::CallerSupplied,
            power_probe_status: RecoveryProbeStatus::CallerSupplied,
            interface_class: network,
            network_class: network,
            connectivity: RecoveryConnectivity::Unknown,
            network_cost: RecoveryNetworkCost::Unknown,
            roaming: RecoveryTruth::Unknown,
            over_data_limit: RecoveryTruth::Unknown,
            approaching_data_limit: RecoveryTruth::Unknown,
            background_data_restricted: RecoveryTruth::Unknown,
            power_source: power,
            power_supply_status: RecoveryPowerSupplyStatus::Unknown,
            battery_status: RecoveryBatteryStatus::Unknown,
            energy_saver_status: RecoveryEnergySaverStatus::Unknown,
            remaining_charge_percent: None,
        }
    }

    #[cfg(not(windows))]
    fn unsupported() -> Self {
        Self {
            source: RecoveryPlatformContextSource::UnsupportedPlatform,
            network_probe_status: RecoveryProbeStatus::Unavailable,
            power_probe_status: RecoveryProbeStatus::Unavailable,
            interface_class: RecoveryNetworkClass::Unknown,
            network_class: RecoveryNetworkClass::Unknown,
            connectivity: RecoveryConnectivity::Unknown,
            network_cost: RecoveryNetworkCost::Unknown,
            roaming: RecoveryTruth::Unknown,
            over_data_limit: RecoveryTruth::Unknown,
            approaching_data_limit: RecoveryTruth::Unknown,
            background_data_restricted: RecoveryTruth::Unknown,
            power_source: RecoveryPowerSource::Unknown,
            power_supply_status: RecoveryPowerSupplyStatus::Unknown,
            battery_status: RecoveryBatteryStatus::Unknown,
            energy_saver_status: RecoveryEnergySaverStatus::Unknown,
            remaining_charge_percent: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RecoveryPlatformChangeWait {
    Changed,
    TimedOut,
}

#[derive(Debug)]
struct RecoveryPlatformChangeSignal {
    sequence: AtomicU64,
    notify: Notify,
}

impl RecoveryPlatformChangeSignal {
    fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            notify: Notify::new(),
        }
    }

    fn sequence(&self) -> u64 {
        self.sequence.load(Ordering::Acquire)
    }

    fn changed(&self) {
        self.sequence.fetch_add(1, Ordering::AcqRel);
        self.notify.notify_waiters();
    }

    async fn wait(&self, observed_sequence: u64, duration: Duration) -> RecoveryPlatformChangeWait {
        if self.sequence() != observed_sequence {
            return RecoveryPlatformChangeWait::Changed;
        }
        let notified = self.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.sequence() != observed_sequence {
            return RecoveryPlatformChangeWait::Changed;
        }
        match tokio::time::timeout(duration, notified).await {
            Ok(()) => RecoveryPlatformChangeWait::Changed,
            Err(_) => RecoveryPlatformChangeWait::TimedOut,
        }
    }
}

pub(crate) struct RecoveryPlatformChangeSubscription {
    signal: Arc<RecoveryPlatformChangeSignal>,
    #[cfg(windows)]
    _windows: windows_events::WindowsRecoveryPlatformEvents,
}

impl RecoveryPlatformChangeSubscription {
    pub(crate) fn is_native(&self) -> bool {
        cfg!(windows)
    }

    pub(crate) fn sequence(&self) -> u64 {
        self.signal.sequence()
    }

    pub(crate) async fn wait(
        &self,
        observed_sequence: u64,
        duration: Duration,
    ) -> RecoveryPlatformChangeWait {
        self.signal.wait(observed_sequence, duration).await
    }
}

pub(crate) fn subscribe_recovery_platform_changes() -> Result<RecoveryPlatformChangeSubscription> {
    let signal = Arc::new(RecoveryPlatformChangeSignal::new());
    #[cfg(windows)]
    let windows = windows_events::WindowsRecoveryPlatformEvents::subscribe(signal.clone())?;
    Ok(RecoveryPlatformChangeSubscription {
        signal,
        #[cfg(windows)]
        _windows: windows,
    })
}

pub(crate) trait RecoveryPlatformContextProvider {
    fn probe(&self) -> RecoveryPlatformContext;
}

pub(crate) struct SystemRecoveryPlatformContextProvider;

impl RecoveryPlatformContextProvider for SystemRecoveryPlatformContextProvider {
    fn probe(&self) -> RecoveryPlatformContext {
        system_probe()
    }
}

pub(crate) fn system_recovery_platform_context() -> RecoveryPlatformContext {
    SystemRecoveryPlatformContextProvider.probe()
}

pub(crate) fn resolve_recovery_platform_context(
    network: Option<RecoveryNetworkClass>,
    power: Option<RecoveryPowerSource>,
) -> Result<RecoveryPlatformContext> {
    match (network, power) {
        (Some(network), Some(power)) => {
            Ok(RecoveryPlatformContext::caller_supplied(network, power))
        }
        (None, None) => Ok(system_recovery_platform_context()),
        _ => bail!("--network-class and --power-source must be supplied together or both omitted"),
    }
}

pub(crate) fn print_recovery_platform_context(context: &RecoveryPlatformContext) {
    println!(
        "history_recovery_platform_context_source={}",
        context.source.as_str()
    );
    println!(
        "history_recovery_network_context_source={}",
        context.source.as_str()
    );
    println!(
        "history_recovery_network_probe_status={}",
        context.network_probe_status.as_str()
    );
    println!(
        "history_recovery_power_probe_status={}",
        context.power_probe_status.as_str()
    );
    println!(
        "history_recovery_interface_network_class={}",
        context.interface_class.as_str()
    );
    println!(
        "history_recovery_current_network_class={}",
        context.network_class.as_str()
    );
    println!(
        "history_recovery_network_connectivity={}",
        context.connectivity.as_str()
    );
    println!(
        "history_recovery_network_cost={}",
        context.network_cost.as_str()
    );
    println!(
        "history_recovery_network_metered={}",
        RecoveryTruth::from_option(context.network_cost.is_metered()).as_str()
    );
    println!(
        "history_recovery_network_roaming={}",
        context.roaming.as_str()
    );
    println!(
        "history_recovery_network_over_data_limit={}",
        context.over_data_limit.as_str()
    );
    println!(
        "history_recovery_network_approaching_data_limit={}",
        context.approaching_data_limit.as_str()
    );
    println!(
        "history_recovery_network_background_data_restricted={}",
        context.background_data_restricted.as_str()
    );
    println!(
        "history_recovery_current_power_source={}",
        context.power_source.as_str()
    );
    println!(
        "history_recovery_power_supply_status={}",
        context.power_supply_status.as_str()
    );
    println!(
        "history_recovery_battery_status={}",
        context.battery_status.as_str()
    );
    println!(
        "history_recovery_energy_saver_status={}",
        context.energy_saver_status.as_str()
    );
    match context.remaining_charge_percent {
        Some(percent) => println!("history_recovery_remaining_charge_percent={percent}"),
        None => println!("history_recovery_remaining_charge_percent=unknown"),
    }
}

fn effective_network_class(
    interface_class: RecoveryNetworkClass,
    cost: RecoveryNetworkCost,
    roaming: RecoveryTruth,
) -> RecoveryNetworkClass {
    if cost == RecoveryNetworkCost::Unknown || roaming == RecoveryTruth::Unknown {
        return RecoveryNetworkClass::Unknown;
    }
    if matches!(
        cost,
        RecoveryNetworkCost::Fixed | RecoveryNetworkCost::Variable
    ) || roaming == RecoveryTruth::Yes
    {
        return RecoveryNetworkClass::Mobile;
    }
    interface_class
}

fn interface_network_class(
    is_wlan: Option<bool>,
    is_wwan: Option<bool>,
    iana_interface_type: Option<u32>,
) -> RecoveryNetworkClass {
    match (is_wlan, is_wwan) {
        (Some(true), Some(true)) => return RecoveryNetworkClass::Unknown,
        (_, Some(true)) => return RecoveryNetworkClass::Mobile,
        (Some(true), _) => return RecoveryNetworkClass::Wifi,
        _ => {}
    }
    match iana_interface_type {
        Some(IANA_ETHERNET_CSMACD) => RecoveryNetworkClass::Ethernet,
        Some(IANA_IEEE80211) => RecoveryNetworkClass::Wifi,
        Some(IANA_WWANPP | IANA_WWANPP2) => RecoveryNetworkClass::Mobile,
        _ => RecoveryNetworkClass::Unknown,
    }
}

fn classify_power_source(
    supply: RecoveryPowerSupplyStatus,
    battery: RecoveryBatteryStatus,
) -> RecoveryPowerSource {
    if supply == RecoveryPowerSupplyStatus::Adequate {
        return RecoveryPowerSource::External;
    }
    if matches!(
        battery,
        RecoveryBatteryStatus::Discharging | RecoveryBatteryStatus::Idle
    ) {
        return RecoveryPowerSource::Battery;
    }
    RecoveryPowerSource::Unknown
}

#[cfg(windows)]
fn system_probe() -> RecoveryPlatformContext {
    let network = windows_probe::probe_network();
    let power = windows_probe::probe_power();
    RecoveryPlatformContext {
        source: RecoveryPlatformContextSource::WindowsNative,
        network_probe_status: network.status,
        power_probe_status: power.status,
        interface_class: network.interface_class,
        network_class: network.network_class,
        connectivity: network.connectivity,
        network_cost: network.cost,
        roaming: network.roaming,
        over_data_limit: network.over_data_limit,
        approaching_data_limit: network.approaching_data_limit,
        background_data_restricted: network.background_data_restricted,
        power_source: power.power_source,
        power_supply_status: power.supply,
        battery_status: power.battery,
        energy_saver_status: power.energy_saver,
        remaining_charge_percent: power.remaining_charge_percent,
    }
}

#[cfg(not(windows))]
fn system_probe() -> RecoveryPlatformContext {
    RecoveryPlatformContext::unsupported()
}

#[cfg(windows)]
mod windows_probe {
    use windows::{
        Networking::Connectivity::{
            ConnectionProfile, NetworkConnectivityLevel, NetworkCostType, NetworkInformation,
        },
        System::Power::{BatteryStatus, EnergySaverStatus, PowerManager, PowerSupplyStatus},
    };

    use super::{
        RecoveryBatteryStatus, RecoveryConnectivity, RecoveryEnergySaverStatus,
        RecoveryNetworkCost, RecoveryPowerSupplyStatus, RecoveryProbeStatus, RecoveryTruth,
        classify_power_source, effective_network_class, interface_network_class,
    };
    use crate::recovery_plan::{RecoveryNetworkClass, RecoveryPowerSource};

    #[derive(Clone, Copy)]
    pub(super) struct WindowsNetworkProbe {
        pub status: RecoveryProbeStatus,
        pub interface_class: RecoveryNetworkClass,
        pub network_class: RecoveryNetworkClass,
        pub connectivity: RecoveryConnectivity,
        pub cost: RecoveryNetworkCost,
        pub roaming: RecoveryTruth,
        pub over_data_limit: RecoveryTruth,
        pub approaching_data_limit: RecoveryTruth,
        pub background_data_restricted: RecoveryTruth,
    }

    pub(super) struct WindowsPowerProbe {
        pub status: RecoveryProbeStatus,
        pub power_source: RecoveryPowerSource,
        pub supply: RecoveryPowerSupplyStatus,
        pub battery: RecoveryBatteryStatus,
        pub energy_saver: RecoveryEnergySaverStatus,
        pub remaining_charge_percent: Option<u8>,
    }

    pub(super) fn probe_network() -> WindowsNetworkProbe {
        let primary = NetworkInformation::GetInternetConnectionProfile()
            .ok()
            .map(|profile| probe_profile(&profile));
        if let Some(primary_probe) = primary
            && primary_probe.interface_class != RecoveryNetworkClass::Unknown
            && primary_probe.status == RecoveryProbeStatus::Available
        {
            return primary_probe;
        }

        let mut internet_profiles = Vec::new();
        let mut local_profiles = Vec::new();
        if let Ok(profiles) = NetworkInformation::GetConnectionProfiles() {
            for profile in &profiles {
                let probe = probe_profile(&profile);
                if probe.interface_class == RecoveryNetworkClass::Unknown {
                    continue;
                }
                match probe.connectivity {
                    RecoveryConnectivity::Internet => internet_profiles.push(probe),
                    RecoveryConnectivity::Local | RecoveryConnectivity::Constrained => {
                        local_profiles.push(probe);
                    }
                    RecoveryConnectivity::None | RecoveryConnectivity::Unknown => {}
                }
            }
        }
        if internet_profiles.len() == 1 {
            return internet_profiles.remove(0);
        }
        if internet_profiles.is_empty() && local_profiles.len() == 1 {
            return local_profiles.remove(0);
        }
        match primary {
            Some(primary_probe) => primary_probe,
            None => unavailable_network(),
        }
    }

    fn probe_profile(profile: &ConnectionProfile) -> WindowsNetworkProbe {
        let is_wlan = profile.IsWlanConnectionProfile().ok();
        let is_wwan = profile.IsWwanConnectionProfile().ok();
        let iana_interface_type = profile
            .NetworkAdapter()
            .and_then(|adapter| adapter.IanaInterfaceType())
            .ok();
        let interface_class = interface_network_class(is_wlan, is_wwan, iana_interface_type);
        let connectivity = profile
            .GetNetworkConnectivityLevel()
            .map(map_connectivity)
            .unwrap_or(RecoveryConnectivity::Unknown);
        let connection_cost = profile.GetConnectionCost().ok();
        let cost = connection_cost
            .as_ref()
            .and_then(|value| value.NetworkCostType().ok())
            .map(map_cost)
            .unwrap_or(RecoveryNetworkCost::Unknown);
        let roaming = RecoveryTruth::from_option(
            connection_cost
                .as_ref()
                .and_then(|value| value.Roaming().ok()),
        );
        let over_data_limit = RecoveryTruth::from_option(
            connection_cost
                .as_ref()
                .and_then(|value| value.OverDataLimit().ok()),
        );
        let approaching_data_limit = RecoveryTruth::from_option(
            connection_cost
                .as_ref()
                .and_then(|value| value.ApproachingDataLimit().ok()),
        );
        let background_data_restricted = RecoveryTruth::from_option(
            connection_cost
                .as_ref()
                .and_then(|value| value.BackgroundDataUsageRestricted().ok()),
        );
        let network_class = if matches!(
            connectivity,
            RecoveryConnectivity::None | RecoveryConnectivity::Unknown
        ) {
            RecoveryNetworkClass::Unknown
        } else {
            effective_network_class(interface_class, cost, roaming)
        };
        let critical_available = network_class != RecoveryNetworkClass::Unknown
            && cost != RecoveryNetworkCost::Unknown
            && roaming != RecoveryTruth::Unknown;
        WindowsNetworkProbe {
            status: if critical_available {
                RecoveryProbeStatus::Available
            } else {
                RecoveryProbeStatus::Unavailable
            },
            interface_class,
            network_class,
            connectivity,
            cost,
            roaming,
            over_data_limit,
            approaching_data_limit,
            background_data_restricted,
        }
    }

    pub(super) fn probe_power() -> WindowsPowerProbe {
        let supply = PowerManager::PowerSupplyStatus()
            .map(map_supply)
            .unwrap_or(RecoveryPowerSupplyStatus::Unknown);
        let battery = PowerManager::BatteryStatus()
            .map(map_battery)
            .unwrap_or(RecoveryBatteryStatus::Unknown);
        let energy_saver = PowerManager::EnergySaverStatus()
            .map(map_energy_saver)
            .unwrap_or(RecoveryEnergySaverStatus::Unknown);
        let remaining_charge_percent = if battery == RecoveryBatteryStatus::NotPresent {
            None
        } else {
            PowerManager::RemainingChargePercent()
                .ok()
                .and_then(|percent| u8::try_from(percent).ok())
                .filter(|percent| *percent <= 100)
        };
        let power_source = classify_power_source(supply, battery);
        WindowsPowerProbe {
            status: if supply != RecoveryPowerSupplyStatus::Unknown
                && battery != RecoveryBatteryStatus::Unknown
            {
                RecoveryProbeStatus::Available
            } else {
                RecoveryProbeStatus::Unavailable
            },
            power_source,
            supply,
            battery,
            energy_saver,
            remaining_charge_percent,
        }
    }

    fn unavailable_network() -> WindowsNetworkProbe {
        WindowsNetworkProbe {
            status: RecoveryProbeStatus::Unavailable,
            interface_class: RecoveryNetworkClass::Unknown,
            network_class: RecoveryNetworkClass::Unknown,
            connectivity: RecoveryConnectivity::Unknown,
            cost: RecoveryNetworkCost::Unknown,
            roaming: RecoveryTruth::Unknown,
            over_data_limit: RecoveryTruth::Unknown,
            approaching_data_limit: RecoveryTruth::Unknown,
            background_data_restricted: RecoveryTruth::Unknown,
        }
    }

    fn map_connectivity(value: NetworkConnectivityLevel) -> RecoveryConnectivity {
        match value {
            NetworkConnectivityLevel::None => RecoveryConnectivity::None,
            NetworkConnectivityLevel::LocalAccess => RecoveryConnectivity::Local,
            NetworkConnectivityLevel::ConstrainedInternetAccess => {
                RecoveryConnectivity::Constrained
            }
            NetworkConnectivityLevel::InternetAccess => RecoveryConnectivity::Internet,
            _ => RecoveryConnectivity::Unknown,
        }
    }

    fn map_cost(value: NetworkCostType) -> RecoveryNetworkCost {
        match value {
            NetworkCostType::Unrestricted => RecoveryNetworkCost::Unrestricted,
            NetworkCostType::Fixed => RecoveryNetworkCost::Fixed,
            NetworkCostType::Variable => RecoveryNetworkCost::Variable,
            _ => RecoveryNetworkCost::Unknown,
        }
    }

    fn map_supply(value: PowerSupplyStatus) -> RecoveryPowerSupplyStatus {
        match value {
            PowerSupplyStatus::NotPresent => RecoveryPowerSupplyStatus::NotPresent,
            PowerSupplyStatus::Inadequate => RecoveryPowerSupplyStatus::Inadequate,
            PowerSupplyStatus::Adequate => RecoveryPowerSupplyStatus::Adequate,
            _ => RecoveryPowerSupplyStatus::Unknown,
        }
    }

    fn map_battery(value: BatteryStatus) -> RecoveryBatteryStatus {
        match value {
            BatteryStatus::NotPresent => RecoveryBatteryStatus::NotPresent,
            BatteryStatus::Discharging => RecoveryBatteryStatus::Discharging,
            BatteryStatus::Idle => RecoveryBatteryStatus::Idle,
            BatteryStatus::Charging => RecoveryBatteryStatus::Charging,
            _ => RecoveryBatteryStatus::Unknown,
        }
    }

    fn map_energy_saver(value: EnergySaverStatus) -> RecoveryEnergySaverStatus {
        match value {
            EnergySaverStatus::Disabled => RecoveryEnergySaverStatus::Disabled,
            EnergySaverStatus::Off => RecoveryEnergySaverStatus::Off,
            EnergySaverStatus::On => RecoveryEnergySaverStatus::On,
            _ => RecoveryEnergySaverStatus::Unknown,
        }
    }
}

#[cfg(windows)]
mod windows_events {
    use std::sync::Arc;

    use anyhow::{Context, Result};
    use windows::{
        Foundation::EventHandler,
        Networking::Connectivity::{NetworkInformation, NetworkStatusChangedEventHandler},
        System::Power::PowerManager,
        core::IInspectable,
    };

    use super::RecoveryPlatformChangeSignal;

    pub(super) struct WindowsRecoveryPlatformEvents {
        _network: NetworkRegistration,
        _power_supply: PowerRegistration,
        _battery: PowerRegistration,
        _energy_saver: PowerRegistration,
    }

    impl WindowsRecoveryPlatformEvents {
        pub(super) fn subscribe(signal: Arc<RecoveryPlatformChangeSignal>) -> Result<Self> {
            Ok(Self {
                _network: NetworkRegistration::subscribe(signal.clone())?,
                _power_supply: PowerRegistration::subscribe(
                    signal.clone(),
                    PowerEventKind::Supply,
                )?,
                _battery: PowerRegistration::subscribe(signal.clone(), PowerEventKind::Battery)?,
                _energy_saver: PowerRegistration::subscribe(signal, PowerEventKind::EnergySaver)?,
            })
        }
    }

    struct NetworkRegistration {
        token: i64,
        _handler: NetworkStatusChangedEventHandler,
    }

    impl NetworkRegistration {
        fn subscribe(signal: Arc<RecoveryPlatformChangeSignal>) -> Result<Self> {
            let handler = NetworkStatusChangedEventHandler::new(move |_| {
                signal.changed();
                Ok(())
            });
            let token = NetworkInformation::NetworkStatusChanged(&handler)
                .context("subscribe to Windows network status changes")?;
            Ok(Self {
                token,
                _handler: handler,
            })
        }
    }

    impl Drop for NetworkRegistration {
        fn drop(&mut self) {
            let _ = NetworkInformation::RemoveNetworkStatusChanged(self.token);
        }
    }

    #[derive(Clone, Copy)]
    enum PowerEventKind {
        Supply,
        Battery,
        EnergySaver,
    }

    struct PowerRegistration {
        kind: PowerEventKind,
        token: i64,
        _handler: EventHandler<IInspectable>,
    }

    impl PowerRegistration {
        fn subscribe(
            signal: Arc<RecoveryPlatformChangeSignal>,
            kind: PowerEventKind,
        ) -> Result<Self> {
            let handler = EventHandler::<IInspectable>::new(move |_, _| {
                signal.changed();
                Ok(())
            });
            let token = match kind {
                PowerEventKind::Supply => PowerManager::PowerSupplyStatusChanged(&handler)
                    .context("subscribe to Windows power supply changes")?,
                PowerEventKind::Battery => PowerManager::BatteryStatusChanged(&handler)
                    .context("subscribe to Windows battery changes")?,
                PowerEventKind::EnergySaver => PowerManager::EnergySaverStatusChanged(&handler)
                    .context("subscribe to Windows energy saver changes")?,
            };
            Ok(Self {
                kind,
                token,
                _handler: handler,
            })
        }
    }

    impl Drop for PowerRegistration {
        fn drop(&mut self) {
            let _ = match self.kind {
                PowerEventKind::Supply => PowerManager::RemovePowerSupplyStatusChanged(self.token),
                PowerEventKind::Battery => PowerManager::RemoveBatteryStatusChanged(self.token),
                PowerEventKind::EnergySaver => {
                    PowerManager::RemoveEnergySaverStatusChanged(self.token)
                }
            };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn platform_change_signal_observes_changes_without_losing_wakeups() {
        let signal = RecoveryPlatformChangeSignal::new();
        let observed = signal.sequence();
        signal.changed();
        assert_eq!(
            signal.wait(observed, Duration::from_secs(1)).await,
            RecoveryPlatformChangeWait::Changed
        );
        assert_eq!(
            signal
                .wait(signal.sequence(), Duration::from_millis(1))
                .await,
            RecoveryPlatformChangeWait::TimedOut
        );
    }

    #[test]
    fn metered_or_roaming_networks_use_the_mobile_policy_bucket() {
        assert_eq!(
            effective_network_class(
                RecoveryNetworkClass::Wifi,
                RecoveryNetworkCost::Fixed,
                RecoveryTruth::No,
            ),
            RecoveryNetworkClass::Mobile
        );
        assert_eq!(
            effective_network_class(
                RecoveryNetworkClass::Ethernet,
                RecoveryNetworkCost::Unrestricted,
                RecoveryTruth::Yes,
            ),
            RecoveryNetworkClass::Mobile
        );
        assert_eq!(
            effective_network_class(
                RecoveryNetworkClass::Ethernet,
                RecoveryNetworkCost::Unknown,
                RecoveryTruth::No,
            ),
            RecoveryNetworkClass::Unknown
        );
    }

    #[test]
    fn interface_classification_is_exact_and_fails_closed() {
        assert_eq!(
            interface_network_class(Some(true), Some(false), Some(IANA_IEEE80211)),
            RecoveryNetworkClass::Wifi
        );
        assert_eq!(
            interface_network_class(Some(false), Some(false), Some(IANA_ETHERNET_CSMACD)),
            RecoveryNetworkClass::Ethernet
        );
        assert_eq!(
            interface_network_class(Some(false), Some(false), Some(131)),
            RecoveryNetworkClass::Unknown
        );
        assert_eq!(
            interface_network_class(Some(true), Some(true), Some(IANA_IEEE80211)),
            RecoveryNetworkClass::Unknown
        );
    }

    #[test]
    fn power_classification_requires_an_adequate_supply_or_present_battery() {
        assert_eq!(
            classify_power_source(
                RecoveryPowerSupplyStatus::Adequate,
                RecoveryBatteryStatus::Charging,
            ),
            RecoveryPowerSource::External
        );
        assert_eq!(
            classify_power_source(
                RecoveryPowerSupplyStatus::NotPresent,
                RecoveryBatteryStatus::Discharging,
            ),
            RecoveryPowerSource::Battery
        );
        assert_eq!(
            classify_power_source(
                RecoveryPowerSupplyStatus::Inadequate,
                RecoveryBatteryStatus::Charging,
            ),
            RecoveryPowerSource::Unknown
        );
    }

    #[test]
    fn manual_context_requires_both_values() -> Result<()> {
        let context = resolve_recovery_platform_context(
            Some(RecoveryNetworkClass::Wifi),
            Some(RecoveryPowerSource::Battery),
        )?;
        assert_eq!(
            context.source,
            RecoveryPlatformContextSource::CallerSupplied
        );
        assert_eq!(context.network_class(), RecoveryNetworkClass::Wifi);
        assert_eq!(context.power_source(), RecoveryPowerSource::Battery);
        assert!(resolve_recovery_platform_context(Some(RecoveryNetworkClass::Wifi), None).is_err());
        assert!(
            resolve_recovery_platform_context(None, Some(RecoveryPowerSource::Battery)).is_err()
        );
        Ok(())
    }

    #[cfg(not(windows))]
    #[test]
    fn unsupported_system_probe_is_explicit_and_unknown() {
        let context = system_recovery_platform_context();
        assert_eq!(
            context.source,
            RecoveryPlatformContextSource::UnsupportedPlatform
        );
        assert_eq!(context.network_class(), RecoveryNetworkClass::Unknown);
        assert_eq!(context.power_source(), RecoveryPowerSource::Unknown);
    }
}
