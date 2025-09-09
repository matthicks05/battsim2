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
}

/// Cell-level monitoring data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellMonitoring {
    pub voltage_min: f64,    // Minimum cell voltage (V)
    pub voltage_max: f64,    // Maximum cell voltage (V)
    pub voltage_avg: f64,    // Average cell voltage (V)
    pub temp_min: f64,       // Minimum cell temperature (°C)
    pub temp_max: f64,       // Maximum cell temperature (°C)
    pub temp_avg: f64,       // Average cell temperature (°C)
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
}

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
}

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
    pub power_setpoint: f64,          // Power command (kW)
    pub current_limit_charge: f64,    // Max charge current (A)
    pub current_limit_discharge: f64, // Max discharge current (A)
    pub voltage_limit_high: f64,      // Max charge voltage (V)
    pub voltage_limit_low: f64,       // Min discharge voltage (V)
    pub soc_limit_high: f64,         // Max SOC target (%)
    pub soc_limit_low: f64,          // Min SOC target (%)
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
}

/// Complete battery state combining all data
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BatteryState {
    pub electrical: ElectricalParams,
    pub cells: CellMonitoring,
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
        for i in 0..8 {
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
        for i in 0..6 {
            if bitmap & (1 << i) != 0 {
                match i {
                    0 => warnings.push(WarningCode::HighSocWarning),
                    1 => warnings.push(WarningCode::LowSocWarning),
                    2 => warnings.push(WarningCode::HighTemperatureWarning),
                    3 => warnings.push(WarningCode::CellImbalanceWarning),
                    4 => warnings.push(WarningCode::ReducedPerformance),
                    5 => warnings.push(WarningCode::MaintenanceDue),
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
        }
    }
}

impl Default for CellMonitoring {
    fn default() -> Self {
        Self {
            voltage_min: 3.20,
            voltage_max: 3.24,
            voltage_avg: 3.22,
            temp_min: 25.0,
            temp_max: 30.0,
            temp_avg: 27.5,
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
        }
    }
}

