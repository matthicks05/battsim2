# Battery Simulator Specification v2.0

## Overview
This document defines the specifications for a lithium-ion battery energy storage system (BESS) simulator designed for stationary storage applications. The simulator provides realistic battery behavior with dynamic physics modeling, accessible via Modbus TCP protocol and a comprehensive terminal user interface.

## Physical Battery Specifications

### System Configuration
- **Battery Chemistry**: Lithium Iron Phosphate (LiFePO4)
- **System Capacity**: 500 kWh (configurable: 100-1000 kWh)
- **System Voltage**: 400V DC nominal (350-450V operating range)
- **Cell Configuration**: 128S1P (128 cells in series)
- **Individual Cell**: 3.2V nominal, ~3906Ah capacity (500kWh/128 cells)

### Electrical Characteristics

#### Voltage Parameters
- **Nominal Voltage**: 400V DC
- **Operating Range**: 350V - 450V DC
- **Max Charge Voltage**: 450V DC
- **Min Discharge Voltage**: 350V DC
- **Cell Voltage Range**: 2.7V - 3.6V per cell
- **Cell Voltage (Normal)**: 3.0V - 3.5V per cell

#### Current and Power Limits
- **Max Continuous Power**: 250 kW
- **Max Charge Current**: 625A (1.25C)
- **Max Discharge Current**: 625A (1.25C)
- **Standby Current**: < 5A

#### Performance Specifications
- **Round-trip Efficiency**: Modeled with I²R losses
- **Self-discharge Rate**: 3% per month (modeled)
- **Cycle Life**: 6000 cycles @ 80% DOD
- **Calendar Life**: 20 years

## Dynamic Characteristics

### State Variables
- **State of Charge (SOC)**: 0-100% (0.1% resolution)
- **State of Health (SOH)**: 100% degrading over time (50-100% range)
- **Cycle Count**: Cumulative full-equivalent cycles
- **Internal Resistance**: Dynamic based on SOC, temperature, and aging

### Temperature Management
- **Operating Temperature**: -10°C to +50°C
- **Cell Temperature Variation**: ±2°C around average
- **Thermal Derating**: Power reduction above 45°C
- **Default Temperature**: 25°C ambient, varies with current

### Internal Resistance Model
The simulator implements a sophisticated resistance model:

```rust
// Base resistance: 0.5 mΩ per cell (new battery)
let base_resistance = 0.5; // mΩ per cell

// SOC dependency (higher resistance at extremes)
let soc_factor = if soc < 10.0 {
    1.0 + (10.0 - soc) * 0.1      // Up to +10% below 10% SOC
} else if soc > 90.0 {
    1.0 + (soc - 90.0) * 0.05     // Up to +5% above 90% SOC
} else {
    1.0
};

// Temperature dependency (25°C reference)
let temp_factor = if temp < 25.0 {
    1.0 + (25.0 - temp) * 0.02    // +2% per degree below 25°C
} else {
    1.0
};

// Aging factor based on SOH
let aging_factor = 100.0 / soh;

// Total resistance = base × factors × cell_count
total_resistance = base_resistance * soc_factor * temp_factor * aging_factor * 128.0;
```

### Aging Model
The simulator includes realistic aging effects:

#### Calendar Aging
- **Base Rate**: 0.05% SOH loss per month at 25°C
- **Temperature Acceleration**: +2% rate per degree above 25°C
- **Continuous**: Applied every simulation step

#### Cycle Aging
- **Rate**: 0.2% SOH loss over 6000 full cycles
- **Threshold**: Only counts energy throughput > 10% of capacity
- **Accumulated**: Tracks partial cycles and sums to full equivalents

## Safety and Protection Systems

### Protection Limits (Implemented)
- **Over-voltage Protection**: 
  - System: 450V
  - Cell: 3.65V per cell
- **Under-voltage Protection**: 
  - System: 350V
  - Cell: 2.5V per cell
- **Over-current Protection**: 750A
- **Over-temperature Protection**: 55°C
- **Under-temperature Protection**: -15°C

### Fault Detection
The simulator monitors for fault conditions and sets appropriate states:

#### Critical Faults (Immediate Shutdown)
- Over/Under Voltage (system or cell level)
- Over Current (>750A)
- Over/Under Temperature
- Internal Faults

#### Warnings (Continue with Monitoring)
- High SOC Warning (>95%)
- Low SOC Warning (<10%)
- High Temperature Warning (>45°C)
- Cell Imbalance Warning (>50mV spread)
- Reduced Performance
- Maintenance Due

## System States and Control

### Operating States
```rust
pub enum SystemState {
    Offline = 0,     // System not available
    Standby = 1,     // Ready but not active
    Charging = 2,    // Accepting power (negative)
    Discharging = 3, // Delivering power (positive)
    Fault = 4,       // Fault condition active
    Maintenance = 5, // In maintenance mode
}
```

