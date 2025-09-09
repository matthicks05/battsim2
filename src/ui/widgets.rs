use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use std::time::{SystemTime, UNIX_EPOCH};
use crate::battery::BatteryState;

/// Render battery gauge widget with dynamic SOC-based fill and color coding
pub fn render_battery_gauge(f: &mut Frame, area: Rect, state: Option<&BatteryState>) {
    let (soc, soc_text) = if let Some(state) = state {
        (state.electrical.soc, format!("{:.1}%", state.electrical.soc))
    } else {
        (0.0, "---%".to_string())
    };

    // Calculate dynamic gauge height based on available area
    // Reserve space for: title border (1), SOC text (1), top border (1), bottom border (1), bottom border (1) = 5 lines
    let gauge_height = (area.height.saturating_sub(5)) as usize;
    let gauge_height = gauge_height.max(4); // Minimum 4 bars
    
    const BAR_WIDTH: &str = "█████████████"; // 13 characters wide
    const EMPTY_BAR: &str = "░░░░░░░░░░░░░"; // 13 characters wide
    
    // Calculate how many bars should be empty (top-to-bottom drain)
    let fill_percentage = (soc / 100.0).clamp(0.0, 1.0);
    let empty_bars = ((1.0 - fill_percentage) * gauge_height as f64).round() as usize;
    let filled_bars = gauge_height - empty_bars;
    
    // Determine color based on SOC level
    let bar_color = get_battery_color(soc);
    let bar_style = Style::default().fg(bar_color);
    
    // Build the battery gauge lines
    let mut gauge_lines = vec![
        // SOC percentage centered and prominent like specification
        Line::from(vec![
            Span::styled(soc_text, Style::default()
                .fg(bar_color)
                .add_modifier(Modifier::BOLD))
        ]),
        Line::from("┌─────────────┐"), // Top border
    ];
    
    // Add empty bars (discharged portion) from top
    for _ in 0..empty_bars {
        gauge_lines.push(Line::from(vec![
            Span::raw("│"),
            Span::styled(EMPTY_BAR, Style::default().fg(Color::DarkGray)),
            Span::raw("│"),
        ]));
    }
    
    // Add filled bars (remaining charge) at bottom
    for _ in 0..filled_bars {
        gauge_lines.push(Line::from(vec![
            Span::raw("│"),
            Span::styled(BAR_WIDTH, bar_style),
            Span::raw("│"),
        ]));
    }
    
    gauge_lines.push(Line::from("└─────────────┘")); // Bottom border

    let gauge = Paragraph::new(gauge_lines)
        .block(Block::default().title("Battery Gauge").borders(Borders::ALL))
        .alignment(ratatui::layout::Alignment::Center);
    
    f.render_widget(gauge, area);
}

/// Determine battery color based on SOC level with flashing for critical levels
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
        Color::Red // Red for low level (10-20%)
    } else if soc < 50.0 {
        Color::Yellow // Yellow for medium level (20-50%)
    } else {
        Color::Green // Green for good level (>50%)
    }
}

/// Render electrical parameters widget with comprehensive real-time data display
pub fn render_electrical_params(f: &mut Frame, area: Rect, state: Option<&BatteryState>) {
    let params_text = if let Some(state) = state {
        vec![
            // Add top padding
            Line::from(""),
            // Voltage - always green for normal operation
            Line::from(vec![
                Span::styled("Voltage:    ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:5.1} V", state.electrical.voltage),
                    Style::default().fg(get_voltage_color(state.electrical.voltage))
                ),
            ]),
            
            // Current with directional color coding
            Line::from(vec![
                Span::styled("Current:    ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:+5.1} A", state.electrical.current),
                    Style::default().fg(get_current_color(state.electrical.current))
                ),
            ]),
            
            // Power with directional color coding
            Line::from(vec![
                Span::styled("Power:      ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:+5.1} kW", state.electrical.power),
                    Style::default().fg(get_power_color(state.electrical.power))
                ),
            ]),
            
            // Add spacing between power and SOC section
            Line::from(""),
            
            // SOC with color coding based on level
            Line::from(vec![
                Span::styled("SOC:        ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:5.1}%", state.electrical.soc),
                    Style::default().fg(get_soc_color(state.electrical.soc))
                ),
            ]),
            
            // Available energy
            Line::from(vec![
                Span::styled("Available:  ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:5.0} kWh", state.electrical.available_energy),
                    Style::default().fg(Color::Cyan)
                ),
            ]),
            
            // Remaining capacity
            Line::from(vec![
                Span::styled("Remaining:  ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:5.0} kWh", state.electrical.remaining_capacity),
                    Style::default().fg(Color::Cyan)
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("No battery data available", Style::default().fg(Color::Red))
            ]),
            Line::from(""),
            Line::from("Initializing system..."),
        ]
    };

    let parameters = Paragraph::new(params_text)
        .block(Block::default().title("Electrical Parameters").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    f.render_widget(parameters, area);
}

