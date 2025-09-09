use std::collections::HashMap;
use anyhow::Result;

use crate::battery::types::*;

/// Modbus register addresses as constants
pub mod addresses {
    // Input Registers (Read-Only) - Function Code 04
    
    // Real-time Measurements (30001-30100)  
    pub const SOC: u16 = 30001;
    pub const SOH: u16 = 30002;
    pub const BATTERY_VOLTAGE: u16 = 30003;
    pub const BATTERY_CURRENT: u16 = 30004;
    pub const BATTERY_POWER: u16 = 30005;         // INT32 - uses 30005-30006
    pub const CELL_VOLTAGE_MIN: u16 = 30007;
    pub const CELL_VOLTAGE_MAX: u16 = 30008;
    pub const CELL_VOLTAGE_AVG: u16 = 30009;
    pub const CELL_TEMP_MIN: u16 = 30010;
    pub const CELL_TEMP_MAX: u16 = 30011;
    pub const CELL_TEMP_AVG: u16 = 30012;
    pub const INTERNAL_RESISTANCE: u16 = 30013;
    pub const AVAILABLE_CHARGE_ENERGY: u16 = 30014;  // UINT32 - uses 30014-30015
    pub const AVAILABLE_DISCHARGE_ENERGY: u16 = 30016; // UINT32 - uses 30016-30017
    pub const CYCLE_COUNT: u16 = 30018;           // UINT32 - uses 30018-30019
    pub const OPERATING_HOURS: u16 = 30020;       // UINT32 - uses 30020-30021
    
    // System Status (30101-30150)
    pub const SYSTEM_STATE: u16 = 30101;
    pub const FAULT_STATUS: u16 = 30102;          // UINT32 - uses 30102-30103
    pub const WARNING_STATUS: u16 = 30104;        // UINT32 - uses 30104-30105
    pub const CONTACTOR_STATUS: u16 = 30106;
    pub const COOLING_STATUS: u16 = 30107;
    
    // Holding Registers (Read/Write) - Function Code 03/06/16
    
    // Control Setpoints (40001-40100)
    pub const COMMAND: u16 = 40001;
    pub const POWER_SETPOINT: u16 = 40002;        // INT32 - uses 40002-40003
    pub const CURRENT_LIMIT_CHARGE: u16 = 40004;
    pub const CURRENT_LIMIT_DISCHARGE: u16 = 40005;
    pub const VOLTAGE_LIMIT_HIGH: u16 = 40006;
    pub const VOLTAGE_LIMIT_LOW: u16 = 40007;
    pub const SOC_LIMIT_HIGH: u16 = 40008;
    pub const SOC_LIMIT_LOW: u16 = 40009;
    
    // Configuration Parameters (40101-40200)
    pub const RATED_CAPACITY: u16 = 40101;        // UINT32 - uses 40101-40102
    pub const RATED_POWER: u16 = 40103;
    pub const CELL_COUNT: u16 = 40104;
    pub const SIMULATION_SPEED: u16 = 40105;
}

/// Modbus data type conversions and scaling
pub struct ModbusConverter;

impl ModbusConverter {
    /// Convert f64 to scaled UINT16 (e.g., voltage in V to tenths)
    pub fn f64_to_uint16_scaled(value: f64, scale: f64) -> u16 {
        let scaled = (value * scale).round();
        scaled.clamp(0.0, 65535.0) as u16
    }
    
    /// Convert f64 to scaled INT16 (e.g., current in A to tenths)
    pub fn f64_to_int16_scaled(value: f64, scale: f64) -> i16 {
        let scaled = (value * scale).round();
        scaled.clamp(-32768.0, 32767.0) as i16
    }
    
    /// Convert f64 to scaled UINT32 (e.g., energy in kWh to tenths)
    pub fn f64_to_uint32_scaled(value: f64, scale: f64) -> u32 {
        let scaled = (value * scale).round();
        scaled.clamp(0.0, 4294967295.0) as u32
    }
    
    /// Convert f64 to scaled INT32 (e.g., power in kW to tenths)
    pub fn f64_to_int32_scaled(value: f64, scale: f64) -> i32 {
        let scaled = (value * scale).round();
        scaled.clamp(-2147483648.0, 2147483647.0) as i32
    }
    
