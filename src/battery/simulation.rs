use std::time::{Duration, Instant};
use anyhow::Result;
use tracing::debug;

use super::types::*;
use super::state::BatteryStateManager;

/// Site auxiliary/house load (controls, comms, lighting, cooling standby draw).
/// Shared by `update_environment` and `calculate_meter_params` so the two
/// don't drift if this ever becomes state-dependent.
fn aux_load_power(state: SystemState) -> f64 {
    if state == SystemState::Offline { 0.0 } else { 2.5 }
}

/// Derives apparent and reactive power from real power and power factor
/// (S = P / pf, Q = sqrt(S^2 - P^2), signed to match real_power's direction).
/// Shared by `calculate_ac_params` and `calculate_meter_params` so their
/// zero-guards can't diverge.
fn apparent_and_reactive_power(real_power: f64, power_factor: f64) -> (f64, f64) {
    let apparent_power = if power_factor.abs() > 0.0 {
        real_power.abs() / power_factor.abs()
    } else {
        0.0
    };
    let reactive_magnitude = (apparent_power.powi(2) - real_power.powi(2)).max(0.0).sqrt();
    let reactive_power = if real_power >= 0.0 { reactive_magnitude } else { -reactive_magnitude };
    (apparent_power, reactive_power)
}

/// Battery physics simulation engine
pub struct BatterySimulator {
    state_manager: BatteryStateManager,
    last_update: Instant,
    accumulated_energy: f64,  // For SOC tracking (Wh)
    temperature_ambient: f64, // Ambient temperature (°C)
}

impl BatterySimulator {
    /// Create a new battery simulator
    pub fn new() -> Self {
        Self {
            state_manager: BatteryStateManager::new(),
            last_update: Instant::now(),
            accumulated_energy: 0.0,
            temperature_ambient: 25.0,
        }
    }

    /// Get reference to the state manager
    pub fn state_manager(&self) -> &BatteryStateManager {
        &self.state_manager
    }

    /// Set ambient temperature
    pub fn set_ambient_temperature(&mut self, temp: f64) {
        self.temperature_ambient = temp;
    }
    
    /// Reset accumulated energy to match a specific SOC
    pub fn reset_accumulated_energy_for_soc(&mut self, soc: f64, capacity_kwh: f64) {
        let capacity_wh = capacity_kwh * 1000.0;
        // Reverse the SOC calculation to find the required accumulated_energy
        // soc = ((capacity_wh / 2.0 - accumulated_energy) / capacity_wh * 100.0)
        // Solving for accumulated_energy:
        // accumulated_energy = capacity_wh / 2.0 - (soc * capacity_wh / 100.0)
        self.accumulated_energy = capacity_wh / 2.0 - (soc * capacity_wh / 100.0);
        tracing::info!("Reset accumulated_energy to {:.1} Wh for SOC {:.1}%", self.accumulated_energy, soc);
    }
    
    /// Reset accumulated energy to match current SOC in state
    pub fn sync_accumulated_energy(&mut self) -> Result<()> {
        let state = self.state_manager.get_state()?;
        self.reset_accumulated_energy_for_soc(state.electrical.soc, state.config.rated_capacity);
        Ok(())
    }

