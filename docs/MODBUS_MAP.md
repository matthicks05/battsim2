# battsim2 Modbus Register Map

battsim2 simulates a single, containerized grid-scale Battery Energy Storage System (BESS)
with an integrated site controller: one Modbus TCP interface exposes system/site status,
PCS (AC-side) measurements, BMS (DC-side) measurements, cell detail, a point-of-interconnection
(POI) meter, and auxiliary/environmental data. This is **not** a copy of any specific vendor's
register map — it's battsim2's own convention, designed to cover the breadth of tags a typical
BESS exposes, so it can be integrated against like any unfamiliar third-party system.

- **Protocol**: Modbus TCP
- **Default port**: 5020 (falls back to 5021, 5022, 5023 if unavailable)
- **Unit ID**: 1
- **Function codes supported**: 03 (Read Holding Registers), 04 (Read Input Registers),
  06 (Write Single Register), 16/0x10 (Write Multiple Registers)
- **Byte order**: Big-endian (standard Modbus). 32-bit values occupy two consecutive
  registers, **high word first**.
- **Update rate**: 10 Hz (100 ms) internally; registers reflect the latest simulated state.

## Data types and scaling

| Type in this doc | Registers | Notes |
|---|---|---|
| UINT16 | 1 | Unsigned, 0-65535 |
| INT16  | 1 | Signed, two's complement |
| UINT32 | 2 | High word at base address, low word at base address + 1 |
| INT32  | 2 | High word at base address, low word at base address + 1 |
| ENUM16 | 1 | UINT16, see enum tables below |
| BITMAP32 | 2 | UINT32, one bit per flag, see bitmap tables below |

"Scale" means: `register_value = engineering_value * scale` (and the inverse to decode).
E.g. a scale of 10 on a voltage of 400.0 V yields a register value of 4000.

## Input Registers (read-only measurements) — Function Code 04

