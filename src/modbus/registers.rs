use std::collections::HashMap;
use anyhow::Result;

use crate::battery::types::*;

/// Modbus register addresses as constants
///
/// This is battsim2's own register map (not a copy of any specific vendor's map) but
/// aims to cover the breadth of tags a typical grid-scale BESS exposes over Modbus:
/// system/site status, PCS (AC) measurements, BMS (DC) measurements, cell detail,
/// a POI/site meter, and auxiliary/environmental data. See docs/MODBUS_MAP.md for the
/// full address/type/scale/unit reference table.
pub mod addresses {
    // ============================================================
    // Input Registers (Read-Only) - Function Code 04
    // ============================================================

    // --- System / Site Status (30001-30099) ---
    pub const SYSTEM_STATE: u16 = 30001;
    pub const OPERATING_MODE: u16 = 30002;
    pub const GRID_CONNECTED: u16 = 30003;
    pub const CONTACTOR_STATUS: u16 = 30004;
    pub const FAULT_STATUS: u16 = 30005;           // UINT32 - uses 30005-30006
    pub const WARNING_STATUS: u16 = 30007;         // UINT32 - uses 30007-30008
    pub const ACTIVE_FAULT_COUNT: u16 = 30009;
    pub const ACTIVE_WARNING_COUNT: u16 = 30010;
    pub const MODBUS_CLIENT_COUNT: u16 = 30011;
    pub const UPTIME_SECONDS: u16 = 30012;         // UINT32 - uses 30012-30013
    pub const OPERATING_HOURS: u16 = 30014;        // UINT32 - uses 30014-30015
    pub const FIRE_SUPPRESSION_STATUS: u16 = 30016;
    pub const SMOKE_DETECTED: u16 = 30017;

    // --- PCS / AC Measurements (30101-30199) ---
    pub const PCS_STATE: u16 = 30101;
    pub const GRID_FORMING: u16 = 30102;
    pub const AC_FREQUENCY: u16 = 30103;
    pub const AC_POWER_FACTOR: u16 = 30104;
    pub const AC_REAL_POWER: u16 = 30105;          // INT32 - uses 30105-30106
    pub const AC_REACTIVE_POWER: u16 = 30107;      // INT32 - uses 30107-30108
    pub const AC_APPARENT_POWER: u16 = 30109;      // UINT32 - uses 30109-30110
    pub const AC_VOLTAGE_L1N: u16 = 30111;
    pub const AC_VOLTAGE_L2N: u16 = 30112;
    pub const AC_VOLTAGE_L3N: u16 = 30113;
    pub const AC_CURRENT_L1: u16 = 30114;
    pub const AC_CURRENT_L2: u16 = 30115;
    pub const AC_CURRENT_L3: u16 = 30116;
    pub const PCS_TEMPERATURE: u16 = 30117;
    pub const ISOLATION_RESISTANCE: u16 = 30118;

    // --- BMS / DC Battery Measurements (30201-30299) ---
    pub const SOC: u16 = 30201;
    pub const SOH: u16 = 30202;
    pub const BATTERY_VOLTAGE: u16 = 30203;
    pub const BATTERY_CURRENT: u16 = 30204;
    pub const BATTERY_POWER: u16 = 30205;          // INT32 - uses 30205-30206
    pub const MAX_CHARGE_POWER: u16 = 30207;       // UINT32 - uses 30207-30208
    pub const MAX_DISCHARGE_POWER: u16 = 30209;    // UINT32 - uses 30209-30210
    pub const AVAILABLE_CHARGE_ENERGY: u16 = 30211;    // UINT32 - uses 30211-30212
    pub const AVAILABLE_DISCHARGE_ENERGY: u16 = 30213; // UINT32 - uses 30213-30214
    pub const LIFETIME_CHARGE_ENERGY: u16 = 30215;     // UINT32 - uses 30215-30216
    pub const LIFETIME_DISCHARGE_ENERGY: u16 = 30217;  // UINT32 - uses 30217-30218
    pub const CYCLE_COUNT: u16 = 30219;            // UINT32 - uses 30219-30220
    pub const INTERNAL_RESISTANCE: u16 = 30221;

    // --- Cell Detail (30301-30399) ---
    pub const CELL_VOLTAGE_MIN: u16 = 30301;
    pub const CELL_VOLTAGE_MAX: u16 = 30302;
    pub const CELL_VOLTAGE_AVG: u16 = 30303;
    pub const CELL_VOLTAGE_DELTA: u16 = 30304;
    pub const CELL_TEMP_MIN: u16 = 30305;
    pub const CELL_TEMP_MAX: u16 = 30306;
    pub const CELL_TEMP_AVG: u16 = 30307;
    pub const BALANCING_CELLS: u16 = 30308;