    /// Update simulation by one time step
    pub async fn update(&mut self, dt: Duration) -> Result<()> {
        let dt_hours = dt.as_secs_f64() / 3600.0; // Convert to hours
        let current_state = self.state_manager.get_state()?;

        // Skip DC-side physics (current/SOC/cell/aging) while offline - there's no
        // charge/discharge to simulate - but keep refreshing everything derived from
        // status/config (AC, meter, power limits, environment) so those registers
        // reflect the offline state instead of freezing at stale pre-shutdown values.
        if current_state.status.state == SystemState::Offline {
            let mut new_electrical = current_state.electrical.clone();
            new_electrical.current = 0.0;
            new_electrical.power = 0.0;
            self.update_power_limits(&mut new_electrical, &current_state.config)?;

            let mut new_info = current_state.info.clone();
            self.update_environment(&mut new_info, &current_state)?;

            let mut new_status = current_state.status.clone();
            self.apply_commands(&current_state.setpoints, &mut new_status)?;
            self.apply_status_mirrors(&current_state.setpoints, &mut new_status);

            let new_ac = self.calculate_ac_params(&new_electrical, &new_status, &current_state.config)?;
            let new_meter = self.calculate_meter_params(&current_state.meter, &new_ac, &new_status, dt_hours)?;

            self.state_manager.update_all(
                new_electrical,
                current_state.cells.clone(),
                new_info,
                new_status,
                new_ac,
                new_meter,
            )?;
            return Ok(());
        }

        // Get simulation speed multiplier
        let speed_multiplier = current_state.config.simulation_speed;
        let effective_dt = dt_hours * speed_multiplier;

        // Calculate new electrical parameters based on setpoints
        let mut new_electrical = self.calculate_electrical_response(&current_state, effective_dt)?;

        // Update SOC based on current flow
        self.update_soc(&mut new_electrical, &current_state.config, effective_dt)?;

        // Calculate cell-level parameters
        let new_cells = self.calculate_cell_monitoring(&new_electrical, &current_state.config, current_state.info.soh)?;

        // Update dynamic power limits (SOC/thermal derated)
        self.update_power_limits(&mut new_electrical, &current_state.config)?;

        // Update system info (aging, resistance, etc.)
        let mut new_info = current_state.info.clone();
        self.update_aging(&mut new_info, &current_state, effective_dt)?;
        self.update_environment(&mut new_info, &current_state)?;

        // Update temperatures with thermal model
        self.update_thermal_model(&mut new_electrical, &new_cells, current_state.info.clone())?;

        // Check for fault conditions
        let mut new_status = current_state.status.clone();
        self.check_fault_conditions(&new_electrical, &new_cells, &current_state.setpoints, &mut new_status)?;

        // Apply setpoint commands
        self.apply_commands(&current_state.setpoints, &mut new_status)?;
        self.apply_status_mirrors(&current_state.setpoints, &mut new_status);

        // Calculate AC-side (PCS) parameters from the DC-side result
        let new_ac = self.calculate_ac_params(&new_electrical, &new_status, &current_state.config)?;

        // Calculate meter/POI parameters from the AC-side result
        let new_meter = self.calculate_meter_params(&current_state.meter, &new_ac, &new_status, effective_dt)?;

        // Update state manager - one lock/clone/broadcast for the whole tick
        // instead of six separate update_* calls.
        self.state_manager.update_all(new_electrical, new_cells, new_info, new_status, new_ac, new_meter)?;

        // Update timing
        self.state_manager.increment_uptime(dt)?;
        self.last_update = Instant::now();

        Ok(())
    }

    /// Calculate electrical response to power setpoint
    fn calculate_electrical_response(&self, state: &BatteryState, _dt_hours: f64) -> Result<ElectricalParams> {
        let mut params = state.electrical.clone();
        let setpoints = &state.setpoints;
        let config = &state.config;

        // Calculate internal resistance (temperature and SOC dependent)
        let internal_resistance = self.calculate_internal_resistance(&state.electrical, &state.info)?;
        
        // Power control logic
        // Enforce the SOC/thermal-derated power limits (MAX_CHARGE_POWER/MAX_DISCHARGE_POWER,
        // computed last tick by update_power_limits) so the published limit registers can't
        // contradict what the simulator actually delivers.
        let target_power = setpoints.power_setpoint
            .clamp(-params.max_charge_power, params.max_discharge_power); // kW
        debug!("Battery simulation: target_power = {:.1} kW, current_soc = {:.1}%", target_power, params.soc);
        
        if target_power.abs() < 0.1 {
            // Standby mode - minimal current
            params.current = 0.0;
            params.power = 0.0;
        } else {
            // Calculate required current for target power
            // P = V*I - I²*R (accounting for internal losses)
            let voltage_oc = self.calculate_open_circuit_voltage(params.soc, config)?;
            
            // Solve quadratic equation: P = V*I - I²*R
            // Rearranged: R*I² - V*I + P = 0
            let a = internal_resistance / 1000.0; // Convert mΩ to Ω
            let b = -voltage_oc;
            let c = target_power * 1000.0; // Convert kW to W
            
            let discriminant = b * b - 4.0 * a * c;
            
            if discriminant >= 0.0 {
                // Both roots satisfy V*I - I²*R = P exactly, but they represent two very
                // different operating points: a low-current/low-loss solution close to the
                // naive P/V estimate, and a high-current/high-loss solution where most of the
                // gross power is burned as I²R and only the remainder reaches the terminals.
                // A real battery/PCS always regulates to the low-current solution, so we must
                // pick the root with the smaller current magnitude, not just "a" valid root.
                let i1 = (-b + discriminant.sqrt()) / (2.0 * a);
                let i2 = (-b - discriminant.sqrt()) / (2.0 * a);

                params.current = if i1.abs() <= i2.abs() { i1 } else { i2 };
                
                // Apply current limits
                params.current = params.current.clamp(
                    -setpoints.current_limit_charge,
                    setpoints.current_limit_discharge
                );
            } else {
                // Power setpoint not achievable at current voltage
                params.current = 0.0;
            }
            
            // Calculate actual voltage under load
            params.voltage = voltage_oc - (params.current * internal_resistance / 1000.0);
            
            // Calculate actual power
            params.power = params.voltage * params.current / 1000.0; // Convert to kW
            debug!("Battery simulation: calculated current = {:.1} A, voltage = {:.1} V, actual power = {:.1} kW", 
                   params.current, params.voltage, params.power);
        }

        // Apply voltage limits
        params.voltage = params.voltage.clamp(config.min_voltage, config.max_voltage);
        
        Ok(params)
    }

