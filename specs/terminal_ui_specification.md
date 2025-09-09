# Battery Simulator Terminal UI Specification v2.0

## Overview
This document defines the terminal-based user interface specifications for the battery simulator. The UI provides real-time visual monitoring of battery state with an ASCII battery gauge and comprehensive electrical parameter display.

## UI Framework

### Technology Stack
- **Library**: `ratatui` v0.24+ (modern Rust TUI framework)
- **Backend**: `crossterm` v0.27+ for cross-platform terminal handling
- **Async Runtime**: `tokio` for concurrent UI updates and data polling
- **Update Rate**: 10 Hz (100ms refresh cycle)
- **Fixed Width**: 80 columns (centered layout)

### Dependencies
```toml
[dependencies]
ratatui = "0.24"
crossterm = "0.27"
tokio = { version = "1.0", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
```

## Main Dashboard Layout

The UI uses a compact, fixed-width layout that centers the content on the screen:

```
┌─ battsim2 - Battery Energy Storage System Simulator ─────────────────┐
├─────────────┬──────────────────┬─────────────────────────────────────┤
│ Electrical  │  Battery Gauge   │ System Info    │ System Status     │
│ Parameters  │                  │                │                   │
│             │   ┌───────────┐  │ SOH:    98.5%  │ State: Standby    │
│ Voltage:    │   │░░░░░░░░░░░│  │ Resistance:    │ Clients: 2        │
│   412.5 V   │   │░░░░░░░░░░░│  │   64.0 mΩ      │                   │
│             │   │███████████│  │ Cycles: 0      │ No Faults         │
│ Current:    │   │███████████│  │                │                   │
│  -45.2 A    │   │███████████│  │ Uptime: 5m     │                   │
│             │   │███████████│  │                │                   │
│ Power:      │   │███████████│  │                │                   │
│  -18.6 kW   │   │███████████│  │                │                   │
│             │   │    85%     │  │                │                   │
│ SOC: 85.0%  │   └───────────┘  │                │                   │
│             │                  │                │                   │
│ Available:  │ Cell Monitoring  │                │                   │
│   425 kWh   │                  │                │                   │
│             │ Cell V Min:      │                │                   │
│ Remaining:  │    3.20 V        │                │                   │
│    75 kWh   │ Cell V Max:      │                │                   │
│             │    3.24 V        │                │                   │
│             │ Cell V Avg:      │                │                   │
│             │    3.22 V        │                │                   │
│             │                  │                │                   │
│             │ Temp Min: 25.0°C │                │                   │
│             │ Temp Max: 30.0°C │                │                   │
│             │ Temp Avg: 27.5°C │                │                   │
├─────────────┴──────────────────┴─────────────────────────────────────┤
│ Recent messages appear here...                                       │
│ ✓ System initialized                                                 │
│ ✓ Modbus server started on port 5020                               │
├─────────────────────────────────────────────────────────────────────┤
│ Commands: [Q]uit [R]eset [S]tart/Stop [C]ommand [H]elp            │
└─────────────────────────────────────────────────────────────────────┘
```

## Widget Specifications

### Battery Gauge Widget

#### Design Implementation
The battery gauge is implemented with dynamic sizing and realistic visual behavior:

- **Width**: 13 characters (fixed)
- **Height**: Dynamic based on available area (minimum 4 bars)
- **Fill Direction**: **Top-to-bottom drain** (empties from top like a real battery)
- **Border**: Single-line box drawing characters (┌┐└┘─│)

#### Fill Patterns and Behavior
```rust
const BAR_WIDTH: &str = "█████████████";  // 13 characters wide - filled
const EMPTY_BAR: &str = "░░░░░░░░░░░░░";  // 13 characters wide - empty

// Top-to-bottom drain calculation
let fill_percentage = (soc / 100.0).clamp(0.0, 1.0);
let empty_bars = ((1.0 - fill_percentage) * gauge_height as f64).round() as usize;
let filled_bars = gauge_height - empty_bars;
```

#### Color Coding System
The gauge uses dynamic color coding with special critical-level flashing:

- **Green** (SOC > 50%): Healthy charge level, normal operation
- **Yellow** (20% ≤ SOC ≤ 50%): Moderate charge level
- **Red** (10% ≤ SOC < 20%): Low battery warning
- **Flashing Red** (SOC < 10%): Critical low battery alert (500ms flash cycle)

#### Flashing Implementation
```rust
fn get_battery_color(soc: f64) -> Color {
    if soc < 10.0 {
        // Flashing red for critical level (<10%)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        if (now / 500) % 2 == 0 { // Flash every 500ms
            Color::Red
        } else {
            Color::DarkGray
        }
    } else if soc < 20.0 {
        Color::Red
    } else if soc < 50.0 {
        Color::Yellow
    } else {
        Color::Green
    }
}
```

#### Test Results and Validation
The battery gauge has been thoroughly tested and validated with the following results:

✅ **Top-to-Bottom Drain Pattern**
- Battery empties from top to bottom as specified
- Discharged portion (empty bars) appears at the top
- Remaining charge (filled bars) is shown at the bottom