/// Get voltage color based on operating range (350-450V)
fn get_voltage_color(voltage: f64) -> Color {
    if !(350.0..=450.0).contains(&voltage) {
        Color::Red // Out of range
    } else if !(370.0..=430.0).contains(&voltage) {
        Color::Yellow // Near limits
    } else {
        Color::Green // Normal range
    }
}

/// Get current color based on direction and magnitude
fn get_current_color(current: f64) -> Color {
    if current.abs() < 5.0 {
        Color::White // Standby current
    } else if current < 0.0 {
        Color::Yellow // Charging (negative current)
    } else {
        Color::Red // Discharging (positive current)
    }
}

/// Get power color based on direction and magnitude
fn get_power_color(power: f64) -> Color {
    if power.abs() < 1.0 {
        Color::White // Standby power
    } else if power < 0.0 {
        Color::Yellow // Charging (negative power)
    } else {
        Color::Red // Discharging (positive power)
    }
}

/// Get SOC color based on charge level
fn get_soc_color(soc: f64) -> Color {
    if soc >= 80.0 {
        Color::Green // High charge
    } else if soc >= 50.0 {
        Color::Cyan // Good charge
    } else if soc >= 20.0 {
        Color::Yellow // Medium charge
    } else if soc >= 10.0 {
        Color::Red // Low charge
    } else {
        Color::Magenta // Critical charge
    }
}

/// Render cell monitoring widget with min/max/avg cell voltages and temperatures
pub fn render_cell_monitoring(f: &mut Frame, area: Rect, state: Option<&BatteryState>) {
    let cell_text = if let Some(state) = state {
        vec![
            // Add top padding
            Line::from(""),
            Line::from(vec![
                Span::styled("Cell V Min: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.3} V", state.cells.voltage_min),
                    Style::default().fg(get_cell_voltage_color(state.cells.voltage_min))
                ),
            ]),
            Line::from(vec![
                Span::styled("Cell V Max: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.3} V", state.cells.voltage_max),
                    Style::default().fg(get_cell_voltage_color(state.cells.voltage_max))
                ),
            ]),
            Line::from(vec![
                Span::styled("Cell V Avg: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.3} V", state.cells.voltage_avg),
                    Style::default().fg(get_cell_voltage_color(state.cells.voltage_avg))
                ),
            ]),
            // Add spacing between voltage and temperature sections
            Line::from(""),
            Line::from(vec![
                Span::styled("Temp Min:   ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.1}°C", state.cells.temp_min),
                    Style::default().fg(get_temperature_color(state.cells.temp_min))
                ),
            ]),
            Line::from(vec![
                Span::styled("Temp Max:   ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.1}°C", state.cells.temp_max),
                    Style::default().fg(get_temperature_color(state.cells.temp_max))
                ),
            ]),
            Line::from(vec![
                Span::styled("Temp Avg:   ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.1}°C", state.cells.temp_avg),
                    Style::default().fg(get_temperature_color(state.cells.temp_avg))
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("No cell data available", Style::default().fg(Color::Red))
            ]),
            Line::from(""),
            Line::from("Initializing sensors..."),
        ]
    };

    let cell_widget = Paragraph::new(cell_text)
        .block(Block::default().title("Cell Monitoring").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    f.render_widget(cell_widget, area);
}