    // --- Meter / POI (30401-30499) ---
    pub const METER_VOLTAGE_L1N: u16 = 30401;
    pub const METER_VOLTAGE_L2N: u16 = 30402;
    pub const METER_VOLTAGE_L3N: u16 = 30403;
    pub const METER_CURRENT_L1: u16 = 30404;
    pub const METER_CURRENT_L2: u16 = 30405;
    pub const METER_CURRENT_L3: u16 = 30406;
    pub const METER_FREQUENCY: u16 = 30407;
    pub const METER_REAL_POWER: u16 = 30408;       // INT32 - uses 30408-30409
    pub const METER_REACTIVE_POWER: u16 = 30410;   // INT32 - uses 30410-30411
    pub const LIFETIME_IMPORT_ENERGY: u16 = 30412; // UINT32 - uses 30412-30413
    pub const LIFETIME_EXPORT_ENERGY: u16 = 30414; // UINT32 - uses 30414-30415
    pub const AUX_LOAD_POWER: u16 = 30416;

    // --- Auxiliary / Environment (30501-30599) ---
    pub const COOLING_STATUS: u16 = 30501;
    pub const AMBIENT_TEMPERATURE: u16 = 30502;
    pub const ENCLOSURE_TEMPERATURE: u16 = 30503;
    pub const AUX_POWER_CONSUMPTION: u16 = 30504;

    // ============================================================
    // Holding Registers (Read/Write) - Function Code 03/06/16
    // ============================================================

    // --- Control Setpoints (40001-40099) ---
    pub const COMMAND: u16 = 40001;
    pub const ACTIVE_POWER_SETPOINT: u16 = 40002;      // INT32 - uses 40002-40003
    pub const REACTIVE_POWER_SETPOINT: u16 = 40004;    // INT32 - uses 40004-40005
    pub const POWER_FACTOR_SETPOINT: u16 = 40006;
    pub const OPERATING_MODE_SETPOINT: u16 = 40007;
    pub const GRID_CONNECT_COMMAND: u16 = 40008;
    pub const CURRENT_LIMIT_CHARGE: u16 = 40009;
    pub const CURRENT_LIMIT_DISCHARGE: u16 = 40010;
    pub const VOLTAGE_LIMIT_HIGH: u16 = 40011;
    pub const VOLTAGE_LIMIT_LOW: u16 = 40012;
    pub const SOC_LIMIT_HIGH: u16 = 40013;
    pub const SOC_LIMIT_LOW: u16 = 40014;
    pub const WATCHDOG_TIMEOUT: u16 = 40015;

    // --- Configuration Parameters (40101-40199) ---
    pub const RATED_CAPACITY: u16 = 40101;         // UINT32 - uses 40101-40102
    pub const RATED_POWER: u16 = 40103;
    pub const RATED_REACTIVE_POWER: u16 = 40104;
    pub const CELL_COUNT: u16 = 40105;
    pub const RATED_AC_VOLTAGE: u16 = 40106;
    pub const NOMINAL_FREQUENCY: u16 = 40107;
    pub const SIMULATION_SPEED: u16 = 40108;
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

    fn set_u32(&mut self, address: u16, value: u32) {
        let (high, low) = ModbusConverter::uint32_to_words(value);
        self.input_registers.insert(address, high);
        self.input_registers.insert(address + 1, low);
    }

    fn set_i32(&mut self, address: u16, value: i32) {
        let (high, low) = ModbusConverter::int32_to_words(value);
        self.input_registers.insert(address, high);
        self.input_registers.insert(address + 1, low);
    }