    /// Convert UINT16 to f64 with scaling (e.g., tenths to V)
    pub fn uint16_to_f64_scaled(value: u16, scale: f64) -> f64 {
        value as f64 / scale
    }
    
    /// Convert INT16 to f64 with scaling (e.g., tenths to A)
    pub fn int16_to_f64_scaled(value: i16, scale: f64) -> f64 {
        value as f64 / scale
    }
    
    /// Convert UINT32 to f64 with scaling (e.g., tenths to kWh)
    pub fn uint32_to_f64_scaled(value: u32, scale: f64) -> f64 {
        value as f64 / scale
    }
    
    /// Convert INT32 to f64 with scaling (e.g., tenths to kW)
    pub fn int32_to_f64_scaled(value: i32, scale: f64) -> f64 {
        value as f64 / scale
    }
    
    /// Split UINT32 into two UINT16 values (high word first - big endian)
    pub fn uint32_to_words(value: u32) -> (u16, u16) {
        let high = ((value >> 16) & 0xFFFF) as u16;
        let low = (value & 0xFFFF) as u16;
        (high, low)
    }
    
    /// Combine two UINT16 values into UINT32 (high word first - big endian)
    pub fn words_to_uint32(high: u16, low: u16) -> u32 {
        ((high as u32) << 16) | (low as u32)
    }
    
    /// Split INT32 into two UINT16 values (high word first - big endian)
    pub fn int32_to_words(value: i32) -> (u16, u16) {
        let unsigned = value as u32;
        Self::uint32_to_words(unsigned)
    }
    
    /// Combine two UINT16 values into INT32 (high word first - big endian)
    pub fn words_to_int32(high: u16, low: u16) -> i32 {
        Self::words_to_uint32(high, low) as i32
    }
}

/// Modbus register map for battery state
pub struct ModbusRegisterMap {
    input_registers: HashMap<u16, u16>,    // Address -> Value (read-only)
    holding_registers: HashMap<u16, u16>,  // Address -> Value (read/write)
}

impl ModbusRegisterMap {
    /// Create a new register map
    pub fn new() -> Self {
        Self {
            input_registers: HashMap::new(),
            holding_registers: HashMap::new(),
        }
    }
    
