use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tokio::sync::watch;
use anyhow::Result;
use tracing::{info, debug};

use super::types::*;

/// Thread-safe battery state manager
#[derive(Clone)]
pub struct BatteryStateManager {
    state: Arc<RwLock<BatteryState>>,
    last_update: Arc<RwLock<Instant>>,
    tx: watch::Sender<BatteryState>,
    rx: watch::Receiver<BatteryState>,
}

impl BatteryStateManager {
    /// Create a new battery state manager with default values
    pub fn new() -> Self {
        let initial_state = BatteryState::default();
        let (tx, rx) = watch::channel(initial_state.clone());
        
        Self {
            state: Arc::new(RwLock::new(initial_state)),
            last_update: Arc::new(RwLock::new(Instant::now())),
            tx,
            rx,
        }
    }
    
    /// Get a clone of the current battery state
    pub fn get_state(&self) -> Result<BatteryState> {
        let state = self.state.read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock"))?;
        Ok(state.clone())
    }
    
    /// Update electrical parameters
    pub fn update_electrical(&self, params: ElectricalParams) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.electrical = params;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Update cell monitoring data
    pub fn update_cells(&self, cells: CellMonitoring) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.cells = cells;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }

    /// Update AC-side (PCS) measurements
    pub fn update_ac(&self, ac: AcParams) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.ac = ac;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }

    /// Update meter/POI measurements
    pub fn update_meter(&self, meter: MeterParams) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.meter = meter;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Update system status
    pub fn update_status(&self, status: SystemStatus) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.status = status;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Update system info
    pub fn update_info(&self, info: SystemInfo) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.info = info;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Update control setpoints
    pub fn update_setpoints(&self, setpoints: ControlSetpoints) -> Result<()> {
        info!("State manager: Updating setpoints - power: {:.1} kW, command: {:?}", 
              setpoints.power_setpoint, setpoints.command);
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.setpoints = setpoints;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        debug!("State manager: Setpoints updated and broadcast");
        Ok(())
    }
    
    /// Update battery configuration
    pub fn update_config(&self, config: BatteryConfig) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.config = config;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Update complete battery state
    pub fn update_state(&self, new_state: BatteryState) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        *state = new_state;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Initialize SOC and recalculate dependent values
    pub fn init_soc(&self, soc: f64) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        state.electrical.soc = soc.clamp(0.0, 100.0);
        let capacity_kwh = state.config.rated_capacity; // Already in kWh
        state.electrical.available_energy = (soc / 100.0) * capacity_kwh;
        state.electrical.remaining_capacity = ((100.0 - soc) / 100.0) * capacity_kwh;
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Initialized SOC to {:.1}%", soc);
        Ok(())
    }
    
    /// Initialize battery capacity and recalculate dependent values
    pub fn init_capacity(&self, capacity_kwh: f64) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        let capacity_kwh_clamped = capacity_kwh.clamp(100.0, 1000.0); // 100kWh to 1000kWh
        state.config.rated_capacity = capacity_kwh_clamped;
        
        // Recalculate available energy based on current SOC
        state.electrical.available_energy = (state.electrical.soc / 100.0) * capacity_kwh_clamped;
        state.electrical.remaining_capacity = ((100.0 - state.electrical.soc) / 100.0) * capacity_kwh_clamped;
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Initialized capacity to {:.1} kWh", capacity_kwh);
        Ok(())
    }
    
    /// Initialize nominal voltage
    pub fn init_voltage(&self, voltage: f64) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        state.config.nominal_voltage = voltage.clamp(350.0, 450.0);
        state.electrical.voltage = voltage; // Also update current voltage
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Initialized voltage to {:.1} V", voltage);
        Ok(())
    }
    
    /// Initialize temperature for all cells
    pub fn init_temperature(&self, temp: f64) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        let clamped_temp = temp.clamp(-10.0, 50.0);
        state.cells.temp_min = clamped_temp;
        state.cells.temp_max = clamped_temp;
        state.cells.temp_avg = clamped_temp;
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Initialized temperature to {:.1}°C", temp);
        Ok(())
    }
    
    /// Initialize State of Health
    pub fn init_soh(&self, soh: f64) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        state.info.soh = soh.clamp(50.0, 100.0);
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Initialized SOH to {:.1}%", soh);
        Ok(())
    }
    
    /// Initialize internal resistance
    pub fn init_resistance(&self, resistance_mohm: f64) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        // Convert mΩ to Ω for internal storage
        state.info.internal_resistance = (resistance_mohm / 1000.0).clamp(0.0001, 0.01);
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Initialized internal resistance to {:.2} mΩ", resistance_mohm);
        Ok(())
    }
    
    /// Initialize cycle count
    pub fn init_cycles(&self, cycles: u32) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        state.info.cycle_count = cycles.min(10000);
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Initialized cycle count to {}", cycles);
        Ok(())
    }
    
    /// Reset all parameters to defaults
    pub fn reset_to_defaults(&self) -> Result<()> {
        let default_state = BatteryState::default();
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        // Reset all values to defaults
        state.electrical = default_state.electrical;
        state.cells = default_state.cells;
        state.info = default_state.info;
        state.config = default_state.config;
        // Keep current status and setpoints
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        info!("Reset battery parameters to defaults");
        Ok(())
    }

    /// Get a watch receiver for state changes
    pub fn subscribe(&self) -> watch::Receiver<BatteryState> {
        self.rx.clone()
    }
    
    /// Get the time since last update
    pub fn time_since_update(&self) -> Result<Duration> {
        let last_update = self.last_update.read()
            .map_err(|_| anyhow::anyhow!("Failed to acquire read lock"))?;
        Ok(last_update.elapsed())
    }
    
    /// Check if data is stale (>2 seconds since update)
    pub fn is_stale(&self) -> Result<bool> {
        Ok(self.time_since_update()? > Duration::from_secs(2))
    }
    
    /// Add a fault to the system status
    pub fn add_fault(&self, fault: FaultCode) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        if !state.status.faults.contains(&fault) {
            state.status.faults.push(fault);
            
            // Set system state to fault for any fault
            state.status.state = SystemState::Fault;
        }
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Remove a fault from the system status
    pub fn clear_fault(&self, fault: FaultCode) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        state.status.faults.retain(|&f| f != fault);
        
        // Clear fault state if no critical faults remain
        if state.status.faults.is_empty() && state.status.state == SystemState::Fault {
            state.status.state = SystemState::Standby;
        }
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Add a warning to the system status
    pub fn add_warning(&self, warning: WarningCode) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        if !state.status.warnings.contains(&warning) {
            state.status.warnings.push(warning);
        }
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Remove a warning from the system status
    pub fn clear_warning(&self, warning: WarningCode) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        state.status.warnings.retain(|&w| w != warning);
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Clear all faults and warnings
    pub fn clear_all_faults_and_warnings(&self) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        
        state.status.faults.clear();
        state.status.warnings.clear();
        
        if state.status.state == SystemState::Fault {
            state.status.state = SystemState::Standby;
        }
        
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Update system state
    pub fn set_system_state(&self, new_state: SystemState) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.status.state = new_state;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Increment uptime
    pub fn increment_uptime(&self, duration: Duration) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.status.uptime += duration;
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Set Modbus client count
    pub fn set_modbus_clients(&self, count: u32) -> Result<()> {
        let mut state = self.state.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        state.status.modbus_clients = count;
        // Note: Don't override online status - that's controlled by system state
        self.update_timestamp()?;
        let _ = self.tx.send(state.clone());
        Ok(())
    }
    
    /// Update the last update timestamp
    fn update_timestamp(&self) -> Result<()> {
        let mut last_update = self.last_update.write()
            .map_err(|_| anyhow::anyhow!("Failed to acquire write lock"))?;
        *last_update = Instant::now();
        Ok(())
    }
}