### System Commands
```rust
pub enum SystemCommand {
    NoCommand = 0,      // No action
    Start = 1,          // Activate system (Offline → Standby)
    Stop = 2,           // Deactivate system (Any → Offline)
    EmergencyStop = 3,  // Immediate stop with fault
    ResetFaults = 4,    // Clear active faults
    MaintenanceMode = 5,// Enter maintenance state
}
```

### State Transitions
- **Start**: `Offline` → `Standby` → `Charging/Discharging` (based on power setpoint)
- **Stop**: Any state → `Offline`
- **Fault Detection**: Any state → `Fault`
- **Fault Reset**: `Fault` → `Standby` (if no active faults)

## Modbus Communication Specification

### Connection Parameters
- **Protocol**: Modbus TCP
- **Default Port**: 5020 (fallback: 5021, 5022, 5023)
- **Unit ID**: 1
- **Byte Order**: Big Endian (Motorola)
- **Word Order**: Big Endian
- **Update Rate**: 10 Hz (100ms)

### Function Code Support
- **03 (Read Holding Registers)**: Read all data points
- **04 (Read Input Registers)**: Read-only measurements
- **06 (Write Single Register)**: Write individual setpoints
- **16 (Write Multiple Registers)**: Write multiple setpoints

## Data Structures

### Core Data Types
```rust
/// Electrical parameters
pub struct ElectricalParams {
    pub voltage: f64,           // Battery DC voltage (V)
    pub current: f64,           // Current (A): + = discharge, - = charge
    pub power: f64,             // Instantaneous power (kW)
    pub soc: f64,              // State of charge (%)
    pub available_energy: f64,  // Energy available for discharge (kWh)
    pub remaining_capacity: f64, // Energy capacity remaining to full (kWh)
}

/// Cell monitoring data
pub struct CellMonitoring {
    pub voltage_min: f64,    // Minimum cell voltage (V)
    pub voltage_max: f64,    // Maximum cell voltage (V)
    pub voltage_avg: f64,    // Average cell voltage (V)
    pub temp_min: f64,       // Minimum cell temperature (°C)
    pub temp_max: f64,       // Maximum cell temperature (°C)
    pub temp_avg: f64,       // Average cell temperature (°C)
}

/// System status
pub struct SystemStatus {
    pub state: SystemState,         // Current operating state
    pub online: bool,              // System availability (derived from state)
    pub faults: Vec<FaultCode>,    // Active system faults
    pub warnings: Vec<WarningCode>, // Active system warnings
    pub modbus_clients: u32,       // Number of connected clients
    pub uptime: Duration,          // System uptime
}

/// System information
pub struct SystemInfo {
    pub soh: f64,                  // State of health (%)
    pub cycle_count: u32,          // Total equivalent full cycles
    pub internal_resistance: f64,   // Battery internal resistance (Ω)
    pub operating_hours: u32,      // Total runtime in hours
    pub contactor_status: u16,     // Contactor states bitmap
    pub cooling_status: CoolingStatus, // Cooling system status
}

/// Control setpoints
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

/// Battery configuration
pub struct BatteryConfig {
    pub rated_capacity: f64,    // System capacity (kWh)
    pub rated_power: f64,       // System power rating (kW)
    pub cell_count: u16,        // Number of cells
    pub simulation_speed: f64,  // Time multiplier
    pub nominal_voltage: f64,   // Nominal system voltage (V)
    pub max_voltage: f64,       // Maximum voltage (V)
    pub min_voltage: f64,       // Minimum voltage (V)
}
```

### Default Configuration Values
```rust
// Electrical Parameters (Default)
voltage: 400.0 V
current: 0.0 A
power: 0.0 kW
soc: 85.0%
available_energy: 425.0 kWh  // 85% of 500 kWh
remaining_capacity: 75.0 kWh // 15% of 500 kWh

// Cell Monitoring (Default)
voltage_min: 3.20 V
voltage_max: 3.24 V
voltage_avg: 3.22 V
temp_min: 25.0°C
temp_max: 30.0°C
temp_avg: 27.5°C

// System Info (Default)
soh: 98.5%
cycle_count: 0
internal_resistance: 64.0 mΩ  // 0.5 mΩ per cell * 128 cells
operating_hours: 0

// Control Setpoints (Default)
command: NoCommand
power_setpoint: 0.0 kW
current_limit_charge: 625.0 A
current_limit_discharge: 625.0 A
voltage_limit_high: 450.0 V
voltage_limit_low: 350.0 V
soc_limit_high: 95.0%
soc_limit_low: 10.0%

// Battery Config (Default)
rated_capacity: 500.0 kWh
rated_power: 250.0 kW
cell_count: 128
simulation_speed: 1.0x
nominal_voltage: 400.0 V
max_voltage: 450.0 V
min_voltage: 350.0 V
```