    /// Update input registers from battery state (read-only measurements)
    pub fn update_input_registers(&mut self, state: &BatteryState) -> Result<()> {
        use addresses::*;

        // --- System / Site Status ---
        self.input_registers.insert(SYSTEM_STATE, state.status.state as u16);
        self.input_registers.insert(OPERATING_MODE, state.status.operating_mode as u16);
        self.input_registers.insert(GRID_CONNECTED, state.status.grid_connected as u16);
        self.input_registers.insert(CONTACTOR_STATUS, state.info.contactor_status);

        let fault_bitmap = FaultCode::to_bitmap(&state.status.faults);
        self.set_u32(FAULT_STATUS, fault_bitmap);
        let warning_bitmap = WarningCode::to_bitmap(&state.status.warnings);
        self.set_u32(WARNING_STATUS, warning_bitmap);

        self.input_registers.insert(ACTIVE_FAULT_COUNT, state.status.faults.len() as u16);
        self.input_registers.insert(ACTIVE_WARNING_COUNT, state.status.warnings.len() as u16);
        self.input_registers.insert(MODBUS_CLIENT_COUNT, state.status.modbus_clients as u16);
        self.set_u32(UPTIME_SECONDS, state.status.uptime.as_secs() as u32);
        self.set_u32(OPERATING_HOURS, state.info.operating_hours);
        self.input_registers.insert(FIRE_SUPPRESSION_STATUS, state.info.fire_suppression_status as u16);
        self.input_registers.insert(SMOKE_DETECTED, state.info.smoke_detected as u16);

        // --- PCS / AC Measurements ---
        self.input_registers.insert(PCS_STATE, state.ac.pcs_state as u16);
        self.input_registers.insert(GRID_FORMING, state.ac.grid_forming as u16);
        self.input_registers.insert(AC_FREQUENCY, ModbusConverter::f64_to_uint16_scaled(state.ac.frequency, 100.0));
        self.input_registers.insert(AC_POWER_FACTOR, ModbusConverter::f64_to_int16_scaled(state.ac.power_factor, 1000.0) as u16);
        self.set_i32(AC_REAL_POWER, ModbusConverter::f64_to_int32_scaled(state.ac.real_power, 10.0));
        self.set_i32(AC_REACTIVE_POWER, ModbusConverter::f64_to_int32_scaled(state.ac.reactive_power, 10.0));
        self.set_u32(AC_APPARENT_POWER, ModbusConverter::f64_to_uint32_scaled(state.ac.apparent_power, 10.0));
        self.input_registers.insert(AC_VOLTAGE_L1N, ModbusConverter::f64_to_uint16_scaled(state.ac.voltage_l1n, 10.0));
        self.input_registers.insert(AC_VOLTAGE_L2N, ModbusConverter::f64_to_uint16_scaled(state.ac.voltage_l2n, 10.0));
        self.input_registers.insert(AC_VOLTAGE_L3N, ModbusConverter::f64_to_uint16_scaled(state.ac.voltage_l3n, 10.0));
        self.input_registers.insert(AC_CURRENT_L1, ModbusConverter::f64_to_uint16_scaled(state.ac.current_l1, 10.0));
        self.input_registers.insert(AC_CURRENT_L2, ModbusConverter::f64_to_uint16_scaled(state.ac.current_l2, 10.0));
        self.input_registers.insert(AC_CURRENT_L3, ModbusConverter::f64_to_uint16_scaled(state.ac.current_l3, 10.0));
        self.input_registers.insert(PCS_TEMPERATURE, ModbusConverter::f64_to_int16_scaled(state.ac.pcs_temperature, 10.0) as u16);
        self.input_registers.insert(ISOLATION_RESISTANCE, ModbusConverter::f64_to_uint16_scaled(state.ac.isolation_resistance, 10.0));

        // --- BMS / DC Battery Measurements ---
        self.input_registers.insert(SOC, ModbusConverter::f64_to_uint16_scaled(state.electrical.soc, 10.0));
        self.input_registers.insert(SOH, ModbusConverter::f64_to_uint16_scaled(state.info.soh, 10.0));
        self.input_registers.insert(BATTERY_VOLTAGE, ModbusConverter::f64_to_uint16_scaled(state.electrical.voltage, 10.0));
        self.input_registers.insert(BATTERY_CURRENT, ModbusConverter::f64_to_int16_scaled(state.electrical.current, 10.0) as u16);
        self.set_i32(BATTERY_POWER, ModbusConverter::f64_to_int32_scaled(state.electrical.power, 10.0));
        self.set_u32(MAX_CHARGE_POWER, ModbusConverter::f64_to_uint32_scaled(state.electrical.max_charge_power, 10.0));
        self.set_u32(MAX_DISCHARGE_POWER, ModbusConverter::f64_to_uint32_scaled(state.electrical.max_discharge_power, 10.0));
        self.set_u32(AVAILABLE_CHARGE_ENERGY, ModbusConverter::f64_to_uint32_scaled(state.electrical.remaining_capacity, 10.0));
        self.set_u32(AVAILABLE_DISCHARGE_ENERGY, ModbusConverter::f64_to_uint32_scaled(state.electrical.available_energy, 10.0));
        self.set_u32(LIFETIME_CHARGE_ENERGY, ModbusConverter::f64_to_uint32_scaled(state.electrical.lifetime_charge_energy, 10.0));
        self.set_u32(LIFETIME_DISCHARGE_ENERGY, ModbusConverter::f64_to_uint32_scaled(state.electrical.lifetime_discharge_energy, 10.0));
        self.set_u32(CYCLE_COUNT, state.info.cycle_count);
        self.input_registers.insert(INTERNAL_RESISTANCE, ModbusConverter::f64_to_uint16_scaled(state.info.internal_resistance, 10.0));

        // --- Cell Detail ---
        self.input_registers.insert(CELL_VOLTAGE_MIN, ModbusConverter::f64_to_uint16_scaled(state.cells.voltage_min * 1000.0, 1.0));
        self.input_registers.insert(CELL_VOLTAGE_MAX, ModbusConverter::f64_to_uint16_scaled(state.cells.voltage_max * 1000.0, 1.0));
        self.input_registers.insert(CELL_VOLTAGE_AVG, ModbusConverter::f64_to_uint16_scaled(state.cells.voltage_avg * 1000.0, 1.0));
        self.input_registers.insert(CELL_VOLTAGE_DELTA, ModbusConverter::f64_to_uint16_scaled(state.cells.voltage_delta * 1000.0, 1.0));
        self.input_registers.insert(CELL_TEMP_MIN, ModbusConverter::f64_to_int16_scaled(state.cells.temp_min, 10.0) as u16);
        self.input_registers.insert(CELL_TEMP_MAX, ModbusConverter::f64_to_int16_scaled(state.cells.temp_max, 10.0) as u16);
        self.input_registers.insert(CELL_TEMP_AVG, ModbusConverter::f64_to_int16_scaled(state.cells.temp_avg, 10.0) as u16);
        self.input_registers.insert(BALANCING_CELLS, state.cells.balancing_cells);

        // --- Meter / POI ---
        self.input_registers.insert(METER_VOLTAGE_L1N, ModbusConverter::f64_to_uint16_scaled(state.meter.voltage_l1n, 10.0));
        self.input_registers.insert(METER_VOLTAGE_L2N, ModbusConverter::f64_to_uint16_scaled(state.meter.voltage_l2n, 10.0));
        self.input_registers.insert(METER_VOLTAGE_L3N, ModbusConverter::f64_to_uint16_scaled(state.meter.voltage_l3n, 10.0));
        self.input_registers.insert(METER_CURRENT_L1, ModbusConverter::f64_to_uint16_scaled(state.meter.current_l1, 10.0));
        self.input_registers.insert(METER_CURRENT_L2, ModbusConverter::f64_to_uint16_scaled(state.meter.current_l2, 10.0));
        self.input_registers.insert(METER_CURRENT_L3, ModbusConverter::f64_to_uint16_scaled(state.meter.current_l3, 10.0));
        self.input_registers.insert(METER_FREQUENCY, ModbusConverter::f64_to_uint16_scaled(state.meter.frequency, 100.0));
        self.set_i32(METER_REAL_POWER, ModbusConverter::f64_to_int32_scaled(state.meter.real_power, 10.0));
        self.set_i32(METER_REACTIVE_POWER, ModbusConverter::f64_to_int32_scaled(state.meter.reactive_power, 10.0));
        self.set_u32(LIFETIME_IMPORT_ENERGY, ModbusConverter::f64_to_uint32_scaled(state.meter.lifetime_import_energy, 10.0));
        self.set_u32(LIFETIME_EXPORT_ENERGY, ModbusConverter::f64_to_uint32_scaled(state.meter.lifetime_export_energy, 10.0));
        self.input_registers.insert(AUX_LOAD_POWER, ModbusConverter::f64_to_uint16_scaled(state.meter.aux_load_power, 10.0));

        // --- Auxiliary / Environment ---
        self.input_registers.insert(COOLING_STATUS, state.info.cooling_status as u16);
        self.input_registers.insert(AMBIENT_TEMPERATURE, ModbusConverter::f64_to_int16_scaled(state.info.ambient_temperature, 10.0) as u16);
        self.input_registers.insert(ENCLOSURE_TEMPERATURE, ModbusConverter::f64_to_int16_scaled(state.info.enclosure_temperature, 10.0) as u16);
        self.input_registers.insert(AUX_POWER_CONSUMPTION, ModbusConverter::f64_to_uint16_scaled(state.info.aux_power_consumption, 10.0));

        Ok(())
    }