✅ **Color Coding System**
- Green: SOC > 50% (good level)
- Yellow: SOC 20-50% (medium level)
- Red: SOC 10-20% (low level)  
- Flashing Red: SOC < 10% (critical level, flashes every 500ms)

✅ **Dynamic Visual Representation**
- Dynamic height based on available area
- Filled bars use solid block characters (█)
- Empty bars use light shade characters (░)
- Professional border appearance

### Electrical Parameters Widget

#### Data Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElectricalParams {
    pub voltage: f64,           // Battery DC voltage (V)
    pub current: f64,           // Current in Amperes (+ = discharge, - = charge)
    pub power: f64,             // Instantaneous power (kW)
    pub soc: f64,              // State of charge (%)
    pub available_energy: f64,  // Energy available for discharge (kWh)
    pub remaining_capacity: f64, // Energy capacity remaining to full (kWh)
}
```

#### Display Format
- **Voltage**: `XXX.X V` (1 decimal place)
- **Current**: `±XXX.X A` (1 decimal place, sign indicates direction)
- **Power**: `±XXX.X kW` (1 decimal place, sign indicates charge/discharge)
- **SOC**: `XX.X%` (1 decimal place)
- **Energy values**: `XXX kWh` (integer, space-efficient)

#### Color Coding
- **Voltage**: Dynamic color based on operating range (Green: 370-430V, Yellow: near limits, Red: out of range)
- **Current**: White (standby <5A), Yellow (charging/negative), Red (discharging/positive)
- **Power**: White (standby <1kW), Yellow (charging/negative), Red (discharging/positive)
- **SOC**: Dynamic color (Green ≥80%, Cyan ≥50%, Yellow ≥20%, Red ≥10%, Magenta <10%)

### Cell Monitoring Widget

#### Data Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CellMonitoring {
    pub voltage_min: f64,    // Minimum cell voltage (V)
    pub voltage_max: f64,    // Maximum cell voltage (V)
    pub voltage_avg: f64,    // Average cell voltage (V)
    pub temp_min: f64,       // Minimum cell temperature (°C)
    pub temp_max: f64,       // Maximum cell temperature (°C)
    pub temp_avg: f64,       // Average cell temperature (°C)
}
```

#### Display Format
- **Cell voltages**: `X.XXX V` (3 decimal places for precision)
- **Temperatures**: `XX.X°C` (1 decimal place)
- **Layout**: Compact two-column arrangement with clear min/max/avg labels

#### Color Coding
- **Cell Voltages**: Green (3.0-3.5V), Yellow (2.7-3.0V or 3.5-3.6V), Red (<2.7V or >3.6V)
- **Temperatures**: Green (0-45°C), Yellow (-10-0°C or 45-50°C), Red (<-10°C or >50°C)

### System Status Widget

