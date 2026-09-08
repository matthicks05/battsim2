use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Electrical parameters for battery monitoring and control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectricalParams {
    pub voltage: f64,           // Battery DC voltage (V)
    pub current: f64,           // Current in Amperes (+ = discharge, - = charge)
    pub power: f64,             // Instantaneous power (kW)
    pub soc: f64,              // State of charge (%)
    pub available_energy: f64,  // Energy available for discharge (kWh)
    pub remaining_capacity: f64, // Energy capacity remaining to full (kWh)
    pub max_charge_power: f64,      // Dynamic charge power limit (kW), SOC/thermal derated
    pub max_discharge_power: f64,   // Dynamic discharge power limit (kW), SOC/thermal derated
    pub lifetime_charge_energy: f64,    // Cumulative energy charged into the battery (kWh)
    pub lifetime_discharge_energy: f64, // Cumulative energy discharged from the battery (kWh)
}

/// Cell-level monitoring data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellMonitoring {
    pub voltage_min: f64,    // Minimum cell voltage (V)
    pub voltage_max: f64,    // Maximum cell voltage (V)
    pub voltage_avg: f64,    // Average cell voltage (V)
    pub voltage_delta: f64,  // Max - min cell voltage (V), imbalance indicator
    pub temp_min: f64,       // Minimum cell temperature (°C)
    pub temp_max: f64,       // Maximum cell temperature (°C)
    pub temp_avg: f64,       // Average cell temperature (°C)
    pub balancing_cells: u16, // Number of cells currently in active balancing
}

/// AC-side parameters as measured at the Power Conversion System (PCS)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcParams {
    pub pcs_state: PcsState,     // PCS operating sub-state
    pub grid_forming: bool,      // true = grid-forming, false = grid-following
    pub frequency: f64,          // Grid frequency (Hz)
    pub power_factor: f64,       // -1.0 to 1.0
    pub real_power: f64,         // AC real power (kW), + = export/discharge
    pub reactive_power: f64,     // AC reactive power (kVAR)
    pub apparent_power: f64,     // AC apparent power (kVA)
    pub voltage_l1n: f64,        // Line-to-neutral voltage, phase 1 (V)
    pub voltage_l2n: f64,        // Line-to-neutral voltage, phase 2 (V)
    pub voltage_l3n: f64,        // Line-to-neutral voltage, phase 3 (V)
    pub current_l1: f64,         // Phase 1 current (A)
    pub current_l2: f64,         // Phase 2 current (A)
    pub current_l3: f64,         // Phase 3 current (A)
    pub pcs_temperature: f64,    // PCS heatsink/enclosure temperature (°C)
    pub isolation_resistance: f64, // DC bus isolation resistance (kOhm)
}

/// Grid/POI meter parameters (site export/import, distinct from PCS output)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeterParams {
    pub voltage_l1n: f64,   // V
    pub voltage_l2n: f64,   // V
    pub voltage_l3n: f64,   // V
    pub current_l1: f64,    // A
    pub current_l2: f64,    // A
    pub current_l3: f64,    // A
    pub frequency: f64,     // Hz
    pub real_power: f64,    // kW, + = export to grid, - = import from grid
    pub reactive_power: f64, // kVAR
    pub lifetime_import_energy: f64, // kWh, cumulative energy imported from grid
    pub lifetime_export_energy: f64, // kWh, cumulative energy exported to grid
    pub aux_load_power: f64, // kW, site auxiliary/house load subtracted from PCS output
}

/// System operational status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub state: SystemState,         // Current operating state
    pub online: bool,              // Modbus connection status
    pub faults: Vec<FaultCode>,    // Active system faults
    pub warnings: Vec<WarningCode>, // Active system warnings
    pub modbus_clients: u32,       // Number of connected clients
    pub uptime: Duration,          // System uptime
    pub grid_connected: bool,      // Main breaker / grid connection status
    pub operating_mode: OperatingMode, // Auto / Manual / Maintenance
}

