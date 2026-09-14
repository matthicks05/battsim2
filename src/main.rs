use anyhow::Result;
use tracing::{info, error};
use std::time::Duration;
use tokio::time;
use tokio::sync::mpsc;

mod battery;
mod modbus;
mod ui;

use battery::{BatterySimulator, ControlSetpoints, BatteryConfig};
use modbus::ModbusTcpServer;
use ui::app::run_ui;

#[tokio::main]
async fn main() -> Result<()> {
    // Create logs directory if it doesn't exist
    std::fs::create_dir_all("logs")?;
    
    // Initialize logging with file output in logs directory
    let file_appender = tracing_appender::rolling::hourly("logs", "battsim");
    let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
    
    tracing_subscriber::fmt()
        .with_writer(non_blocking)
        .with_ansi(false) // No color codes in file
        .init();
    
    // Keep the guard alive for the duration of the program
    std::mem::forget(_guard);
    
    info!("battsim2 v0.1.0 starting...");
    
    // Initialize the battery simulator
    let mut simulator = BatterySimulator::new();
    
    // Get the state manager from the simulator
    let state_manager = simulator.state_manager().clone();
    let battery_rx = state_manager.subscribe();
    
    
    // Create a simple test channel first
    info!("Creating test channel...");
    let (test_tx, mut test_rx) = mpsc::channel::<i32>(10);
    
    // Test the channel immediately
    tokio::spawn(async move {
        info!("Test task: sending 42");
        test_tx.send(42).await.unwrap();
        info!("Test task: sent 42");
    });
    
    tokio::spawn(async move {
        info!("Test receiver: waiting for message");
        let msg = test_rx.recv().await;
        info!("Test receiver: got {:?}", msg);
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Create a ControlSetpoints test channel to isolate the issue
    info!("Creating ControlSetpoints test channel...");
    let (control_test_tx, mut control_test_rx) = mpsc::channel::<ControlSetpoints>(10);
    
    // Test ControlSetpoints channel immediately
    tokio::spawn(async move {
        info!("ControlSetpoints test task: creating setpoints");
        let setpoints = ControlSetpoints {
            command: battery::SystemCommand::Start,
            power_setpoint: -50.0,
            current_limit_charge: 100.0,
            current_limit_discharge: 100.0,
            voltage_limit_high: 58.4,
            voltage_limit_low: 44.0,
            soc_limit_high: 100.0,
            soc_limit_low: 0.0,
            ..Default::default()
        };
        info!("ControlSetpoints test task: sending setpoints");
        control_test_tx.send(setpoints).await.unwrap();
        info!("ControlSetpoints test task: sent setpoints");
    });
    
    tokio::spawn(async move {
        info!("ControlSetpoints test receiver: waiting for message");
        let msg = control_test_rx.recv().await;
        info!("ControlSetpoints test receiver: got {:?}", msg);
    });
    
    tokio::time::sleep(Duration::from_millis(100)).await;
    
    // Simplified approach: Pass state manager directly to UI instead of using channels
    info!("Setting up direct state manager access for UI...");
    
    // Clone state manager for UI access
    let ui_state_manager = state_manager.clone();
    
    // Modbus holding-register writes (setpoints/config) are delivered over these channels;
    // apply them to the shared battery state so they actually reach the simulation.
    let (setpoints_tx, mut setpoints_rx) = tokio::sync::mpsc::channel::<ControlSetpoints>(100);
    let (config_tx, mut config_rx) = tokio::sync::mpsc::channel::<BatteryConfig>(100);
    let modbus_server = ModbusTcpServer::new(battery_rx.clone(), setpoints_tx, config_tx);
    info!("Modbus server initialized");

    let setpoints_state_manager = state_manager.clone();
    let _setpoints_apply_task = tokio::spawn(async move {
        while let Some(setpoints) = setpoints_rx.recv().await {
            if let Err(e) = setpoints_state_manager.update_setpoints(setpoints) {
                error!("Failed to apply Modbus setpoints write: {}", e);
            }
        }
    });

    let config_state_manager = state_manager.clone();
    let _config_apply_task = tokio::spawn(async move {
        while let Some(config) = config_rx.recv().await {
            if let Err(e) = config_state_manager.update_config(config) {
                error!("Failed to apply Modbus config write: {}", e);
            }
        }
    });
    
    // Start all concurrent tasks
    let _simulation_task = tokio::spawn(async move {
        info!("Starting battery simulation...");
        let mut interval = time::interval(Duration::from_millis(100)); // 10 Hz
        loop {
            interval.tick().await;
            if let Err(e) = simulator.update(Duration::from_millis(100)).await {
                error!("Simulation error: {}", e);
            }
        }
    });
    
    let _modbus_task = tokio::spawn(async move {
        // Try different ports if 5020 is occupied
        for port in [5020, 5021, 5022, 5023] {
            let addr = format!("0.0.0.0:{}", port);
            info!("Trying to start Modbus server on {}...", addr);
            match modbus_server.start(&addr).await {
                Ok(_) => {
                    info!("Modbus server started successfully on {}", addr);
                    return;
                }
                Err(e) => {
                    error!("Failed to bind to {}: {}", addr, e);
                    if port == 5023 {
                        error!("All Modbus ports failed, continuing without Modbus server");
                        return;
                    }
                }
            }
        }
    });
    let ui_task = tokio::spawn(async move {
        info!("Starting terminal UI with direct state manager access...");
        if let Err(e) = run_ui(battery_rx, ui_state_manager).await {
            error!("UI error: {}", e);
        }
    });
    
    // Wait for UI task to complete (user quits) - let other tasks run independently
    info!("Main: Waiting for UI task to complete...");
    let _ = ui_task.await;
    info!("Main: UI task completed, shutting down...");
    
    info!("battsim2 shutting down...");
    Ok(())
}