    /// Update input registers from battery state (read-only measurements)
    pub fn update_input_registers(&mut self, state: &BatteryState) -> Result<()> {
        use addresses::*;
        
        // Real-time measurements
        self.input_registers.insert(SOC, ModbusConverter::f64_to_uint16_scaled(state.electrical.soc, 10.0));
        self.input_registers.insert(SOH, ModbusConverter::f64_to_uint16_scaled(state.info.soh, 10.0));
        self.input_registers.insert(BATTERY_VOLTAGE, ModbusConverter::f64_to_uint16_scaled(state.electrical.voltage, 10.0));
        self.input_registers.insert(BATTERY_CURRENT, ModbusConverter::f64_to_int16_scaled(state.electrical.current, 10.0) as u16);
        
        // Battery power (INT32)
        let power_scaled = ModbusConverter::f64_to_int32_scaled(state.electrical.power, 10.0);
        let (power_high, power_low) = ModbusConverter::int32_to_words(power_scaled);
        self.input_registers.insert(BATTERY_POWER, power_high);
        self.input_registers.insert(BATTERY_POWER + 1, power_low);
        
        // Cell voltages (converted to mV)
        self.input_registers.insert(CELL_VOLTAGE_MIN, ModbusConverter::f64_to_uint16_scaled(state.cells.voltage_min * 1000.0, 1.0));
        self.input_registers.insert(CELL_VOLTAGE_MAX, ModbusConverter::f64_to_uint16_scaled(state.cells.voltage_max * 1000.0, 1.0));
        self.input_registers.insert(CELL_VOLTAGE_AVG, ModbusConverter::f64_to_uint16_scaled(state.cells.voltage_avg * 1000.0, 1.0));
        
        // Cell temperatures
        self.input_registers.insert(CELL_TEMP_MIN, ModbusConverter::f64_to_int16_scaled(state.cells.temp_min, 10.0) as u16);
        self.input_registers.insert(CELL_TEMP_MAX, ModbusConverter::f64_to_int16_scaled(state.cells.temp_max, 10.0) as u16);
        self.input_registers.insert(CELL_TEMP_AVG, ModbusConverter::f64_to_int16_scaled(state.cells.temp_avg, 10.0) as u16);
        
        // Internal resistance
        self.input_registers.insert(INTERNAL_RESISTANCE, ModbusConverter::f64_to_uint16_scaled(state.info.internal_resistance, 10.0));
        
        // Energy availability (UINT32)
        let charge_energy_scaled = ModbusConverter::f64_to_uint32_scaled(state.electrical.remaining_capacity, 10.0);
        let (charge_high, charge_low) = ModbusConverter::uint32_to_words(charge_energy_scaled);
        self.input_registers.insert(AVAILABLE_CHARGE_ENERGY, charge_high);
        self.input_registers.insert(AVAILABLE_CHARGE_ENERGY + 1, charge_low);
        
        let discharge_energy_scaled = ModbusConverter::f64_to_uint32_scaled(state.electrical.available_energy, 10.0);
        let (discharge_high, discharge_low) = ModbusConverter::uint32_to_words(discharge_energy_scaled);
        self.input_registers.insert(AVAILABLE_DISCHARGE_ENERGY, discharge_high);
        self.input_registers.insert(AVAILABLE_DISCHARGE_ENERGY + 1, discharge_low);
        
        // Cycle count and operating hours (UINT32)
        let (cycle_high, cycle_low) = ModbusConverter::uint32_to_words(state.info.cycle_count);
        self.input_registers.insert(CYCLE_COUNT, cycle_high);
        self.input_registers.insert(CYCLE_COUNT + 1, cycle_low);
        
        let (hours_high, hours_low) = ModbusConverter::uint32_to_words(state.info.operating_hours);
        self.input_registers.insert(OPERATING_HOURS, hours_high);
        self.input_registers.insert(OPERATING_HOURS + 1, hours_low);
        
        // System status
        self.input_registers.insert(SYSTEM_STATE, state.status.state as u16);
        
        // Fault and warning bitmaps (UINT32)
        let fault_bitmap = FaultCode::to_bitmap(&state.status.faults);
        let (fault_high, fault_low) = ModbusConverter::uint32_to_words(fault_bitmap);
        self.input_registers.insert(FAULT_STATUS, fault_high);
        self.input_registers.insert(FAULT_STATUS + 1, fault_low);
        
        let warning_bitmap = WarningCode::to_bitmap(&state.status.warnings);
        let (warning_high, warning_low) = ModbusConverter::uint32_to_words(warning_bitmap);
        self.input_registers.insert(WARNING_STATUS, warning_high);
        self.input_registers.insert(WARNING_STATUS + 1, warning_low);
        
        self.input_registers.insert(CONTACTOR_STATUS, state.info.contactor_status);
        self.input_registers.insert(COOLING_STATUS, state.info.cooling_status as u16);
        
        Ok(())
    }
    
    /// Update holding registers from setpoints and config (read/write)
    pub fn update_holding_registers(&mut self, state: &BatteryState) -> Result<()> {
        use addresses::*;
        
        // Control setpoints
        self.holding_registers.insert(COMMAND, state.setpoints.command as u16);
        
        let power_setpoint_scaled = ModbusConverter::f64_to_int32_scaled(state.setpoints.power_setpoint, 10.0);
        let (power_high, power_low) = ModbusConverter::int32_to_words(power_setpoint_scaled);
        self.holding_registers.insert(POWER_SETPOINT, power_high);
        self.holding_registers.insert(POWER_SETPOINT + 1, power_low);
        
        self.holding_registers.insert(CURRENT_LIMIT_CHARGE, ModbusConverter::f64_to_uint16_scaled(state.setpoints.current_limit_charge, 10.0));
        self.holding_registers.insert(CURRENT_LIMIT_DISCHARGE, ModbusConverter::f64_to_uint16_scaled(state.setpoints.current_limit_discharge, 10.0));
        self.holding_registers.insert(VOLTAGE_LIMIT_HIGH, ModbusConverter::f64_to_uint16_scaled(state.setpoints.voltage_limit_high, 10.0));
        self.holding_registers.insert(VOLTAGE_LIMIT_LOW, ModbusConverter::f64_to_uint16_scaled(state.setpoints.voltage_limit_low, 10.0));
        self.holding_registers.insert(SOC_LIMIT_HIGH, ModbusConverter::f64_to_uint16_scaled(state.setpoints.soc_limit_high, 10.0));
        self.holding_registers.insert(SOC_LIMIT_LOW, ModbusConverter::f64_to_uint16_scaled(state.setpoints.soc_limit_low, 10.0));
        
        // Configuration parameters
        let capacity_scaled = ModbusConverter::f64_to_uint32_scaled(state.config.rated_capacity, 10.0);
        let (capacity_high, capacity_low) = ModbusConverter::uint32_to_words(capacity_scaled);
        self.holding_registers.insert(RATED_CAPACITY, capacity_high);
        self.holding_registers.insert(RATED_CAPACITY + 1, capacity_low);
        
        self.holding_registers.insert(RATED_POWER, ModbusConverter::f64_to_uint16_scaled(state.config.rated_power, 1.0));
        self.holding_registers.insert(CELL_COUNT, state.config.cell_count);
        self.holding_registers.insert(SIMULATION_SPEED, ModbusConverter::f64_to_uint16_scaled(state.config.simulation_speed, 10.0));
        
        Ok(())
    }
    