    /// Update State of Charge based on current integration
    fn update_soc(&mut self, params: &mut ElectricalParams, config: &BatteryConfig, dt_hours: f64) -> Result<()> {
        // Energy change in this time step (Wh)
        let energy_delta = params.power * 1000.0 * dt_hours; // Convert kW to W, then to Wh
        
        // Update accumulated energy (negative = charging, positive = discharging)
        self.accumulated_energy += energy_delta;
        
        // Calculate new SOC
        let capacity_wh = config.rated_capacity * 1000.0; // Convert kWh to Wh
        params.soc = ((capacity_wh / 2.0 - self.accumulated_energy) / capacity_wh * 100.0)
            .clamp(0.0, 100.0);
        
        // Update energy availability
        params.available_energy = (params.soc / 100.0) * config.rated_capacity;
        params.remaining_capacity = ((100.0 - params.soc) / 100.0) * config.rated_capacity;
        
        // Apply SOC-based self discharge (3% per month)
        let self_discharge_rate = 0.03 / (30.0 * 24.0); // Per hour
        let self_discharge = params.soc * self_discharge_rate * dt_hours;
        params.soc = (params.soc - self_discharge).max(0.0);

        // Track cumulative lifetime charge/discharge energy (never reset)
        if params.power > 0.0 {
            params.lifetime_discharge_energy += params.power * dt_hours;
        } else if params.power < 0.0 {
            params.lifetime_charge_energy += -params.power * dt_hours;
        }

        Ok(())
    }

    /// Update dynamic charge/discharge power limits based on SOC headroom
    fn update_power_limits(&self, params: &mut ElectricalParams, config: &BatteryConfig) -> Result<()> {
        let charge_derate = if params.soc > 95.0 {
            0.2
        } else if params.soc > 90.0 {
            0.6
        } else {
            1.0
        };

        let discharge_derate = if params.soc < 5.0 {
            0.2
        } else if params.soc < 10.0 {
            0.6
        } else {
            1.0
        };

        params.max_charge_power = config.rated_power * charge_derate;
        params.max_discharge_power = config.rated_power * discharge_derate;

        Ok(())
    }

    /// Update ambient/enclosure temperature and auxiliary load reporting
    fn update_environment(&self, info: &mut SystemInfo, state: &BatteryState) -> Result<()> {
        info.ambient_temperature = self.temperature_ambient;
        info.enclosure_temperature = self.temperature_ambient + 3.0;
        info.aux_power_consumption = aux_load_power(state.status.state);
        Ok(())
    }

    /// Mirror a subset of write-only setpoints back into status for read-back
    fn apply_status_mirrors(&self, setpoints: &ControlSetpoints, status: &mut SystemStatus) {
        status.operating_mode = setpoints.operating_mode;
        // The main breaker can't be reporting "closed" while Offline or while
        // a fault has tripped it open.
        status.grid_connected = setpoints.grid_connect_command
            && status.state != SystemState::Offline
            && status.state != SystemState::Fault;
    }