/// Additional system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub soh: f64,                  // State of health (%)
    pub cycle_count: u32,          // Total equivalent full cycles
    pub internal_resistance: f64,   // Battery internal resistance (mΩ)
    pub operating_hours: u32,      // Total runtime in hours
    pub contactor_status: u16,     // Contactor states bitmap
    pub cooling_status: CoolingStatus, // Cooling system status
    pub ambient_temperature: f64,   // Ambient temperature (°C)
    pub enclosure_temperature: f64, // Enclosure/container temperature (°C)
    pub aux_power_consumption: f64, // Auxiliary/house load power draw (kW)
    pub fire_suppression_status: FireSuppressionStatus,
    pub smoke_detected: bool,
}

/// Battery system operating states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum SystemState {
    Offline = 0,
    Standby = 1,
    Charging = 2,
    Discharging = 3,
    Fault = 4,
    Maintenance = 5,
}

/// PCS operating sub-state (mirrors a typical inverter start-up sequence)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum PcsState {
    Standby = 0,
    Precharge = 1,
    SoftStart = 2,
    Running = 3,
    Fault = 4,
}

/// Operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum OperatingMode {
    Auto = 0,
    Manual = 1,
    Maintenance = 2,
}

/// Fire suppression system status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum FireSuppressionStatus {
    Normal = 0,
    Alarm = 1,
    Discharged = 2,
}

/// System commands for control operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum SystemCommand {
    NoCommand = 0,
    Start = 1,
    Stop = 2,
    EmergencyStop = 3,
    ResetFaults = 4,
    MaintenanceMode = 5,
}

/// Fault codes as individual flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum FaultCode {
    OverVoltage = 0,
    UnderVoltage = 1,
    OverCurrent = 2,
    OverTemperature = 3,
    UnderTemperature = 4,
    CommunicationFault = 5,
    ContactorFault = 6,
    InternalFault = 7,
    GridFault = 8,
    IsolationFault = 9,
    FireAlarm = 10,
    SmokeDetected = 11,
    BmsCommunicationFault = 12,
    PcsCommunicationFault = 13,
    CellImbalanceFault = 14,
    WatchdogTimeout = 15,
}

/// Total number of defined fault code bits (keep in sync with FaultCode variants)
const FAULT_CODE_COUNT: u8 = 16;

/// Warning codes as individual flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
pub enum WarningCode {
    HighSocWarning = 0,     // >95%
    LowSocWarning = 1,      // <10%
    HighTemperatureWarning = 2,
    CellImbalanceWarning = 3,
    ReducedPerformance = 4,
    MaintenanceDue = 5,
    GridFrequencyWarning = 6,
    GridVoltageWarning = 7,
    AuxPowerWarning = 8,
    LowInsulationWarning = 9,
}

/// Total number of defined warning code bits (keep in sync with WarningCode variants)
const WARNING_CODE_COUNT: u8 = 10;

/// Cooling system operational states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u16)]
pub enum CoolingStatus {
    Offline = 0,
    Standby = 1,
    Active = 2,
    Fault = 3,
}

/// Control setpoints for battery operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlSetpoints {
    pub command: SystemCommand,        // System command
    pub power_setpoint: f64,          // Active power command (kW)
    pub current_limit_charge: f64,    // Max charge current (A)
    pub current_limit_discharge: f64, // Max discharge current (A)
    pub voltage_limit_high: f64,      // Max charge voltage (V)
    pub voltage_limit_low: f64,       // Min discharge voltage (V)
    pub soc_limit_high: f64,         // Max SOC target (%)
    pub soc_limit_low: f64,          // Min SOC target (%)
    pub reactive_power_setpoint: f64, // Reactive power command (kVAR)
    pub power_factor_setpoint: f64,   // Target power factor (-1.0 to 1.0)
    pub operating_mode: OperatingMode, // Requested operating mode
    pub grid_connect_command: bool,   // Request main breaker close/open
    pub watchdog_timeout: u16,        // EMS comms watchdog timeout (s)
}