    /// Read input register value
    pub fn read_input_register(&self, address: u16) -> Option<u16> {
        self.input_registers.get(&address).copied()
    }
    
    /// Read multiple input registers
    pub fn read_input_registers(&self, start_address: u16, count: u16) -> Vec<u16> {
        (start_address..start_address + count)
            .map(|addr| self.input_registers.get(&addr).copied().unwrap_or(0))
            .collect()
    }
    
    /// Read holding register value
    pub fn read_holding_register(&self, address: u16) -> Option<u16> {
        self.holding_registers.get(&address).copied()
    }
    
    /// Read multiple holding registers
    pub fn read_holding_registers(&self, start_address: u16, count: u16) -> Vec<u16> {
        (start_address..start_address + count)
            .map(|addr| self.holding_registers.get(&addr).copied().unwrap_or(0))
            .collect()
    }
    
    /// Write holding register value
    pub fn write_holding_register(&mut self, address: u16, value: u16) -> Result<()> {
        self.holding_registers.insert(address, value);
        Ok(())
    }
    
    /// Write multiple holding registers
    pub fn write_holding_registers(&mut self, start_address: u16, values: &[u16]) -> Result<()> {
        for (i, &value) in values.iter().enumerate() {
            self.holding_registers.insert(start_address + i as u16, value);
        }
        Ok(())
    }
    
    /// Convert holding register changes to setpoints/config updates
    pub fn extract_setpoints_from_registers(&self) -> Result<ControlSetpoints> {
        use addresses::*;
        
        let mut setpoints = ControlSetpoints::default();
        
        if let Some(command) = self.holding_registers.get(&COMMAND) {
            setpoints.command = match *command {
                0 => SystemCommand::NoCommand,
                1 => SystemCommand::Start,
                2 => SystemCommand::Stop,
                3 => SystemCommand::EmergencyStop,
                4 => SystemCommand::ResetFaults,
                5 => SystemCommand::MaintenanceMode,
                _ => SystemCommand::NoCommand,
            };
        }
        
        // Power setpoint (INT32)
        if let (Some(&high), Some(&low)) = (
            self.holding_registers.get(&POWER_SETPOINT),
            self.holding_registers.get(&(POWER_SETPOINT + 1))
        ) {
            let power_raw = ModbusConverter::words_to_int32(high, low);
            setpoints.power_setpoint = ModbusConverter::int32_to_f64_scaled(power_raw, 10.0);
        }
        
        // Current limits
        if let Some(charge_limit) = self.holding_registers.get(&CURRENT_LIMIT_CHARGE) {
            setpoints.current_limit_charge = ModbusConverter::uint16_to_f64_scaled(*charge_limit, 10.0);
        }
        if let Some(discharge_limit) = self.holding_registers.get(&CURRENT_LIMIT_DISCHARGE) {
            setpoints.current_limit_discharge = ModbusConverter::uint16_to_f64_scaled(*discharge_limit, 10.0);
        }
        
        // Voltage limits
        if let Some(high_limit) = self.holding_registers.get(&VOLTAGE_LIMIT_HIGH) {
            setpoints.voltage_limit_high = ModbusConverter::uint16_to_f64_scaled(*high_limit, 10.0);
        }
        if let Some(low_limit) = self.holding_registers.get(&VOLTAGE_LIMIT_LOW) {
            setpoints.voltage_limit_low = ModbusConverter::uint16_to_f64_scaled(*low_limit, 10.0);
        }
        
        // SOC limits
        if let Some(high_soc) = self.holding_registers.get(&SOC_LIMIT_HIGH) {
            setpoints.soc_limit_high = ModbusConverter::uint16_to_f64_scaled(*high_soc, 10.0);
        }
        if let Some(low_soc) = self.holding_registers.get(&SOC_LIMIT_LOW) {
            setpoints.soc_limit_low = ModbusConverter::uint16_to_f64_scaled(*low_soc, 10.0);
        }
        
        Ok(setpoints)
    }
    
