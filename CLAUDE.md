# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

BattSim2 is a lithium-ion battery energy storage system (BESS) simulator written in Rust. It simulates realistic battery behavior with Modbus TCP/RTU protocol access, featuring a real-time terminal-based UI for monitoring battery state and system parameters.

## Development Commands

### Build and Run
- `cargo run` - Start the battery simulator with terminal UI and Modbus server
- `cargo build` - Build the project
- `cargo check` - Quick compile check without producing executables
- `cargo test` - Run unit and integration tests
- `cargo fix` - Apply compiler-suggested fixes for warnings

### Development Tools
- `cargo clippy` - Run the Clippy linter for code quality
- `cargo fmt` - Format code according to Rust conventions

## Architecture Overview

### Core Components

The project follows a modular architecture with three main systems running concurrently:

1. **Battery Simulation Engine** (`src/battery/`)
   - `types.rs` - Data structures for battery state, electrical parameters, cell monitoring
   - `simulation.rs` - Core physics simulation with thermal modeling and efficiency calculations
   - `state.rs` - Thread-safe state management with broadcasting to subscribers

2. **Modbus Communication** (`src/modbus/`)
   - `server.rs` - Async Modbus TCP server handling client connections
   - `registers.rs` - Register map implementation following industrial standards
   - Supports Function Codes 03, 04, 06, 16 for read/write operations

3. **Terminal User Interface** (`src/ui/`)
   - `app.rs` - Main application logic and event handling
   - `widgets.rs` - Custom UI widgets including battery gauge visualization
   - `terminal.rs` - Terminal setup and management utilities
   - Built with `ratatui` and `crossterm` for cross-platform support

### Data Flow Architecture

The system uses a simplified direct-access approach for communication between components:
- Battery state updates broadcast via `tokio::sync::watch` channels to UI and Modbus components
- Control setpoints applied directly through thread-safe `BatteryStateManager` method calls
- UI communicates directly with the state manager rather than using intermediate channels
- All components run as concurrent Tokio tasks

### Key Data Structures

- `BatteryState` - Complete system state combining electrical, cell, status, and configuration data
- `ElectricalParams` - Real-time electrical measurements (voltage, current, power, SOC)
- `SystemStatus` - Operational state, faults, warnings, and connection status
- `ControlSetpoints` - Command and setpoint structure for system control

## Battery System Specifications

### Physical Parameters
- **Chemistry**: LiFePO4 (Lithium Iron Phosphate)
- **Capacity**: 500 kWh (configurable 100-1000 kWh)
- **Voltage**: 400V nominal (350-450V operating range)
- **Power**: 250 kW continuous, 375 kW peak (10s)
- **Configuration**: 128S1P (128 cells in series)

### Modbus Communication
- **Default Port**: 5020 (falls back to 5021-5023 if unavailable)
- **Protocol**: Modbus TCP primary, RTU support planned
- **Register Map**: Input registers (30001+) for measurements, Holding registers (40001+) for setpoints
- **Update Rate**: 10 Hz (100ms) for real-time data

### Terminal UI Features
- Real-time battery gauge with top-to-bottom drain visualization
- Color-coded status indicators (Green/Yellow/Red/Flashing Red)
- Comprehensive electrical parameter monitoring
- Cell-level voltage and temperature tracking
- System status and fault/warning displays
- Interactive command mode for control operations

## Development Notes

### Testing Strategy
- Unit tests available via `cargo test`
- Integration testing through Modbus client simulation
- UI testing through state injection and widget validation

### Error Handling
- Uses `anyhow::Result` for error propagation
- Structured logging with `tracing` crate
- Graceful degradation for connection failures
- Comprehensive fault detection and reporting

### Performance Considerations
- 10 Hz update rate for simulation and UI
- Efficient state broadcasting to minimize allocations
- Modbus server handles multiple concurrent clients
- Terminal UI uses differential rendering to prevent flicker

### Code Quality
- Run `cargo fix` to automatically apply suggested fixes
- Use `cargo clippy` for additional code quality suggestions
- Use `cargo fmt` for consistent code formatting

### Architecture Notes
- The project originally used complex async channel communication but was simplified to direct method calls on the thread-safe `BatteryStateManager`
- This architectural decision eliminated potential channel communication issues and simplified the codebase
- The `BatteryStateManager` uses `Arc<RwLock>` internally, making direct access safe across async tasks