/// Render system status widget with connection status, faults, and warnings
pub fn render_system_status(f: &mut Frame, area: Rect, state: Option<&BatteryState>) {
    let status_text = if let Some(state) = state {
        let mut lines = vec![
            // Add top padding  
            Line::from(""),
        ];

        // System state with color coding
        lines.push(Line::from(vec![
            Span::styled("State: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("{:?}", state.status.state),
                get_system_state_color(state.status.state)
            ),
        ]));

        // Modbus clients count
        lines.push(Line::from(vec![
            Span::styled("Clients: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("{}", state.status.modbus_clients)),
        ]));

        // Add spacing before fault status
        lines.push(Line::from(""));

        // Fault status
        if state.status.faults.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("No Faults", Style::default().fg(Color::Green))
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled("Faults:", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
            ]));
            for fault in &state.status.faults {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(format!("{:?}", fault), Style::default().fg(Color::Red)),
                ]));
            }
        }

        // Warning status
        if !state.status.warnings.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(vec![
                Span::styled("Warnings:", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            ]));
            for warning in &state.status.warnings {
                lines.push(Line::from(vec![
                    Span::raw("  • "),
                    Span::styled(format!("{:?}", warning), Style::default().fg(Color::Yellow)),
                ]));
            }
        }

        lines
    } else {
        vec![
            Line::from(vec![
                Span::styled("○ ", Style::default().fg(Color::Gray)),
                Span::raw("Offline"),
            ]),
            Line::from(""),
            Line::from("System initializing..."),
        ]
    };

    let status_widget = Paragraph::new(status_text)
        .block(Block::default().title("System Status").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    f.render_widget(status_widget, area);
}

/// Render system info widget with SOH, resistance, cycles, and uptime
pub fn render_system_info(f: &mut Frame, area: Rect, state: Option<&BatteryState>) {
    let info_text = if let Some(state) = state {
        vec![
            // Add top padding
            Line::from(""),
            Line::from(vec![
                Span::styled("SOH:        ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.1}%", state.info.soh),
                    Style::default().fg(get_soh_color(state.info.soh))
                ),
            ]),
            Line::from(vec![
                Span::styled("Resistance: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{:.1} mΩ", state.info.internal_resistance * 1000.0),
                    Style::default().fg(Color::Cyan)
                ),
            ]),
            Line::from(vec![
                Span::styled("Cycles:     ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("{}", state.info.cycle_count),
                    Style::default().fg(Color::White)
                ),
            ]),
            // Add spacing before uptime
            Line::from(""),
            Line::from(vec![
                Span::styled("Uptime:     ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled(
                    format_duration(state.status.uptime),
                    Style::default().fg(Color::Green)
                ),
            ]),
        ]
    } else {
        vec![
            Line::from(vec![
                Span::styled("No system data available", Style::default().fg(Color::Red))
            ]),
            Line::from(""),
            Line::from("Initializing..."),
        ]
    };

    let info_widget = Paragraph::new(info_text)
        .block(Block::default().title("System Info").borders(Borders::ALL))
        .wrap(ratatui::widgets::Wrap { trim: true });
    
    f.render_widget(info_widget, area);
}

/// Get cell voltage color based on safe operating range (2.7-3.6V per spec)
fn get_cell_voltage_color(voltage: f64) -> Color {
    if !(2.7..=3.6).contains(&voltage) {
        Color::Red // Dangerous range
    } else if !(3.0..=3.5).contains(&voltage) {
        Color::Yellow // Caution range
    } else {
        Color::Green // Normal range
    }
}

/// Get temperature color based on operating range (-10°C to +50°C per spec)
fn get_temperature_color(temp: f64) -> Color {
    if !(-10.0..=50.0).contains(&temp) {
        Color::Red // Out of spec range
    } else if !(0.0..=45.0).contains(&temp) {
        Color::Yellow // Caution range (thermal derating above 45°C)
    } else {
        Color::Green // Normal range
    }
}

/// Get system state color
fn get_system_state_color(state: crate::battery::SystemState) -> Style {
    use crate::battery::SystemState;
    match state {
        SystemState::Standby => Style::default().fg(Color::Green),
        SystemState::Charging => Style::default().fg(Color::Yellow),
        SystemState::Discharging => Style::default().fg(Color::Cyan),
        SystemState::Fault => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        SystemState::Maintenance => Style::default().fg(Color::Blue),
        SystemState::Offline => Style::default().fg(Color::Gray),
    }
}

/// Get SOH color based on battery health
fn get_soh_color(soh: f64) -> Color {
    if soh >= 95.0 {
        Color::Green // Excellent health
    } else if soh >= 85.0 {
        Color::Cyan // Good health
    } else if soh >= 70.0 {
        Color::Yellow // Fair health
    } else if soh >= 50.0 {
        Color::Red // Poor health
    } else {
        Color::Magenta // Critical health
    }
}

/// Format duration in a compact human-readable format
fn format_duration(duration: std::time::Duration) -> String {
    let total_seconds = duration.as_secs();
    let days = total_seconds / 86400;
    let hours = (total_seconds % 86400) / 3600;
    let minutes = (total_seconds % 3600) / 60;

    if days > 0 {
        format!("{}d {}h {}m", days, hours, minutes)
    } else if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else {
        format!("{}m", minutes)
    }
}