/// Battery configuration parameters
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BatteryConfig {
    pub rated_capacity: f64,    // System capacity (kWh)
    pub rated_power: f64,       // System power rating (kW)
    pub cell_count: u16,        // Number of cells
    pub simulation_speed: f64,  // Time multiplier
    pub nominal_voltage: f64,   // Nominal system voltage (V)
    pub max_voltage: f64,       // Maximum voltage (V)
    pub min_voltage: f64,       // Minimum voltage (V)
    pub rated_reactive_power: f64, // Reactive power rating (kVAR)
    pub rated_ac_voltage: f64,     // Nominal AC line-to-line voltage (V)
    pub nominal_frequency: f64,    // Nominal grid frequency (Hz)
}

/// Complete battery state combining all data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryState {
    pub electrical: ElectricalParams,
    pub cells: CellMonitoring,
    pub ac: AcParams,
    pub meter: MeterParams,
    pub status: SystemStatus,
    pub info: SystemInfo,
    pub setpoints: ControlSetpoints,
    pub config: BatteryConfig,
}

/// Utility functions for fault and warning bitmaps
impl FaultCode {
    pub fn to_bitmap(faults: &[FaultCode]) -> u32 {
        faults.iter().fold(0u32, |acc, &fault| acc | (1 << fault as u8))
    }

    pub fn from_bitmap(bitmap: u32) -> Vec<FaultCode> {
        let mut faults = Vec::new();
        for i in 0..FAULT_CODE_COUNT {
            if bitmap & (1 << i) != 0 {
                match i {
                    0 => faults.push(FaultCode::OverVoltage),
                    1 => faults.push(FaultCode::UnderVoltage),
                    2 => faults.push(FaultCode::OverCurrent),
                    3 => faults.push(FaultCode::OverTemperature),
                    4 => faults.push(FaultCode::UnderTemperature),
                    5 => faults.push(FaultCode::CommunicationFault),
                    6 => faults.push(FaultCode::ContactorFault),
                    7 => faults.push(FaultCode::InternalFault),
                    8 => faults.push(FaultCode::GridFault),
                    9 => faults.push(FaultCode::IsolationFault),
                    10 => faults.push(FaultCode::FireAlarm),
                    11 => faults.push(FaultCode::SmokeDetected),
                    12 => faults.push(FaultCode::BmsCommunicationFault),
                    13 => faults.push(FaultCode::PcsCommunicationFault),
                    14 => faults.push(FaultCode::CellImbalanceFault),
                    15 => faults.push(FaultCode::WatchdogTimeout),
                    _ => {}
                }
            }
        }
        faults
    }
}

impl WarningCode {
    pub fn to_bitmap(warnings: &[WarningCode]) -> u32 {
        warnings.iter().fold(0u32, |acc, &warning| acc | (1 << warning as u8))
    }

    pub fn from_bitmap(bitmap: u32) -> Vec<WarningCode> {
        let mut warnings = Vec::new();
        for i in 0..WARNING_CODE_COUNT {
            if bitmap & (1 << i) != 0 {
                match i {
                    0 => warnings.push(WarningCode::HighSocWarning),
                    1 => warnings.push(WarningCode::LowSocWarning),
                    2 => warnings.push(WarningCode::HighTemperatureWarning),
                    3 => warnings.push(WarningCode::CellImbalanceWarning),
                    4 => warnings.push(WarningCode::ReducedPerformance),
                    5 => warnings.push(WarningCode::MaintenanceDue),
                    6 => warnings.push(WarningCode::GridFrequencyWarning),
                    7 => warnings.push(WarningCode::GridVoltageWarning),
                    8 => warnings.push(WarningCode::AuxPowerWarning),
                    9 => warnings.push(WarningCode::LowInsulationWarning),
                    _ => {}
                }
            }
        }
        warnings
    }
}

