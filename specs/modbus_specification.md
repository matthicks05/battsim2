# Modbus Register Map Specification v1.0

## Overview
This document defines the complete Modbus register map for the BattSim2 battery simulator. The implementation provides industrial-standard Modbus TCP access to all battery parameters, measurements, and control functions.

## Connection Parameters

### Network Configuration
- **Protocol**: Modbus TCP
- **Default Port**: 5020 (fallback: 5021, 5022, 5023)
- **Unit ID**: 1
- **Byte Order**: Big Endian (Motorola)
- **Word Order**: Big Endian (most significant word first)
- **Update Rate**: 10 Hz (100ms refresh cycle)
- **Max Clients**: Multiple concurrent connections supported

### Function Code Support
- **03 (Read Holding Registers)**: Access control setpoints and configuration
- **04 (Read Input Registers)**: Access real-time measurements and status
- **06 (Write Single Register)**: Modify individual control parameters
- **16 (Write Multiple Registers)**: Modify multiple control parameters

## Data Types and Scaling

### Data Type Definitions
- **UINT16**: Unsigned 16-bit integer (0 to 65535)
- **INT16**: Signed 16-bit integer (-32768 to +32767)
- **UINT32**: Unsigned 32-bit integer (0 to 4294967295) - Uses 2 consecutive registers
- **INT32**: Signed 32-bit integer (-2147483648 to +2147483647) - Uses 2 consecutive registers
- **ENUM**: Enumerated values using UINT16 encoding

### Scaling Convention
- **Scale 1.0**: No scaling (direct value)
- **Scale 10.0**: One decimal place (value × 10)
- **Scale 100.0**: Two decimal places (value × 100)
- **Scale 1000.0**: Voltage in millivolts (V × 1000)

### 32-bit Register Layout
32-bit values occupy two consecutive Modbus registers:
- **First Register**: High word (bits 31-16)
- **Second Register**: Low word (bits 15-0)
- **Example**: Value 0x12345678 → Register[N]=0x1234, Register[N+1]=0x5678

## Input Registers (Read-Only) - Function Code 04

### Real-time Measurements (30001-30100)

| Address | Parameter | Data Type | Units | Scale | Range | Description |
|---------|-----------|-----------|--------|-------|-------|-------------|
| 30001 | SOC | UINT16 | % | 10.0 | 0-1000 | State of Charge |
| 30002 | SOH | UINT16 | % | 10.0 | 500-1000 | State of Health |
| 30003 | Battery Voltage | UINT16 | V | 10.0 | 3500-4500 | DC Bus Voltage |
| 30004 | Battery Current | INT16 | A | 10.0 | -6250 to +6250 | Current (+ = discharge) |
| 30005 | Battery Power (High) | UINT16 | kW | 10.0 | - | Power MSW |
| 30006 | Battery Power (Low) | UINT16 | kW | 10.0 | - | Power LSW |
| 30007 | Cell Voltage Min | UINT16 | mV | 1.0 | 2500-3600 | Lowest Cell Voltage |
| 30008 | Cell Voltage Max | UINT16 | mV | 1.0 | 2500-3600 | Highest Cell Voltage |
| 30009 | Cell Voltage Avg | UINT16 | mV | 1.0 | 2500-3600 | Average Cell Voltage |
| 30010 | Cell Temp Min | INT16 | °C | 10.0 | -150 to +500 | Coldest Cell Temp |
| 30011 | Cell Temp Max | INT16 | °C | 10.0 | -150 to +500 | Hottest Cell Temp |
| 30012 | Cell Temp Avg | INT16 | °C | 10.0 | -150 to +500 | Average Cell Temp |
| 30013 | Internal Resistance | UINT16 | mΩ | 10.0 | 1-100 | Battery Internal R |
| 30014 | Available Charge Energy (High) | UINT16 | kWh | 10.0 | - | Energy to Full MSW |
| 30015 | Available Charge Energy (Low) | UINT16 | kWh | 10.0 | - | Energy to Full LSW |
| 30016 | Available Discharge Energy (High) | UINT16 | kWh | 10.0 | - | Energy to Empty MSW |
| 30017 | Available Discharge Energy (Low) | UINT16 | kWh | 10.0 | - | Energy to Empty LSW |
| 30018 | Cycle Count (High) | UINT16 | cycles | 1.0 | - | Total Cycles MSW |
| 30019 | Cycle Count (Low) | UINT16 | cycles | 1.0 | - | Total Cycles LSW |
| 30020 | Operating Hours (High) | UINT16 | hours | 1.0 | - | Total Runtime MSW |
| 30021 | Operating Hours (Low) | UINT16 | hours | 1.0 | - | Total Runtime LSW |

### System Status (30101-30150)