    /// Derive AC-side (PCS) measurements from the DC-side electrical result
    fn calculate_ac_params(&self, electrical: &ElectricalParams, status: &SystemStatus, config: &BatteryConfig) -> Result<AcParams> {
        const PCS_EFFICIENCY: f64 = 0.97;

        let pcs_state = if !status.faults.is_empty() {
            PcsState::Fault
        } else {
            match status.state {
                SystemState::Offline => PcsState::Standby,
                SystemState::Charging | SystemState::Discharging => PcsState::Running,
                SystemState::Fault => PcsState::Fault,
                _ => PcsState::Standby,
            }
        };

        // Apply converter losses: discharging loses some power to AC, charging draws extra from AC
        let real_power = if electrical.power > 0.0 {
            electrical.power * PCS_EFFICIENCY
        } else if electrical.power < 0.0 {
            electrical.power / PCS_EFFICIENCY
        } else {
            0.0
        };

        let power_factor: f64 = if real_power.abs() > 0.1 { 0.99 } else { 1.0 };
        let (apparent_power, reactive_power) = apparent_and_reactive_power(real_power, power_factor);

        // Small grid frequency droop proportional to loading, typical of grid-following inverters
        let loading_fraction = if config.rated_power > 0.0 { real_power / config.rated_power } else { 0.0 };
        let frequency = (config.nominal_frequency - loading_fraction * 0.02).clamp(59.5, 60.5);

        let voltage_ln = config.rated_ac_voltage / 3f64.sqrt();
        let current_per_phase = if voltage_ln > 0.0 {
            (apparent_power * 1000.0) / (3.0 * voltage_ln)
        } else {
            0.0
        };

        let pcs_temperature = self.temperature_ambient + (current_per_phase / 200.0) * 10.0;

        Ok(AcParams {
            pcs_state,
            grid_forming: false,
            frequency,
            power_factor,
            real_power,
            reactive_power,
            apparent_power,
            voltage_l1n: voltage_ln,
            voltage_l2n: voltage_ln,
            voltage_l3n: voltage_ln,
            current_l1: current_per_phase,
            current_l2: current_per_phase,
            current_l3: current_per_phase,
            pcs_temperature,
            isolation_resistance: 500.0,
        })
    }

    /// Derive meter/POI measurements from the AC-side (PCS) result
    fn calculate_meter_params(&self, previous_meter: &MeterParams, ac: &AcParams, status: &SystemStatus, dt_hours: f64) -> Result<MeterParams> {
        let aux_load = aux_load_power(status.state);
        let real_power = ac.real_power - aux_load;
        let (_, reactive_power) = apparent_and_reactive_power(real_power, ac.power_factor);

        let mut lifetime_import_energy = previous_meter.lifetime_import_energy;
        let mut lifetime_export_energy = previous_meter.lifetime_export_energy;
        if real_power > 0.0 {
            lifetime_export_energy += real_power * dt_hours;
        } else if real_power < 0.0 {
            lifetime_import_energy += -real_power * dt_hours;
        }

        Ok(MeterParams {
            voltage_l1n: ac.voltage_l1n,
            voltage_l2n: ac.voltage_l2n,
            voltage_l3n: ac.voltage_l3n,
            current_l1: ac.current_l1,
            current_l2: ac.current_l2,
            current_l3: ac.current_l3,
            frequency: ac.frequency,
            real_power,
            reactive_power,
            lifetime_import_energy,
            lifetime_export_energy,
            aux_load_power: aux_load,
        })
    }