### System / Site Status (30001-30099)

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 30001 | SYSTEM_STATE | ENUM16 | - | - | See [SystemState](#systemstate) |
| 30002 | OPERATING_MODE | ENUM16 | - | - | See [OperatingMode](#operatingmode) |
| 30003 | GRID_CONNECTED | UINT16 | - | bool | 0=disconnected, 1=connected |
| 30004 | CONTACTOR_STATUS | UINT16 | - | bitmap | Contactor state bits |
| 30005-30006 | FAULT_STATUS | BITMAP32 | - | - | See [FaultCode](#faultcode) |
| 30007-30008 | WARNING_STATUS | BITMAP32 | - | - | See [WarningCode](#warningcode) |
| 30009 | ACTIVE_FAULT_COUNT | UINT16 | - | count | Number of active faults |
| 30010 | ACTIVE_WARNING_COUNT | UINT16 | - | count | Number of active warnings |
| 30011 | MODBUS_CLIENT_COUNT | UINT16 | - | count | Connected Modbus clients |
| 30012-30013 | UPTIME_SECONDS | UINT32 | 1 | s | System uptime |
| 30014-30015 | OPERATING_HOURS | UINT32 | 1 | h | Total runtime |
| 30016 | FIRE_SUPPRESSION_STATUS | ENUM16 | - | - | 0=Normal, 1=Alarm, 2=Discharged |
| 30017 | SMOKE_DETECTED | UINT16 | - | bool | 0/1 |

### PCS / AC Measurements (30101-30199)

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 30101 | PCS_STATE | ENUM16 | - | - | See [PcsState](#pcsstate) |
| 30102 | GRID_FORMING | UINT16 | - | bool | 0=grid-following, 1=grid-forming |
| 30103 | AC_FREQUENCY | UINT16 | 100 | Hz | Grid frequency |
| 30104 | AC_POWER_FACTOR | INT16 | 1000 | - | -1.000 to 1.000 |
| 30105-30106 | AC_REAL_POWER | INT32 | 10 | kW | + = export/discharge |
| 30107-30108 | AC_REACTIVE_POWER | INT32 | 10 | kVAR | |
| 30109-30110 | AC_APPARENT_POWER | UINT32 | 10 | kVA | |
| 30111 | AC_VOLTAGE_L1N | UINT16 | 10 | V | Line-to-neutral, phase 1 |
| 30112 | AC_VOLTAGE_L2N | UINT16 | 10 | V | Line-to-neutral, phase 2 |
| 30113 | AC_VOLTAGE_L3N | UINT16 | 10 | V | Line-to-neutral, phase 3 |
| 30114 | AC_CURRENT_L1 | UINT16 | 10 | A | Phase 1 current |
| 30115 | AC_CURRENT_L2 | UINT16 | 10 | A | Phase 2 current |
| 30116 | AC_CURRENT_L3 | UINT16 | 10 | A | Phase 3 current |
| 30117 | PCS_TEMPERATURE | INT16 | 10 | °C | PCS heatsink/enclosure temp |
| 30118 | ISOLATION_RESISTANCE | UINT16 | 10 | kΩ | DC bus isolation resistance |

### BMS / DC Battery Measurements (30201-30299)

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 30201 | SOC | UINT16 | 10 | % | State of charge |
| 30202 | SOH | UINT16 | 10 | % | State of health |
| 30203 | BATTERY_VOLTAGE | UINT16 | 10 | V | DC bus voltage |
| 30204 | BATTERY_CURRENT | INT16 | 10 | A | + = discharge, - = charge |
| 30205-30206 | BATTERY_POWER | INT32 | 10 | kW | + = discharge, - = charge |
| 30207-30208 | MAX_CHARGE_POWER | UINT32 | 10 | kW | Dynamic limit (SOC/thermal derated) |
| 30209-30210 | MAX_DISCHARGE_POWER | UINT32 | 10 | kW | Dynamic limit (SOC/thermal derated) |
| 30211-30212 | AVAILABLE_CHARGE_ENERGY | UINT32 | 10 | kWh | Energy capacity remaining to full |
| 30213-30214 | AVAILABLE_DISCHARGE_ENERGY | UINT32 | 10 | kWh | Energy available for discharge |
| 30215-30216 | LIFETIME_CHARGE_ENERGY | UINT32 | 10 | kWh | Cumulative energy charged (never resets) |
| 30217-30218 | LIFETIME_DISCHARGE_ENERGY | UINT32 | 10 | kWh | Cumulative energy discharged (never resets) |
| 30219-30220 | CYCLE_COUNT | UINT32 | 1 | cycles | Equivalent full cycles |
| 30221 | INTERNAL_RESISTANCE | UINT16 | 10 | mΩ | Pack internal resistance |

### Cell Detail (30301-30399)

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 30301 | CELL_VOLTAGE_MIN | UINT16 | 1 | mV | Minimum cell voltage |
| 30302 | CELL_VOLTAGE_MAX | UINT16 | 1 | mV | Maximum cell voltage |
| 30303 | CELL_VOLTAGE_AVG | UINT16 | 1 | mV | Average cell voltage |
| 30304 | CELL_VOLTAGE_DELTA | UINT16 | 1 | mV | Max - min (imbalance indicator) |
| 30305 | CELL_TEMP_MIN | INT16 | 10 | °C | Minimum cell temperature |
| 30306 | CELL_TEMP_MAX | INT16 | 10 | °C | Maximum cell temperature |
| 30307 | CELL_TEMP_AVG | INT16 | 10 | °C | Average cell temperature |
| 30308 | BALANCING_CELLS | UINT16 | 1 | count | Cells currently active-balancing |

### Meter / POI (30401-30499)

Distinct from the PCS block above: this represents the site's point-of-interconnection
meter, i.e. PCS output net of auxiliary/house load.

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 30401 | METER_VOLTAGE_L1N | UINT16 | 10 | V | |
| 30402 | METER_VOLTAGE_L2N | UINT16 | 10 | V | |
| 30403 | METER_VOLTAGE_L3N | UINT16 | 10 | V | |
| 30404 | METER_CURRENT_L1 | UINT16 | 10 | A | |
| 30405 | METER_CURRENT_L2 | UINT16 | 10 | A | |
| 30406 | METER_CURRENT_L3 | UINT16 | 10 | A | |
| 30407 | METER_FREQUENCY | UINT16 | 100 | Hz | |
| 30408-30409 | METER_REAL_POWER | INT32 | 10 | kW | + = export to grid, - = import |
| 30410-30411 | METER_REACTIVE_POWER | INT32 | 10 | kVAR | |
| 30412-30413 | LIFETIME_IMPORT_ENERGY | UINT32 | 10 | kWh | Cumulative energy imported |
| 30414-30415 | LIFETIME_EXPORT_ENERGY | UINT32 | 10 | kWh | Cumulative energy exported |
| 30416 | AUX_LOAD_POWER | UINT16 | 10 | kW | Site auxiliary/house load |

### Auxiliary / Environment (30501-30599)

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 30501 | COOLING_STATUS | ENUM16 | - | - | 0=Offline, 1=Standby, 2=Active, 3=Fault |
| 30502 | AMBIENT_TEMPERATURE | INT16 | 10 | °C | |
| 30503 | ENCLOSURE_TEMPERATURE | INT16 | 10 | °C | |
| 30504 | AUX_POWER_CONSUMPTION | UINT16 | 10 | kW | |

## Holding Registers (read/write) — Function Codes 03/06/16

### Control Setpoints (40001-40099)

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 40001 | COMMAND | ENUM16 | - | - | See [SystemCommand](#systemcommand) |
| 40002-40003 | ACTIVE_POWER_SETPOINT | INT32 | 10 | kW | + = discharge, - = charge |
| 40004-40005 | REACTIVE_POWER_SETPOINT | INT32 | 10 | kVAR | |
| 40006 | POWER_FACTOR_SETPOINT | INT16 | 1000 | - | -1.000 to 1.000 |
| 40007 | OPERATING_MODE_SETPOINT | ENUM16 | - | - | See [OperatingMode](#operatingmode) |
| 40008 | GRID_CONNECT_COMMAND | UINT16 | - | bool | Request main breaker close (1) / open (0) |
| 40009 | CURRENT_LIMIT_CHARGE | UINT16 | 10 | A | Max charge current |
| 40010 | CURRENT_LIMIT_DISCHARGE | UINT16 | 10 | A | Max discharge current |
| 40011 | VOLTAGE_LIMIT_HIGH | UINT16 | 10 | V | Max charge voltage |
| 40012 | VOLTAGE_LIMIT_LOW | UINT16 | 10 | V | Min discharge voltage |
| 40013 | SOC_LIMIT_HIGH | UINT16 | 10 | % | Max SOC target |
| 40014 | SOC_LIMIT_LOW | UINT16 | 10 | % | Min SOC target |
| 40015 | WATCHDOG_TIMEOUT | UINT16 | 1 | s | EMS comms watchdog timeout |

### Configuration Parameters (40101-40199)

Nameplate/config values. Writable in battsim2 for test convenience, but a real system
would typically treat most of these as commissioning-time constants.

| Address | Name | Type | Scale | Unit | Description |
|---|---|---|---|---|---|
| 40101-40102 | RATED_CAPACITY | UINT32 | 10 | kWh | System energy capacity |
| 40103 | RATED_POWER | UINT16 | 1 | kW | System power rating |
| 40104 | RATED_REACTIVE_POWER | UINT16 | 1 | kVAR | System reactive power rating |
| 40105 | CELL_COUNT | UINT16 | 1 | count | Series cell count |
| 40106 | RATED_AC_VOLTAGE | UINT16 | 1 | V | Nominal AC line-to-line voltage |
| 40107 | NOMINAL_FREQUENCY | UINT16 | 100 | Hz | Nominal grid frequency (50 or 60) |
| 40108 | SIMULATION_SPEED | UINT16 | 10 | x | Simulation time multiplier (sim-only, not a real BESS tag) |

## Enumerations

### SystemState
`0`=Offline `1`=Standby `2`=Charging `3`=Discharging `4`=Fault `5`=Maintenance

### PcsState
`0`=Standby `1`=Precharge `2`=SoftStart `3`=Running `4`=Fault

### OperatingMode
`0`=Auto `1`=Manual `2`=Maintenance

### SystemCommand
`0`=NoCommand `1`=Start `2`=Stop `3`=EmergencyStop `4`=ResetFaults `5`=MaintenanceMode

### FaultCode (bit position in FAULT_STATUS)
`0`=OverVoltage `1`=UnderVoltage `2`=OverCurrent `3`=OverTemperature `4`=UnderTemperature
`5`=CommunicationFault `6`=ContactorFault `7`=InternalFault `8`=GridFault `9`=IsolationFault
`10`=FireAlarm `11`=SmokeDetected `12`=BmsCommunicationFault `13`=PcsCommunicationFault
`14`=CellImbalanceFault `15`=WatchdogTimeout

### WarningCode (bit position in WARNING_STATUS)
`0`=HighSocWarning `1`=LowSocWarning `2`=HighTemperatureWarning `3`=CellImbalanceWarning
`4`=ReducedPerformance `5`=MaintenanceDue `6`=GridFrequencyWarning `7`=GridVoltageWarning
`8`=AuxPowerWarning `9`=LowInsulationWarning

## Notes for integrators

- All AC-side and meter values are derived from the DC-side simulation each update cycle
  (PCS efficiency ~97%, symmetric 3-phase assumption) — there's no separate PCS/meter
  hardware simulation, just internally-consistent derived values.
- `METER_REAL_POWER` = `AC_REAL_POWER` minus `AUX_LOAD_POWER`; use the meter block for
  POI/revenue-grade polling and the PCS block for equipment-level polling.
- Writing to any register in 40001-40099 is treated as a setpoint change; writing to
  40101-40199 is treated as a configuration change (see `src/modbus/server.rs`).
- This map is intentionally independent of any real vendor's addressing (Delta, Dynapower,
  SYL, etc.) — it's meant to be treated as an unfamiliar system's datasheet for integration
  practice, not a drop-in replacement for a specific vendor's config module in
  `batt-mono-repo/edge/config-helpers`.
- Source of truth: `src/modbus/registers.rs` (`pub mod addresses`).