| Address | Parameter | Data Type | Units | Scale | Range | Description |
|---------|-----------|-----------|--------|-------|-------|-------------|
| 30101 | System State | UINT16 | enum | 1.0 | 0-5 | Operating State |
| 30102 | Fault Status (High) | UINT16 | bitmap | 1.0 | - | Active Faults MSW |
| 30103 | Fault Status (Low) | UINT16 | bitmap | 1.0 | - | Active Faults LSW |
| 30104 | Warning Status (High) | UINT16 | bitmap | 1.0 | - | Active Warnings MSW |
| 30105 | Warning Status (Low) | UINT16 | bitmap | 1.0 | - | Active Warnings LSW |
| 30106 | Contactor Status | UINT16 | bitmap | 1.0 | 0-65535 | Contactor States |
| 30107 | Cooling Status | UINT16 | enum | 1.0 | 0-3 | Cooling System State |

## Holding Registers (Read/Write) - Function Code 03/06/16

### Control Setpoints (40001-40100)

| Address | Parameter | Data Type | Units | Scale | Range | Description |
|---------|-----------|-----------|--------|-------|-------|-------------|
| 40001 | Command | UINT16 | enum | 1.0 | 0-5 | System Command |
| 40002 | Power Setpoint (High) | UINT16 | kW | 10.0 | - | Power Command MSW |
| 40003 | Power Setpoint (Low) | UINT16 | kW | 10.0 | - | Power Command LSW |
| 40004 | Current Limit Charge | UINT16 | A | 10.0 | 0-6250 | Max Charge Current |
| 40005 | Current Limit Discharge | UINT16 | A | 10.0 | 0-6250 | Max Discharge Current |
| 40006 | Voltage Limit High | UINT16 | V | 10.0 | 3500-4500 | Max Charge Voltage |
| 40007 | Voltage Limit Low | UINT16 | V | 10.0 | 3500-4500 | Min Discharge Voltage |
| 40008 | SOC Limit High | UINT16 | % | 10.0 | 0-1000 | Max SOC Target |
| 40009 | SOC Limit Low | UINT16 | % | 10.0 | 0-1000 | Min SOC Target |

### Configuration Parameters (40101-40200)

| Address | Parameter | Data Type | Units | Scale | Range | Description |
|---------|-----------|-----------|--------|-------|-------|-------------|
| 40101 | Rated Capacity (High) | UINT16 | kWh | 10.0 | - | System Capacity MSW |
| 40102 | Rated Capacity (Low) | UINT16 | kWh | 10.0 | - | System Capacity LSW |
| 40103 | Rated Power | UINT16 | kW | 1.0 | 100-1000 | System Power Rating |
| 40104 | Cell Count | UINT16 | count | 1.0 | 1-1000 | Number of Cells |
| 40105 | Simulation Speed | UINT16 | x | 10.0 | 1-1000 | Time Multiplier |

## Enumeration Definitions

### System State (30101, Read-Only)
| Value | State | Description |
|-------|-------|-------------|
| 0 | Offline | System not available |
| 1 | Standby | Ready but not active |
| 2 | Charging | Accepting power (negative current) |
| 3 | Discharging | Delivering power (positive current) |
| 4 | Fault | Fault condition active |
| 5 | Maintenance | In maintenance mode |

### System Commands (40001, Write)
| Value | Command | Description |
|-------|---------|-------------|
| 0 | No Command | No action required |
| 1 | Start | Activate system (Offline → Standby) |
| 2 | Stop | Deactivate system (Any → Offline) |
| 3 | Emergency Stop | Immediate stop with fault state |
| 4 | Reset Faults | Clear active faults and warnings |
| 5 | Maintenance Mode | Enter maintenance state |

### Fault Codes (30102-30103, Bitmap)
| Bit | Fault | Description |
|-----|-------|-------------|
| 0 | Over Voltage | System or cell voltage too high |
| 1 | Under Voltage | System or cell voltage too low |
| 2 | Over Current | Current exceeded safety limit |
| 3 | Over Temperature | Cell temperature too high |
| 4 | Under Temperature | Cell temperature too low |
| 5 | Communication Fault | Modbus communication error |
| 6 | Contactor Fault | Contactor operation failure |
| 7 | Internal Fault | Internal system error |

### Warning Codes (30104-30105, Bitmap)
| Bit | Warning | Description |
|-----|---------|-------------|
| 0 | High SOC Warning | SOC > 95% |
| 1 | Low SOC Warning | SOC < 10% |
| 2 | High Temperature Warning | Cell temperature > 45°C |
| 3 | Cell Imbalance Warning | Cell voltage spread > 50mV |
| 4 | Reduced Performance | Operating at reduced capacity |
| 5 | Maintenance Due | Scheduled maintenance required |

### Cooling Status (30107, Read-Only)
| Value | Status | Description |
|-------|--------|-------------|
| 0 | Offline | Cooling system not active |
| 1 | Standby | Ready but not cooling |
| 2 | Active | Actively cooling |
| 3 | Fault | Cooling system fault |