    /// Update holding registers from setpoints and config (read/write)
    pub fn update_holding_registers(&mut self, state: &BatteryState) -> Result<()> {
        use addresses::*;

        // Control setpoints
        self.holding_registers.insert(COMMAND, state.setpoints.command as u16);

        let power_setpoint_scaled = ModbusConverter::f64_to_int32_scaled(state.setpoints.power_setpoint, 10.0);
        let (power_high, power_low) = ModbusConverter::int32_to_words(power_setpoint_scaled);
        self.holding_registers.insert(ACTIVE_POWER_SETPOINT, power_high);
        self.holding_registers.insert(ACTIVE_POWER_SETPOINT + 1, power_low);

        let reactive_setpoint_scaled = ModbusConverter::f64_to_int32_scaled(state.setpoints.reactive_power_setpoint, 10.0);
        let (reactive_high, reactive_low) = ModbusConverter::int32_to_words(reactive_setpoint_scaled);
        self.holding_registers.insert(REACTIVE_POWER_SETPOINT, reactive_high);
        self.holding_registers.insert(REACTIVE_POWER_SETPOINT + 1, reactive_low);

        self.holding_registers.insert(POWER_FACTOR_SETPOINT, ModbusConverter::f64_to_int16_scaled(state.setpoints.power_factor_setpoint, 1000.0) as u16);
        self.holding_registers.insert(OPERATING_MODE_SETPOINT, state.setpoints.operating_mode as u16);
        self.holding_registers.insert(GRID_CONNECT_COMMAND, state.setpoints.grid_connect_command as u16);
        self.holding_registers.insert(CURRENT_LIMIT_CHARGE, ModbusConverter::f64_to_uint16_scaled(state.setpoints.current_limit_charge, 10.0));
        self.holding_registers.insert(CURRENT_LIMIT_DISCHARGE, ModbusConverter::f64_to_uint16_scaled(state.setpoints.current_limit_discharge, 10.0));
        self.holding_registers.insert(VOLTAGE_LIMIT_HIGH, ModbusConverter::f64_to_uint16_scaled(state.setpoints.voltage_limit_high, 10.0));
        self.holding_registers.insert(VOLTAGE_LIMIT_LOW, ModbusConverter::f64_to_uint16_scaled(state.setpoints.voltage_limit_low, 10.0));
        self.holding_registers.insert(SOC_LIMIT_HIGH, ModbusConverter::f64_to_uint16_scaled(state.setpoints.soc_limit_high, 10.0));
        self.holding_registers.insert(SOC_LIMIT_LOW, ModbusConverter::f64_to_uint16_scaled(state.setpoints.soc_limit_low, 10.0));
        self.holding_registers.insert(WATCHDOG_TIMEOUT, state.setpoints.watchdog_timeout);

        // Configuration parameters
        let capacity_scaled = ModbusConverter::f64_to_uint32_scaled(state.config.rated_capacity, 10.0);
        let (capacity_high, capacity_low) = ModbusConverter::uint32_to_words(capacity_scaled);
        self.holding_registers.insert(RATED_CAPACITY, capacity_high);
        self.holding_registers.insert(RATED_CAPACITY + 1, capacity_low);

        self.holding_registers.insert(RATED_POWER, ModbusConverter::f64_to_uint16_scaled(state.config.rated_power, 1.0));
        self.holding_registers.insert(RATED_REACTIVE_POWER, ModbusConverter::f64_to_uint16_scaled(state.config.rated_reactive_power, 1.0));
        self.holding_registers.insert(CELL_COUNT, state.config.cell_count);
        self.holding_registers.insert(RATED_AC_VOLTAGE, ModbusConverter::f64_to_uint16_scaled(state.config.rated_ac_voltage, 1.0));
        self.holding_registers.insert(NOMINAL_FREQUENCY, ModbusConverter::f64_to_uint16_scaled(state.config.nominal_frequency, 100.0));
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

        // Active power setpoint (INT32)
        if let (Some(&high), Some(&low)) = (
            self.holding_registers.get(&ACTIVE_POWER_SETPOINT),
            self.holding_registers.get(&(ACTIVE_POWER_SETPOINT + 1))
        ) {
            let power_raw = ModbusConverter::words_to_int32(high, low);
            setpoints.power_setpoint = ModbusConverter::int32_to_f64_scaled(power_raw, 10.0);
        }

        // Reactive power setpoint (INT32)
        if let (Some(&high), Some(&low)) = (
            self.holding_registers.get(&REACTIVE_POWER_SETPOINT),
            self.holding_registers.get(&(REACTIVE_POWER_SETPOINT + 1))
        ) {
            let reactive_raw = ModbusConverter::words_to_int32(high, low);
            setpoints.reactive_power_setpoint = ModbusConverter::int32_to_f64_scaled(reactive_raw, 10.0);
        }

        if let Some(pf) = self.holding_registers.get(&POWER_FACTOR_SETPOINT) {
            setpoints.power_factor_setpoint = ModbusConverter::int16_to_f64_scaled(*pf as i16, 1000.0);
        }

        if let Some(mode) = self.holding_registers.get(&OPERATING_MODE_SETPOINT) {
            setpoints.operating_mode = match *mode {
                0 => OperatingMode::Auto,
                1 => OperatingMode::Manual,
                2 => OperatingMode::Maintenance,
                _ => OperatingMode::Auto,
            };
        }

        if let Some(grid_connect) = self.holding_registers.get(&GRID_CONNECT_COMMAND) {
            setpoints.grid_connect_command = *grid_connect != 0;
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

        if let Some(watchdog) = self.holding_registers.get(&WATCHDOG_TIMEOUT) {
            setpoints.watchdog_timeout = *watchdog;
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
        if let Some(reactive_power) = self.holding_registers.get(&RATED_REACTIVE_POWER) {
            config.rated_reactive_power = ModbusConverter::uint16_to_f64_scaled(*reactive_power, 1.0);
        }
        if let Some(cells) = self.holding_registers.get(&CELL_COUNT) {
            config.cell_count = *cells;
        }
        if let Some(ac_voltage) = self.holding_registers.get(&RATED_AC_VOLTAGE) {
            config.rated_ac_voltage = ModbusConverter::uint16_to_f64_scaled(*ac_voltage, 1.0);
        }
        if let Some(frequency) = self.holding_registers.get(&NOMINAL_FREQUENCY) {
            config.nominal_frequency = ModbusConverter::uint16_to_f64_scaled(*frequency, 100.0);
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

        // Check SOC register (85% * 10 = 850, per ElectricalParams default)
        assert_eq!(map.read_input_register(addresses::SOC), Some(850));

        // Check voltage register (400V * 10 = 4000)
        assert_eq!(map.read_input_register(addresses::BATTERY_VOLTAGE), Some(4000));

        // Check power setpoint extraction
        map.write_holding_register(addresses::ACTIVE_POWER_SETPOINT, 0x03E8).unwrap();  // High word = 1000
        map.write_holding_register(addresses::ACTIVE_POWER_SETPOINT + 1, 0x0000).unwrap();  // Low word = 0
        let setpoints = map.extract_setpoints_from_registers().unwrap();
        assert_eq!(setpoints.power_setpoint, 6553600.0);  // 0x03E80000 / 10 = 65536000 / 10 = 6553600

        // Test a simpler case
        map.write_holding_register(addresses::ACTIVE_POWER_SETPOINT, 0x0000).unwrap();  // High word = 0
        map.write_holding_register(addresses::ACTIVE_POWER_SETPOINT + 1, 0x07D0).unwrap();  // Low word = 2000
        let setpoints = map.extract_setpoints_from_registers().unwrap();
        assert_eq!(setpoints.power_setpoint, 200.0);  // 2000 / 10 = 200
    }

    #[test]
    fn test_ac_and_meter_registers() {
        let mut map = ModbusRegisterMap::new();
        let mut state = BatteryState::default();
        state.ac.real_power = 123.4;
        state.meter.real_power = -50.0;

        map.update_input_registers(&state).unwrap();

        let (high, low) = (
            map.read_input_register(addresses::AC_REAL_POWER).unwrap(),
            map.read_input_register(addresses::AC_REAL_POWER + 1).unwrap(),
        );
        let raw = ModbusConverter::words_to_int32(high, low);
        assert_eq!(ModbusConverter::int32_to_f64_scaled(raw, 10.0), 123.4);

        let (high, low) = (
            map.read_input_register(addresses::METER_REAL_POWER).unwrap(),
            map.read_input_register(addresses::METER_REAL_POWER + 1).unwrap(),
        );
        let raw = ModbusConverter::words_to_int32(high, low);
        assert_eq!(ModbusConverter::int32_to_f64_scaled(raw, 10.0), -50.0);
    }
}