    /// Calculate cell-level monitoring data
    fn calculate_cell_monitoring(&self, electrical: &ElectricalParams, config: &BatteryConfig, soh: f64) -> Result<CellMonitoring> {
        // Average cell voltage
        let avg_cell_voltage = electrical.voltage / config.cell_count as f64;

        // Cell-to-cell voltage variation grows as the pack ages (SOH degrades from 100%
        // down to the 50% floor in update_aging), so BALANCING_CELLS/voltage_delta respond
        // to real aging state instead of always reporting the same fixed spread. Roughly:
        // negligible spread above ~93% SOH, BALANCING_CELLS active from ~93%, imbalance
        // warning from ~81%, imbalance fault from ~62% down to the 50% floor.
        let aging_factor = 100.0 / soh.max(50.0);
        let excess_aging = (aging_factor - 1.0).max(0.0);
        let voltage_variation = avg_cell_voltage * (0.003 + 0.02 * excess_aging);
        let temp_variation = 2.0;

        // Calculate temperature based on current and ambient
        let current_heating = (electrical.current.abs() / 100.0) * 5.0; // Heat from current
        let base_temp = self.temperature_ambient + current_heating;

        let voltage_delta = voltage_variation * 2.0;
        let balancing_cells = if voltage_delta > 0.03 {
            (config.cell_count as f64 * 0.1).round() as u16
        } else {
            0
        };

        Ok(CellMonitoring {
            voltage_min: avg_cell_voltage - voltage_variation,
            voltage_max: avg_cell_voltage + voltage_variation,
            voltage_avg: avg_cell_voltage,
            voltage_delta,
            temp_min: base_temp - temp_variation,
            temp_max: base_temp + temp_variation,
            temp_avg: base_temp,
            balancing_cells,
        })
    }

    /// Calculate open circuit voltage based on SOC
    fn calculate_open_circuit_voltage(&self, soc: f64, config: &BatteryConfig) -> Result<f64> {
        // LiFePO4 voltage curve approximation
        let soc_normalized = (soc / 100.0).clamp(0.0, 1.0);
        
        // Typical LiFePO4 cell OCV curve (3.2V nominal)
        let cell_voltage = if soc_normalized > 0.95 {
            3.6 - (soc_normalized - 0.95) * 4.0 // Voltage rise at high SOC
        } else if soc_normalized > 0.05 {
            3.2 + (soc_normalized - 0.5) * 0.4 // Flat region
        } else {
            3.0 - (0.05 - soc_normalized) * 6.0 // Voltage drop at low SOC
        };
        
        Ok(cell_voltage * config.cell_count as f64)
    }

    /// Calculate internal resistance with temperature and SOC dependencies
    fn calculate_internal_resistance(&self, electrical: &ElectricalParams, info: &SystemInfo) -> Result<f64> {
        let base_resistance = 0.5; // mΩ per cell (new battery)
        
        // SOC dependency (higher at extremes)
        let soc_factor = if electrical.soc < 10.0 {
            1.0 + (10.0 - electrical.soc) * 0.1
        } else if electrical.soc > 90.0 {
            1.0 + (electrical.soc - 90.0) * 0.05
        } else {
            1.0
        };
        
        // Temperature dependency (25°C reference)
        let temp_factor = if self.temperature_ambient < 25.0 {
            1.0 + (25.0 - self.temperature_ambient) * 0.02
        } else {
            1.0
        };
        
        // Aging factor based on SOH
        let aging_factor = 100.0 / info.soh;
        
        Ok(base_resistance * soc_factor * temp_factor * aging_factor * 128.0) // 128 cells
    }

    /// Update thermal model
    fn update_thermal_model(&self, electrical: &mut ElectricalParams, cells: &CellMonitoring, _info: SystemInfo) -> Result<()> {
        // Thermal derating above 45°C
        if cells.temp_avg > 45.0 {
            let derating_factor = 1.0 - ((cells.temp_avg - 45.0) / 10.0).min(0.5);
            electrical.current *= derating_factor;
            electrical.power *= derating_factor;
        }
        
        Ok(())
    }