## Data Conversion Examples

### Reading SOC (30001)
```rust
// Register value: 852 (with scale 10.0)
// Actual SOC: 852 / 10.0 = 85.2%
let soc_percent = register_value as f64 / 10.0;
```

### Writing Power Setpoint (40002-40003)
```rust
// Set 125.5 kW discharge power
let power_kw = 125.5;
let scaled_power = (power_kw * 10.0) as i32;  // 1255
let high_word = ((scaled_power >> 16) & 0xFFFF) as u16;  // 0
let low_word = (scaled_power & 0xFFFF) as u16;           // 1255

// Write to registers
write_register(40002, high_word);  // Power Setpoint High
write_register(40003, low_word);   // Power Setpoint Low
```

### Reading Fault Status (30102-30103)
```rust
// Read 32-bit fault bitmap
let fault_high = read_register(30102);
let fault_low = read_register(30103);
let fault_bitmap = ((fault_high as u32) << 16) | (fault_low as u32);

// Check individual faults
let over_voltage = (fault_bitmap & (1 << 0)) != 0;
let under_voltage = (fault_bitmap & (1 << 1)) != 0;
let over_current = (fault_bitmap & (1 << 2)) != 0;
```

## Error Handling

### Modbus Exception Codes
- **01 (Illegal Function)**: Function code not supported
- **02 (Illegal Data Address)**: Register address out of range
- **03 (Illegal Data Value)**: Invalid data value for register
- **04 (Slave Device Failure)**: Internal processing error

### Register Validation
- **Read Operations**: Return 0 for unimplemented registers
- **Write Operations**: Validate range and type before applying
- **32-bit Values**: Both registers must be written as a pair
- **Reserved Addresses**: Return exception code 02

## Implementation Notes

### Update Timing
- **Input Registers**: Updated every 100ms from battery simulation
- **Holding Registers**: Applied immediately when written
- **State Changes**: Broadcast to all connected clients
- **Command Processing**: Commands processed on next simulation cycle

### Client Connection Management
- **Multiple Clients**: Supported with concurrent access
- **Connection Tracking**: Client count available in SystemStatus
- **Disconnection Handling**: Graceful cleanup of client resources
- **Timeout Handling**: Standard Modbus TCP timeout behavior

### Data Consistency
- **Atomic Updates**: Complete register map updated atomically
- **Scaling Consistency**: All related values use same scale factors
- **Endianness**: Consistent big-endian byte and word order
- **32-bit Alignment**: Multi-register values properly aligned

## Testing and Validation

### Register Access Patterns
```python
# Python example using pymodbus
from pymodbus.client.sync import ModbusTcpClient

client = ModbusTcpClient('localhost', port=5020)

# Read current SOC
soc_raw = client.read_input_registers(30001, 1).registers[0]
soc_percent = soc_raw / 10.0
print(f"SOC: {soc_percent}%")

# Read battery power (32-bit)
power_regs = client.read_input_registers(30005, 2).registers
power_raw = (power_regs[0] << 16) | power_regs[1]
power_kw = power_raw / 10.0
print(f"Power: {power_kw} kW")

# Set power command to 50kW discharge
power_scaled = int(50.0 * 10)  # 500
client.write_registers(40002, [0, power_scaled])  # High=0, Low=500

# Start the system
client.write_register(40001, 1)  # SystemCommand::Start
```

### Critical Register Monitoring
**High-Priority Polling (100ms):**
- SOC (30001)
- Battery Voltage (30003)
- Battery Current (30004)
- System State (30101)
- Fault Status (30102-30103)

**Medium-Priority Polling (1s):**
- Cell voltages (30007-30009)
- Cell temperatures (30010-30012)
- Warning Status (30104-30105)

**Low-Priority Polling (10s):**
- Configuration parameters (40101-40105)
- Cycle count (30018-30019)
- Operating hours (30020-30021)

## Integration Examples

### SCADA System Integration
```
Register Polling Schedule:
- Real-time data (30001-30021): 100ms
- Status data (30101-30107): 500ms  
- Configuration verification: 10s

Alarm Configuration:
- SOC < 10%: Low Battery Warning
- SOC > 95%: High Battery Warning
- System State = 4: Critical Fault Alarm
- Any Fault Bit Set: System Fault Alarm
```

### PLC Integration
```
Function Block: Battery_Monitor
Inputs:
  - Modbus_Client_Instance
  - Enable_Monitoring: BOOL
Outputs:
  - SOC: REAL (%)
  - Power: REAL (kW)
  - System_Healthy: BOOL
  - Fault_Active: BOOL
```

---
*Document Version: 1.0*  
*Last Updated: 2025-09-09*  
*Status: Implemented and Tested*  
*Related Documents: battery_specification.md, terminal_ui_specification.md*