    /// Convert holding register changes to config updates
    pub fn extract_config_from_registers(&self) -> Result<BatteryConfig> {
        use addresses::*;
        
        let mut config = BatteryConfig::default();
        
        // Rated capacity (UINT32)
        if let (Some(&high), Some(&low)) = (
            self.holding_registers.get(&RATED_CAPACITY),
            self.holding_registers.get(&(RATED_CAPACITY + 1))
        ) {
            let capacity_raw = ModbusConverter::words_to_uint32(high, low);
            config.rated_capacity = ModbusConverter::uint32_to_f64_scaled(capacity_raw, 10.0);
        }
        
        if let Some(power) = self.holding_registers.get(&RATED_POWER) {
            config.rated_power = ModbusConverter::uint16_to_f64_scaled(*power, 1.0);
        }
        if let Some(cells) = self.holding_registers.get(&CELL_COUNT) {
            config.cell_count = *cells;
        }
        if let Some(speed) = self.holding_registers.get(&SIMULATION_SPEED) {
            config.simulation_speed = ModbusConverter::uint16_to_f64_scaled(*speed, 10.0);
        }
        
        Ok(config)
    }
}

impl Default for ModbusRegisterMap {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_data_type_conversions() {
        // Test UINT16 scaling
        assert_eq!(ModbusConverter::f64_to_uint16_scaled(45.7, 10.0), 457);
        assert_eq!(ModbusConverter::uint16_to_f64_scaled(457, 10.0), 45.7);
        
        // Test INT16 scaling
        assert_eq!(ModbusConverter::f64_to_int16_scaled(-123.4, 10.0), -1234);
        assert_eq!(ModbusConverter::int16_to_f64_scaled(-1234, 10.0), -123.4);
        
        // Test 32-bit word splitting
        let (high, low) = ModbusConverter::uint32_to_words(0x12345678);
        assert_eq!(high, 0x1234);
        assert_eq!(low, 0x5678);
        assert_eq!(ModbusConverter::words_to_uint32(high, low), 0x12345678);
    }
    
    #[test]
    fn test_register_mapping() {
        let mut map = ModbusRegisterMap::new();
        let state = BatteryState::default();
        
        map.update_input_registers(&state).unwrap();
        map.update_holding_registers(&state).unwrap();
        
        // Check SOC register (50% * 10 = 500)
        assert_eq!(map.read_input_register(addresses::SOC), Some(500));
        
        // Check voltage register (400V * 10 = 4000)
        assert_eq!(map.read_input_register(addresses::BATTERY_VOLTAGE), Some(4000));
        
        // Check power setpoint extraction
        map.write_holding_register(addresses::POWER_SETPOINT, 0x03E8).unwrap();  // High word = 1000
        map.write_holding_register(addresses::POWER_SETPOINT + 1, 0x0000).unwrap();  // Low word = 0
        let setpoints = map.extract_setpoints_from_registers().unwrap();
        assert_eq!(setpoints.power_setpoint, 6553600.0);  // 0x03E80000 / 10 = 65536000 / 10 = 6553600
        
        // Test a simpler case
        map.write_holding_register(addresses::POWER_SETPOINT, 0x0000).unwrap();  // High word = 0  
        map.write_holding_register(addresses::POWER_SETPOINT + 1, 0x07D0).unwrap();  // Low word = 2000
        let setpoints = map.extract_setpoints_from_registers().unwrap();
        assert_eq!(setpoints.power_setpoint, 200.0);  // 2000 / 10 = 200
    }
}