    /// Update aging model (SOH degradation)
    fn update_aging(&self, info: &mut SystemInfo, state: &BatteryState, dt_hours: f64) -> Result<()> {
        // Calendar aging (0.05% per month at 25°C)
        let calendar_aging_rate = 0.0005 / (30.0 * 24.0); // Per hour
        let temp_acceleration = if self.temperature_ambient > 25.0 {
            1.0 + (self.temperature_ambient - 25.0) * 0.02
        } else {
            1.0
        };
        
        let calendar_degradation = calendar_aging_rate * temp_acceleration * dt_hours;
        
        // Cycle aging (based on energy throughput)
        let energy_throughput = state.electrical.power.abs() * dt_hours; // kWh
        let cycle_degradation = if energy_throughput > 0.0 {
            energy_throughput / (6000.0 * state.config.rated_capacity) * 0.2 // 20% degradation over 6000 cycles
        } else {
            0.0
        };
        
        // Apply aging
        info.soh = (info.soh - (calendar_degradation + cycle_degradation) * 100.0).max(50.0);
        
        // Update cycle count if significant energy throughput
        if energy_throughput > state.config.rated_capacity * 0.1 {
            info.cycle_count += (energy_throughput / state.config.rated_capacity * 100.0) as u32;
        }
        
        // Update internal resistance based on aging
        info.internal_resistance = 64.0 * (100.0 / info.soh);
        
        Ok(())
    }

    /// Check for fault conditions
    fn check_fault_conditions(&self, electrical: &ElectricalParams, cells: &CellMonitoring, setpoints: &ControlSetpoints, status: &mut SystemStatus) -> Result<()> {
        // Clear existing warnings for re-evaluation (but keep faults until explicitly reset)
        status.warnings.clear();

        // Check and add new faults (avoiding duplicates)
        if electrical.voltage > 450.0 && !status.faults.contains(&FaultCode::OverVoltage) {
            status.faults.push(FaultCode::OverVoltage);
        }
        if electrical.voltage < 350.0 && !status.faults.contains(&FaultCode::UnderVoltage) {
            status.faults.push(FaultCode::UnderVoltage);
        }
        
        // Current faults
        if electrical.current.abs() > 750.0 && !status.faults.contains(&FaultCode::OverCurrent) {
            status.faults.push(FaultCode::OverCurrent);
        }
        
        // Temperature faults
        if cells.temp_max > 55.0 && !status.faults.contains(&FaultCode::OverTemperature) {
            status.faults.push(FaultCode::OverTemperature);
        }
        if cells.temp_min < -15.0 && !status.faults.contains(&FaultCode::UnderTemperature) {
            status.faults.push(FaultCode::UnderTemperature);
        }
        
        // Cell voltage faults
        if cells.voltage_max > 3.65 && !status.faults.contains(&FaultCode::OverVoltage) {
            status.faults.push(FaultCode::OverVoltage);
        }
        if cells.voltage_min < 2.5 && !status.faults.contains(&FaultCode::UnderVoltage) {
            status.faults.push(FaultCode::UnderVoltage);
        }

        // Severe cell-to-cell imbalance (beyond the CellImbalanceWarning threshold below)
        if cells.voltage_delta > 0.10 && !status.faults.contains(&FaultCode::CellImbalanceFault) {
            status.faults.push(FaultCode::CellImbalanceFault);
        }

        // EMS comms watchdog: self-clearing, since a stale watchdog is a liveness signal
        // rather than a hardware fault - it should recover on its own once writes resume.
        // watchdog_timeout == 0 means the watchdog is disabled.
        if setpoints.watchdog_timeout > 0 {
            let stale = self
                .state_manager
                .time_since_setpoints_update()
                .map(|elapsed| elapsed > Duration::from_secs(setpoints.watchdog_timeout as u64))
                .unwrap_or(false);
            if stale {
                if !status.faults.contains(&FaultCode::WatchdogTimeout) {
                    status.faults.push(FaultCode::WatchdogTimeout);
                }
            } else {
                status.faults.retain(|f| *f != FaultCode::WatchdogTimeout);
            }
        }

        // Warnings
        if electrical.soc > 95.0 {
            status.warnings.push(WarningCode::HighSocWarning);
        }
        if electrical.soc < 10.0 {
            status.warnings.push(WarningCode::LowSocWarning);
        }
        if cells.temp_max > 45.0 {
            status.warnings.push(WarningCode::HighTemperatureWarning);
        }
        if (cells.voltage_max - cells.voltage_min) > 0.05 {
            status.warnings.push(WarningCode::CellImbalanceWarning);
        }
        
        // Set system state based on faults
        if !status.faults.is_empty() {
            status.state = SystemState::Fault;
        } else if electrical.power > 1.0 {
            status.state = SystemState::Discharging;
        } else if electrical.power < -1.0 {
            status.state = SystemState::Charging;
        } else {
            status.state = SystemState::Standby;
        }
        
        Ok(())
    }