## Physics Simulation

### SOC Calculation
The simulator uses energy integration for accurate SOC tracking:

```rust
// Energy change in this time step (Wh)
let energy_delta = power * 1000.0 * dt_hours; // kW to W to Wh

// Update accumulated energy (+ = discharging, - = charging)
accumulated_energy += energy_delta;

// Calculate SOC from accumulated energy
let capacity_wh = rated_capacity * 1000.0; // kWh to Wh
soc = ((capacity_wh / 2.0 - accumulated_energy) / capacity_wh * 100.0)
      .clamp(0.0, 100.0);
```

### Power Control
The simulator implements realistic power control with I²R losses:

```rust
// P = V*I - I²*R (accounting for internal losses)
let voltage_oc = calculate_open_circuit_voltage(soc);

// Solve quadratic equation: P = V*I - I²*R
// Rearranged: R*I² - V*I + P = 0
let current = solve_quadratic_for_current(target_power, voltage_oc, resistance);

// Apply current limits
current = current.clamp(-current_limit_charge, current_limit_discharge);

// Calculate actual voltage and power under load
actual_voltage = voltage_oc - (current * internal_resistance);
actual_power = actual_voltage * current / 1000.0; // Convert to kW
```

### Open Circuit Voltage Model
LiFePO4 voltage curve implementation:

```rust
fn calculate_open_circuit_voltage(soc: f64) -> f64 {
    let soc_normalized = (soc / 100.0).clamp(0.0, 1.0);
    
    let cell_voltage = if soc_normalized > 0.95 {
        3.6 - (soc_normalized - 0.95) * 4.0 // Voltage rise at high SOC
    } else if soc_normalized > 0.05 {
        3.2 + (soc_normalized - 0.5) * 0.4 // Flat region (typical LiFePO4)
    } else {
        3.0 - (0.05 - soc_normalized) * 6.0 // Voltage drop at low SOC
    };
    
    cell_voltage * cell_count
}
```

## Runtime Configuration

### Initialization Commands
The simulator supports runtime configuration through init commands (system must be stopped):

- **`init soc <0-100>`** - Set initial SOC percentage
- **`init capacity <100-1000>`** - Set battery capacity in kWh
- **`init voltage <350-450>`** - Set nominal voltage in V
- **`init temp <-10-50>`** - Set initial temperature in °C
- **`init soh <50-100>`** - Set State of Health percentage
- **`init resistance <0.1-10>`** - Set internal resistance in mΩ
- **`init cycles <0-10000>`** - Set cycle count
- **`init reset`** - Reset all parameters to defaults

### Validation Ranges
All initialization commands include input validation:
- SOC: 0.0 - 100.0%
- Capacity: 100.0 - 1000.0 kWh
- Voltage: 350.0 - 450.0 V
- Temperature: -10.0 - 50.0°C
- SOH: 50.0 - 100.0%
- Resistance: 0.1 - 10.0 mΩ
- Cycles: 0 - 10000

## Terminal User Interface

The battery simulator includes a comprehensive real-time terminal-based monitoring interface with:

- **Dynamic Battery Gauge**: Visual SOC representation with top-to-bottom drain
- **Real-time Parameters**: Electrical, cell, and system monitoring
- **Command Interface**: Interactive control and configuration
- **Color-coded Status**: Intuitive status indication
- **Message System**: Command feedback and system notifications

**See**: `terminal_ui_specification.md` for complete UI design, widget specifications, and implementation details.

## Implementation Status

### ✅ Completed Features
- Complete physics simulation with I²R losses
- Dynamic internal resistance modeling
- Realistic aging simulation (calendar + cycle)
- LiFePO4 voltage curve implementation
- Fault detection and protection systems
- Runtime configuration system
- Terminal user interface
- Modbus TCP server (basic implementation)

### 🔄 Current Limitations
- **Modbus Register Map**: Simplified implementation (not full specification)
- **RTU Support**: Not implemented (TCP only)
- **Multi-client**: Basic support without full industrial features
- **Advanced Thermal**: Simplified thermal modeling

### 📋 Architecture Notes
- **Update Rate**: 10 Hz for all systems (UI, simulation, Modbus)
- **State Management**: Thread-safe with `Arc<RwLock>` and `tokio::sync::watch`
- **Error Handling**: Comprehensive with `anyhow::Result`
- **Logging**: Structured logging with hourly rotation
- **Configuration**: Runtime-configurable parameters

---

### Future Extensions
- Complete Modbus register map implementation
- Modbus RTU support
- Advanced thermal modeling with multiple zones
- Multi-string/parallel configurations
- Grid services simulation (frequency response, etc.)
- Historical data storage and trending
- Web-based monitoring interface

---
*Document Version: 2.0*  
*Last Updated: 2025-09-09*  
*Status: Implemented and Tested*  
*Related Documents: terminal_ui_specification.md*