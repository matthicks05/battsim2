use std::time::{Duration, Instant};
use anyhow::Result;
use tracing::debug;

use super::types::*;
use super::state::BatteryStateManager;

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
        
        // Skip physics simulation when system is offline
        if current_state.status.state == SystemState::Offline {
            // Only apply commands when offline (for start/stop functionality)
            let mut new_status = current_state.status.clone();
            self.apply_commands(&current_state.setpoints, &mut new_status)?;
            self.state_manager.update_status(new_status)?;
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
        let new_cells = self.calculate_cell_monitoring(&new_electrical, &current_state.config)?;
        
        // Update system info (aging, resistance, etc.)
        let mut new_info = current_state.info.clone();
        self.update_aging(&mut new_info, &current_state, effective_dt)?;
        
        // Update temperatures with thermal model
        self.update_thermal_model(&mut new_electrical, &new_cells, current_state.info.clone())?;
        
        // Check for fault conditions
        let mut new_status = current_state.status.clone();
        self.check_fault_conditions(&new_electrical, &new_cells, &mut new_status)?;
        
        // Apply setpoint commands
        self.apply_commands(&current_state.setpoints, &mut new_status)?;

        // Update state manager
        self.state_manager.update_electrical(new_electrical)?;
        self.state_manager.update_cells(new_cells)?;
        self.state_manager.update_info(new_info)?;
        self.state_manager.update_status(new_status)?;
        
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
        let target_power = setpoints.power_setpoint; // kW
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
                // Take the solution that gives current in the right direction
                let i1 = (-b + discriminant.sqrt()) / (2.0 * a);
                let i2 = (-b - discriminant.sqrt()) / (2.0 * a);
                
                // Choose current based on charge/discharge direction
                params.current = if target_power > 0.0 {
                    // Discharging (positive current)
                    i1.max(i2)
                } else {
                    // Charging (negative current)
                    i1.min(i2)
                };
                
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
        
        Ok(())
    }

    /// Calculate cell-level monitoring data
    fn calculate_cell_monitoring(&self, electrical: &ElectricalParams, config: &BatteryConfig) -> Result<CellMonitoring> {
        // Average cell voltage
        let avg_cell_voltage = electrical.voltage / config.cell_count as f64;
        
        // Add realistic cell variation (±1% for voltage, ±2°C for temperature)
        let voltage_variation = avg_cell_voltage * 0.01;
        let temp_variation = 2.0;
        
        // Calculate temperature based on current and ambient
        let current_heating = (electrical.current.abs() / 100.0) * 5.0; // Heat from current
        let base_temp = self.temperature_ambient + current_heating;
        
        Ok(CellMonitoring {
            voltage_min: avg_cell_voltage - voltage_variation,
            voltage_max: avg_cell_voltage + voltage_variation,
            voltage_avg: avg_cell_voltage,
            temp_min: base_temp - temp_variation,
            temp_max: base_temp + temp_variation,
            temp_avg: base_temp,
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
    fn check_fault_conditions(&self, electrical: &ElectricalParams, cells: &CellMonitoring, status: &mut SystemStatus) -> Result<()> {
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