    /// Apply system commands
    fn apply_commands(&mut self, setpoints: &ControlSetpoints, status: &mut SystemStatus) -> Result<()> {
        match setpoints.command {
            SystemCommand::Start => {
                if status.state == SystemState::Offline {
                    status.state = SystemState::Standby;
                    status.online = true; // System is now online
                    // Sync accumulated energy to match current SOC when starting
                    if let Err(e) = self.sync_accumulated_energy() {
                        tracing::error!("Failed to sync accumulated energy on start: {}", e);
                    }
                }
            }
            SystemCommand::Stop => {
                status.state = SystemState::Offline;
                status.online = false; // System is now offline
            }
            SystemCommand::EmergencyStop => {
                status.state = SystemState::Offline;
                status.online = false; // System is now offline
                status.faults.push(FaultCode::InternalFault);
            }
            SystemCommand::ResetFaults => {
                if status.state == SystemState::Fault {
                    status.faults.clear();
                    status.state = SystemState::Standby;
                    status.online = true; // System is back online
                }
            }
            SystemCommand::MaintenanceMode => {
                status.state = SystemState::Maintenance;
                status.online = true; // System is online but in maintenance mode
            }
            SystemCommand::NoCommand => {
                // No action required
            }
        }
        
        Ok(())
    }
}

impl Default for BatterySimulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_simulator_creation() {
        let sim = BatterySimulator::new();
        let state = sim.state_manager.get_state().unwrap();
        