impl Default for BatteryStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_manager_creation() {
        let manager = BatteryStateManager::new();
        let state = manager.get_state().unwrap();
        
        assert_eq!(state.electrical.voltage, 400.0);
        assert_eq!(state.status.state, SystemState::Standby);
        assert!(state.status.faults.is_empty());
    }
    
    #[test]
    fn test_fault_management() {
        let manager = BatteryStateManager::new();
        
        // Add fault
        manager.add_fault(FaultCode::OverVoltage).unwrap();
        let state = manager.get_state().unwrap();
        assert!(state.status.faults.contains(&FaultCode::OverVoltage));
        assert_eq!(state.status.state, SystemState::Fault);
        
        // Clear fault
        manager.clear_fault(FaultCode::OverVoltage).unwrap();
        let state = manager.get_state().unwrap();
        assert!(!state.status.faults.contains(&FaultCode::OverVoltage));
        assert_eq!(state.status.state, SystemState::Standby);
    }
    
    #[test]
    fn test_electrical_update() {
        let manager = BatteryStateManager::new();
        
        let params = ElectricalParams {
            voltage: 410.5,
            current: -50.0,
            power: -20.5,
            soc: 75.2,
            available_energy: 376.0,
            remaining_capacity: 124.0,
            ..Default::default()
        };
        
        manager.update_electrical(params.clone()).unwrap();
        let state = manager.get_state().unwrap();
        
        assert_eq!(state.electrical.voltage, 410.5);
        assert_eq!(state.electrical.current, -50.0);
        assert_eq!(state.electrical.soc, 75.2);
    }
}