/// Default implementations
impl Default for ElectricalParams {
    fn default() -> Self {
        Self {
            voltage: 400.0,
            current: 0.0,
            power: 0.0,
            soc: 85.0,
            available_energy: 425.0,  // 85% of 500 kWh
            remaining_capacity: 75.0,  // 15% of 500 kWh
            max_charge_power: 250.0,
            max_discharge_power: 250.0,
            lifetime_charge_energy: 0.0,
            lifetime_discharge_energy: 0.0,
        }
    }
}

impl Default for CellMonitoring {
    fn default() -> Self {
        Self {
            voltage_min: 3.20,
            voltage_max: 3.24,
            voltage_avg: 3.22,
            voltage_delta: 0.04,
            temp_min: 25.0,
            temp_max: 30.0,
            temp_avg: 27.5,
            balancing_cells: 0,
        }
    }
}

impl Default for AcParams {
    fn default() -> Self {
        Self {
            pcs_state: PcsState::Standby,
            grid_forming: false,
            frequency: 60.0,
            power_factor: 1.0,
            real_power: 0.0,
            reactive_power: 0.0,
            apparent_power: 0.0,
            voltage_l1n: 277.0,
            voltage_l2n: 277.0,
            voltage_l3n: 277.0,
            current_l1: 0.0,
            current_l2: 0.0,
            current_l3: 0.0,
            pcs_temperature: 25.0,
            isolation_resistance: 500.0,
        }
    }
}

impl Default for MeterParams {
    fn default() -> Self {
        Self {
            voltage_l1n: 277.0,
            voltage_l2n: 277.0,
            voltage_l3n: 277.0,
            current_l1: 0.0,
            current_l2: 0.0,
            current_l3: 0.0,
            frequency: 60.0,
            real_power: 0.0,
            reactive_power: 0.0,
            lifetime_import_energy: 0.0,
            lifetime_export_energy: 0.0,
            aux_load_power: 0.0,
        }
    }
}

impl Default for SystemStatus {
    fn default() -> Self {
        Self {
            state: SystemState::Offline, // Start in offline state
            online: false, // Consistent with offline state
            faults: Vec::new(),
            warnings: Vec::new(),
            modbus_clients: 0,
            uptime: Duration::from_secs(0),
            grid_connected: false,
            operating_mode: OperatingMode::Auto,
        }
    }
}

impl Default for SystemInfo {
    fn default() -> Self {
        Self {
            soh: 98.5,
            cycle_count: 0,
            internal_resistance: 64.0, // 0.5 mΩ per cell * 128 cells
            operating_hours: 0,
            contactor_status: 0,
            cooling_status: CoolingStatus::Standby,
            ambient_temperature: 25.0,
            enclosure_temperature: 28.0,
            aux_power_consumption: 0.0,
            fire_suppression_status: FireSuppressionStatus::Normal,
            smoke_detected: false,
        }
    }
}

impl Default for ControlSetpoints {
    fn default() -> Self {
        Self {
            command: SystemCommand::NoCommand,
            power_setpoint: 0.0,
            current_limit_charge: 625.0,
            current_limit_discharge: 625.0,
            voltage_limit_high: 450.0,
            voltage_limit_low: 350.0,
            soc_limit_high: 95.0,
            soc_limit_low: 10.0,
            reactive_power_setpoint: 0.0,
            power_factor_setpoint: 1.0,
            operating_mode: OperatingMode::Auto,
            grid_connect_command: true,
            watchdog_timeout: 30,
        }
    }
}

impl Default for BatteryConfig {
    fn default() -> Self {
        Self {
            rated_capacity: 500.0,
            rated_power: 250.0,
            cell_count: 128,
            simulation_speed: 1.0,
            nominal_voltage: 400.0,
            max_voltage: 450.0,
            min_voltage: 350.0,
            rated_reactive_power: 125.0,
            rated_ac_voltage: 480.0,
            nominal_frequency: 60.0,
        }
    }
}