        assert_eq!(state.electrical.soc, 50.0);
        assert_eq!(state.status.state, SystemState::Standby);
    }
    
    #[tokio::test]
    async fn test_offline_registers_refresh_instead_of_freezing() {
        let mut sim = BatterySimulator::new();

        // Start and discharge so AC/meter go non-zero.
        let mut setpoints = ControlSetpoints::default();
        setpoints.command = SystemCommand::Start;
        setpoints.power_setpoint = 50.0;
        sim.state_manager.update_setpoints(setpoints.clone()).unwrap();
        sim.update(Duration::from_millis(100)).await.unwrap();
        sim.update(Duration::from_millis(100)).await.unwrap();
        let state = sim.state_manager.get_state().unwrap();
        assert!(state.ac.real_power > 0.0, "expected non-zero AC power before stopping");

        // Stop, then tick again - AC/meter must reflect Offline, not stay frozen.
        setpoints.command = SystemCommand::Stop;
        sim.state_manager.update_setpoints(setpoints).unwrap();
        sim.update(Duration::from_millis(100)).await.unwrap();
        sim.update(Duration::from_millis(100)).await.unwrap();

        let state = sim.state_manager.get_state().unwrap();
        assert_eq!(state.status.state, SystemState::Offline);
        assert_eq!(state.ac.real_power, 0.0, "AC power should zero out, not freeze at the pre-stop value");
        assert_eq!(state.meter.real_power, 0.0, "meter power should zero out, not freeze at the pre-stop value");
        assert_eq!(state.ac.pcs_state, PcsState::Standby);
        assert!(!state.status.grid_connected);
    }

    #[tokio::test]
    async fn test_grid_connected_false_during_fault() {
        let sim = BatterySimulator::new();
        // grid_connect_command defaults to true; a fault should still force the
        // reported breaker state to open regardless.
        sim.state_manager.add_fault(FaultCode::InternalFault).unwrap();
        let mut sim = sim;
        sim.update(Duration::from_millis(100)).await.unwrap();

        let state = sim.state_manager.get_state().unwrap();
        assert_eq!(state.status.state, SystemState::Fault);
        assert!(!state.status.grid_connected, "GRID_CONNECTED must not report closed during an active fault");
    }

    #[tokio::test]
    async fn test_power_limit_actually_enforced() {
        let mut sim = BatterySimulator::new();

        let mut setpoints = ControlSetpoints::default();
        setpoints.command = SystemCommand::Start;
        sim.state_manager.update_setpoints(setpoints.clone()).unwrap();
        sim.state_manager.init_soc(96.0).unwrap(); // charge_derate = 0.2 above 95% SOC
        sim.update(Duration::from_millis(100)).await.unwrap(); // computes max_charge_power from this SOC

        let limited_state = sim.state_manager.get_state().unwrap();
        let max_charge_power = limited_state.electrical.max_charge_power;
        assert!(max_charge_power < 250.0, "expected a derated charge limit near 96% SOC");

        // Command a much larger charge than the derated limit allows.
        setpoints.power_setpoint = -200.0;
        sim.state_manager.update_setpoints(setpoints).unwrap();
        sim.update(Duration::from_millis(100)).await.unwrap();

        let state = sim.state_manager.get_state().unwrap();
        assert!(
            state.electrical.power.abs() <= max_charge_power + 1.0,
            "delivered charge power {} exceeded the published MAX_CHARGE_POWER limit {}",
            state.electrical.power, max_charge_power
        );
    }

    #[tokio::test]
    async fn test_watchdog_timeout_fault_is_self_clearing() {
        let mut sim = BatterySimulator::new();

        let mut setpoints = ControlSetpoints::default();
        setpoints.command = SystemCommand::Start;
        setpoints.watchdog_timeout = 1; // 1 second
        sim.state_manager.update_setpoints(setpoints.clone()).unwrap();
        sim.update(Duration::from_millis(100)).await.unwrap();

        let state = sim.state_manager.get_state().unwrap();
        assert!(!state.status.faults.contains(&FaultCode::WatchdogTimeout));

        tokio::time::sleep(Duration::from_millis(1200)).await;
        sim.update(Duration::from_millis(100)).await.unwrap();
        let state = sim.state_manager.get_state().unwrap();
        assert!(state.status.faults.contains(&FaultCode::WatchdogTimeout), "expected a stale watchdog to fault");

        // A fresh setpoints write resets the watchdog; it should self-clear.
        sim.state_manager.update_setpoints(setpoints).unwrap();
        sim.update(Duration::from_millis(100)).await.unwrap();
        let state = sim.state_manager.get_state().unwrap();
        assert!(!state.status.faults.contains(&FaultCode::WatchdogTimeout), "watchdog fault should self-clear once writes resume");
    }

    #[tokio::test]
    async fn test_soc_update() {
        let mut sim = BatterySimulator::new();
        
        // Set discharge power
        let mut setpoints = ControlSetpoints::default();
        setpoints.power_setpoint = 100.0; // 100 kW discharge
        sim.state_manager.update_setpoints(setpoints).unwrap();
        
        // Run simulation for 1 hour
        sim.update(Duration::from_secs(3600)).await.unwrap();
        
        let state = sim.state_manager.get_state().unwrap();
        
        // SOC should decrease due to discharge
        assert!(state.electrical.soc < 50.0);
        assert!(state.electrical.current > 0.0); // Positive current = discharge
        assert!(state.electrical.power > 0.0);   // Positive power = discharge
    }
    
    #[tokio::test]
    async fn test_fault_detection() {
        let mut sim = BatterySimulator::new();
        
        // Test adding a fault
        sim.state_manager.add_fault(FaultCode::InternalFault).unwrap();
        let state = sim.state_manager.get_state().unwrap();
        assert_eq!(state.status.state, SystemState::Fault);
        assert!(state.status.faults.contains(&FaultCode::InternalFault));
        
        // Test reset fault command
        let mut setpoints = ControlSetpoints::default();
        setpoints.command = SystemCommand::ResetFaults;
        sim.state_manager.update_setpoints(setpoints).unwrap();
        
        // Run simulation to apply reset command
        sim.update(Duration::from_millis(100)).await.unwrap();
        
        let state = sim.state_manager.get_state().unwrap();
        assert_eq!(state.status.state, SystemState::Standby); // Should be cleared
        assert!(state.status.faults.is_empty()); // Faults should be cleared
        
        // Test emergency stop command
        let mut setpoints = ControlSetpoints::default();
        setpoints.command = SystemCommand::EmergencyStop;
        sim.state_manager.update_setpoints(setpoints).unwrap();
        
        sim.update(Duration::from_millis(100)).await.unwrap();
        
        let state = sim.state_manager.get_state().unwrap();
        assert_eq!(state.status.state, SystemState::Offline);
        assert!(state.status.faults.contains(&FaultCode::InternalFault));
    }
}