#### Data Structure
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemStatus {
    pub state: SystemState,         // Current operating state
    pub online: bool,              // Modbus connection status (derived from state)
    pub faults: Vec<FaultCode>,    // Active system faults
    pub warnings: Vec<WarningCode>, // Active system warnings
    pub modbus_clients: u32,       // Number of connected clients
    pub uptime: Duration,          // System uptime
}

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
```

#### Status Display
- **State Display**: Color-coded system state (Green: Standby, Yellow: Charging, Cyan: Discharging, Red: Fault, Blue: Maintenance, Gray: Offline)
- **Client Count**: Number of connected Modbus clients
- **Fault Display**: List of active fault descriptions (Red text)
- **Warning Display**: List of active warning descriptions (Yellow text)
- **No Status Indicators**: Removed redundant online/offline indicators (state is already color-coded)

### System Info Widget

#### Data Elements
- **SOH (State of Health)**: `XX.X%` - Battery aging indicator with color coding
- **Internal Resistance**: `XX.X mΩ` - Battery impedance (converted from internal Ω storage)
- **Cycle Count**: `XXXX` - Total equivalent full cycles
- **Uptime**: `XXXm` or `XXh XXm` or `XXd XXh XXm` - System runtime (compact format)

#### Color Coding
- **SOH**: Green (≥95%), Cyan (≥85%), Yellow (≥70%), Red (≥50%), Magenta (<50%)
- **Other values**: Cyan for informational data

## User Interface Controls

### Keyboard Commands
| Key | Function | Description |
|-----|----------|-------------|
| **Q** | Quit | Exit the application gracefully |
| **R** | Reset | Clear active faults and warnings |
| **S** | Start/Stop | Toggle battery system operation |
| **C** | Command Mode | Enter interactive command interface |
| **H** | Help | Display help screen overlay |
| **Esc** | Cancel | Exit current mode/dialog |

### Command Mode Interface

#### Available Commands

**System Control:**
- `start` - Start the battery system
- `stop` - Stop the battery system  
- `emergency_stop` - Immediate system shutdown
- `maintenance_mode` - Enter maintenance state

**Power Control:**
- `set power <kW>` - Set power setpoint and start system

**Initialization Commands** (system must be stopped):
- `init soc <0-100>` - Set initial SOC percentage
- `init capacity <100-1000>` - Set battery capacity in kWh
- `init voltage <350-450>` - Set nominal voltage in V
- `init temp <-10-50>` - Set initial temperature in °C
- `init soh <50-100>` - Set State of Health percentage
- `init resistance <0.1-10>` - Set internal resistance in mΩ
- `init cycles <0-10000>` - Set cycle count
- `init reset` - Reset all parameters to defaults

#### Command Interface Features
- **Command History**: Navigate with ↑/↓ arrow keys
- **Input Validation**: Range checking with clear error messages
- **Context Awareness**: Init commands only work when system is stopped
- **Auto-completion**: Tab completion for command names
- **Help System**: Built-in help with command descriptions

### Help Screen Overlay

The help system provides comprehensive command documentation in a popup overlay:

```
┌─ Help - Battery Simulator Controls ─────────────────────┐
│                                                         │
│ Keyboard Commands:                                      │
│   Q - Quit application                                  │
│   R - Reset faults and warnings                        │  
│   S - Toggle system start/stop                         │
│   C - Enter command mode                               │
│   H - Show this help                                   │
│                                                         │
│ Command Mode (press C or :):                           │
│ System Control:                                        │
│   start - Start the battery system                     │
│   stop - Stop the battery system                       │
│   emergency_stop - Immediate system shutdown           │
│   maintenance_mode - Enter maintenance state           │
│                                                         │
│ Power Control:                                         │
│   set power <kW> - Set power setpoint                 │
│                                                         │
│ Init Commands (system must be stopped):               │
│   init soc <%> - Set initial SOC (0-100%)            │
│   init capacity <kWh> - Set battery capacity          │
│   init voltage <V> - Set nominal voltage              │
│   init temp <°C> - Set cell temperature              │
│   init soh <%> - Set State of Health                 │
│   init resistance <mΩ> - Set internal resistance     │  
│   init cycles <count> - Set cycle count               │
│   init reset - Reset all parameters to defaults       │
│                                                         │
│ Press H or Esc to close this help                     │
└─────────────────────────────────────────────────────────┘
```

## Layout Architecture

### Fixed-Width Centered Design
The UI uses a fixed 80-column width, centered on the screen:

```rust
// Create a fixed-width centered area (80 columns wide)
let fixed_width_area = centered_rect_fixed(80, f.size());

let chunks = Layout::default()
    .direction(Direction::Vertical)
    .constraints([
        Constraint::Length(1), // Compact header
        Constraint::Min(0),    // Main content area  
        Constraint::Length(4), // Compact messages panel
        Constraint::Length(1), // Compact status bar
    ])
    .split(fixed_width_area);
```

### Widget Layout
- **Left Column** (24 chars): Electrical Parameters (60%) + Cell Monitoring (40%)
- **Center Column** (32 chars): Battery Gauge (full height)
- **Right Column** (24 chars): System Info (40%) + System Status (60%)

### Message System
- **Compact Message Panel**: Shows last 3 messages
- **Message Types**: Success (✓), Error (✗), Info (no prefix)
- **Message History**: Maintains last 100 messages, auto-trims to 50 when full

## Data Flow and Updates

### Update Rate
- **UI Refresh**: 10 Hz (100ms) for all widgets
- **State Broadcasting**: `tokio::sync::watch` channels for efficient updates
- **Direct Access**: UI communicates directly with `BatteryStateManager`

### Error Handling
- **Connection Loss**: Display "---" for unavailable data
- **Invalid Values**: Show error messages in command interface
- **Range Validation**: All init commands validate input ranges

## Performance and Accessibility

### Rendering Optimization
- **Fixed Layout**: Eliminates complex responsive calculations
- **Efficient Updates**: Direct state access without intermediate channels
- **Color Consistency**: Centralized color determination functions

### Terminal Compatibility
- **Unicode Support**: Uses box-drawing characters and block symbols
- **Fallback Handling**: Graceful degradation for unsupported terminals
- **Cross-Platform**: Works on Windows, macOS, and Linux terminals

## Implementation Status

### ✅ Completed Features
- Dynamic battery gauge with top-to-bottom drain
- Comprehensive electrical parameter display  
- Cell monitoring with min/max/avg values
- System status with color-coded states
- Full command system with validation
- Help system with command documentation
- Fixed-width centered layout
- Message system with history
- Real-time updates at 10 Hz

### 🔄 Differences from Original Spec
- **Simplified Online Status**: Removed redundant online/offline indicators
- **Enhanced Init Commands**: Added comprehensive initialization system
- **Improved Layout**: More compact and efficient use of space
- **Better Color Coding**: More sophisticated status-based coloring
- **Dynamic Gauge Height**: Adapts to available terminal space

---
*Document Version: 2.0*  
*Last Updated: 2025-09-09*  
*Status: Implemented and Tested*
*Related Documents: battery_specification.md*