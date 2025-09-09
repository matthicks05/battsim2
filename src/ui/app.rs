use std::io;
use std::time::Duration;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame, Terminal,
};
use tokio::sync::watch;
use crate::battery::{BatteryState, ControlSetpoints, BatteryStateManager};
use super::widgets::{render_battery_gauge, render_electrical_params, render_cell_monitoring, render_system_status, render_system_info};

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum InputMode {
    Normal,
    Command,
    Help,
}

pub struct App {
    pub input_mode: InputMode,
    pub command_input: String,
    pub command_history: Vec<String>,
    pub history_index: usize,
    pub messages: Vec<String>,
    pub should_quit: bool,
    pub tick_rate: Duration,
    pub show_help: bool,
    pub pending_setpoints: Option<ControlSetpoints>,
    pub state_manager: Option<BatteryStateManager>,
}

impl Default for App {
    fn default() -> App {
        App {
            input_mode: InputMode::Normal,
            command_input: String::new(),
            command_history: Vec::new(),
            history_index: 0,
            messages: vec!["battsim2 initialized".to_string()],
            should_quit: false,
            tick_rate: Duration::from_millis(100), // 10 Hz updates
            show_help: false,
            pending_setpoints: None,
            state_manager: None,
        }
    }
}

impl App {
    pub fn new() -> App {
        Default::default()
    }
    
    pub fn set_state_manager(&mut self, state_manager: BatteryStateManager) {
        self.state_manager = Some(state_manager);
    }
    
    
    pub fn quit(&mut self) {
        self.should_quit = true;
    }
    
    pub fn toggle_input_mode(&mut self) {
        match self.input_mode {
            InputMode::Normal => {
                self.input_mode = InputMode::Command;
                self.command_input.clear();
                self.history_index = self.command_history.len();
            }
            InputMode::Command => {
                self.input_mode = InputMode::Normal;
            }
            InputMode::Help => {
                self.input_mode = InputMode::Normal;
                self.show_help = false;
            }
        }
    }
    
    pub fn show_help(&mut self) {
        self.input_mode = InputMode::Help;
        self.show_help = true;
    }
    
    pub fn reset_faults(&mut self) {
        self.messages.push("Reset faults command issued".to_string());
        // This would integrate with the state manager to clear faults
    }
    
    pub fn toggle_system_operation(&mut self) {
        if let Some(ref state_manager) = self.state_manager {
            // Get current state to determine what action to take
            match state_manager.get_state() {
                Ok(current_state) => {
                    let mut setpoints = ControlSetpoints::default();
                    let (command, message) = match current_state.status.state {
                        crate::battery::SystemState::Offline => {
                            (crate::battery::SystemCommand::Start, "✓ Starting system...")
                        }
                        _ => {
                            (crate::battery::SystemCommand::Stop, "✓ Stopping system...")
                        }
                    };
                    setpoints.command = command;
                    match state_manager.update_setpoints(setpoints) {
                        Ok(_) => self.messages.push(message.to_string()),
                        Err(e) => self.messages.push(format!("✗ Failed to toggle system: {}", e)),
                    }
                }
                Err(e) => self.messages.push(format!("✗ Failed to get system state: {}", e)),
            }
        } else {
            self.messages.push("✗ State manager not available".to_string());
        }
    }
    
    pub fn take_pending_setpoints(&mut self) -> Option<ControlSetpoints> {
        self.pending_setpoints.take()
    }
    
    pub fn enter_char(&mut self, c: char) {
        if self.input_mode == InputMode::Command {
            self.command_input.push(c);
        }
    }
    
    pub fn delete_char(&mut self) {
        if self.input_mode == InputMode::Command {
            self.command_input.pop();
        }
    }
    
    pub fn submit_command(&mut self) {
        if self.input_mode == InputMode::Command {
            if !self.command_input.is_empty() {
                        self.command_history.push(self.command_input.clone());
                self.execute_command(self.command_input.clone());
                self.command_input.clear();
            }
            self.input_mode = InputMode::Normal;
        }
    }
    
    pub fn navigate_history(&mut self, direction: i32) {
        if self.input_mode == InputMode::Command && !self.command_history.is_empty() {
            if direction > 0 && self.history_index > 0 {
                // Navigate up (older commands)
                self.history_index -= 1;
                self.command_input = self.command_history[self.history_index].clone();
            } else if direction < 0 && self.history_index < self.command_history.len() - 1 {
                // Navigate down (newer commands)
                self.history_index += 1;
                self.command_input = self.command_history[self.history_index].clone();
            }
        }
    }
    
    // Validation functions for init commands
    fn validate_soc(value: f64) -> bool {
        (0.0..=100.0).contains(&value)
    }
    
    fn validate_capacity(value: f64) -> bool {
        (100.0..=1000.0).contains(&value)
    }
    
    fn validate_voltage(value: f64) -> bool {
        (350.0..=450.0).contains(&value)
    }
    
    fn validate_temperature(value: f64) -> bool {
        (-10.0..=50.0).contains(&value)
    }
    
    fn validate_soh(value: f64) -> bool {
        (50.0..=100.0).contains(&value)
    }
    
    fn validate_resistance(value: f64) -> bool {
        (0.1..=10.0).contains(&value)
    }
    
    fn validate_cycles(value: u32) -> bool {
        value <= 10000
    }

    fn execute_command(&mut self, command: String) {
        let cmd_parts: Vec<&str> = command.split_whitespace().collect();
        if cmd_parts.is_empty() {
            return;
        }

        match cmd_parts[0].to_lowercase().as_str() {
            "set" => {
                if cmd_parts.len() >= 3 {
                    match cmd_parts[1] {
                        "power" => {
                            if let Ok(power) = cmd_parts[2].parse::<f64>() {
                                // Create new setpoints with the power command
                                let setpoints = ControlSetpoints {
                                    power_setpoint: power,
                                    command: crate::battery::SystemCommand::Start,
                                    ..Default::default()
                                };
                                self.pending_setpoints = Some(setpoints);
                                self.messages.push(format!("Set power setpoint to {} kW", power));
                            } else {
                                self.messages.push("Invalid power value".to_string());
                            }
                        }
                        "soc_min" => {
                            if let Ok(soc) = cmd_parts[2].parse::<f64>() {
                                self.messages.push(format!("Set minimum SOC to {}%", soc));
                            } else {
                                self.messages.push("Invalid SOC value".to_string());
                            }
                        }
                        "soc_max" => {
                            if let Ok(soc) = cmd_parts[2].parse::<f64>() {
                                self.messages.push(format!("Set maximum SOC to {}%", soc));
                            } else {
                                self.messages.push("Invalid SOC value".to_string());
                            }
                        }
                        _ => {
                            self.messages.push(format!("Unknown parameter: {}", cmd_parts[1]));
                        }
                    }
                } else {
                    self.messages.push("Usage: set <parameter> <value>".to_string());
                }
            }
            "emergency_stop" => {
                self.messages.push("EMERGENCY STOP activated!".to_string());
            }
            "maintenance_mode" => {
                self.messages.push("Entering maintenance mode".to_string());
            }
            "start" => {
                if let Some(ref state_manager) = self.state_manager {
                    let setpoints = ControlSetpoints {
                        command: crate::battery::SystemCommand::Start,
                        ..Default::default()
                    };
                    match state_manager.update_setpoints(setpoints) {
                        Ok(_) => self.messages.push("✓ System start command issued".to_string()),
                        Err(e) => self.messages.push(format!("✗ Failed to start system: {}", e)),
                    }
                } else {
                    self.messages.push("✗ State manager not available".to_string());
                }
            }
            "stop" => {
                if let Some(ref state_manager) = self.state_manager {
                    let setpoints = ControlSetpoints {
                        command: crate::battery::SystemCommand::Stop,
                        ..Default::default()
                    };
                    match state_manager.update_setpoints(setpoints) {
                        Ok(_) => self.messages.push("✓ System stop command issued".to_string()),
                        Err(e) => self.messages.push(format!("✗ Failed to stop system: {}", e)),
                    }
                } else {
                    self.messages.push("✗ State manager not available".to_string());
                }
            }
            "init" => {
                // Check if system is stopped before allowing init commands
                if let Some(ref state_manager) = self.state_manager {
                    match state_manager.get_state() {
                        Ok(current_state) => {
                            if current_state.status.state != crate::battery::SystemState::Offline {
                                self.messages.push("✗ Init commands only allowed when system is stopped. Use 'stop' command first.".to_string());
                                return;
                            }
                        }
                        Err(e) => {
                            self.messages.push(format!("✗ Failed to get system state: {}", e));
                            return;
                        }
                    }
                } else {
                    self.messages.push("✗ State manager not available".to_string());
                    return;
                }
                
                if cmd_parts.len() >= 2 {
                    match cmd_parts[1].to_lowercase().as_str() {
                        "reset" => {
                            if let Some(ref state_manager) = self.state_manager {
                                match state_manager.reset_to_defaults() {
                                    Ok(_) => {
                                        self.messages.push("✓ Reset battery parameters to defaults".to_string());
                                    }
                                    Err(e) => self.messages.push(format!("✗ Failed to reset: {}", e)),
                                }
                            } else {
                                self.messages.push("✗ State manager not available".to_string());
                            }
                        }
                        "soc" => {
                            if cmd_parts.len() >= 3 {
                                if let Ok(soc) = cmd_parts[2].parse::<f64>() {
                                    if Self::validate_soc(soc) {
                                        if let Some(ref state_manager) = self.state_manager {
                                            match state_manager.init_soc(soc) {
                                                Ok(_) => {
                                                    self.messages.push(format!("✓ Set initial SOC to {:.1}%", soc));
                                                }
                                                Err(e) => self.messages.push(format!("✗ Failed to set SOC: {}", e)),
                                            }
                                        } else {
                                            self.messages.push("✗ State manager not available".to_string());
                                        }
                                    } else {
                                        self.messages.push("SOC must be between 0.0 and 100.0%".to_string());
                                    }
                                } else {
                                    self.messages.push("Invalid SOC value".to_string());
                                }
                            } else {
                                self.messages.push("Usage: init soc <percentage>".to_string());
                            }
                        }
                        "capacity" => {
                            if cmd_parts.len() >= 3 {
                                if let Ok(capacity) = cmd_parts[2].parse::<f64>() {
                                    if Self::validate_capacity(capacity) {
                                        if let Some(ref state_manager) = self.state_manager {
                                            match state_manager.init_capacity(capacity) {
                                                Ok(_) => {
                                                    self.messages.push(format!("✓ Set battery capacity to {:.1} kWh", capacity));
                                                }
                                                Err(e) => self.messages.push(format!("✗ Failed to set capacity: {}", e)),
                                            }
                                        } else {
                                            self.messages.push("✗ State manager not available".to_string());
                                        }
                                    } else {
                                        self.messages.push("Capacity must be between 100.0 and 1000.0 kWh".to_string());
                                    }
                                } else {
                                    self.messages.push("Invalid capacity value".to_string());
                                }
                            } else {
                                self.messages.push("Usage: init capacity <kWh>".to_string());
                            }
                        }
                        "voltage" => {
                            if cmd_parts.len() >= 3 {
                                if let Ok(voltage) = cmd_parts[2].parse::<f64>() {
                                    if Self::validate_voltage(voltage) {
                                        if let Some(ref state_manager) = self.state_manager {
                                            match state_manager.init_voltage(voltage) {
                                                Ok(_) => self.messages.push(format!("✓ Set nominal voltage to {:.1} V", voltage)),
                                                Err(e) => self.messages.push(format!("✗ Failed to set voltage: {}", e)),
                                            }
                                        } else {
                                            self.messages.push("✗ State manager not available".to_string());
                                        }
                                    } else {
                                        self.messages.push("Voltage must be between 350.0 and 450.0 V".to_string());
                                    }
                                } else {
                                    self.messages.push("Invalid voltage value".to_string());
                                }
                            } else {
                                self.messages.push("Usage: init voltage <volts>".to_string());
                            }
                        }
                        "temp" => {
                            if cmd_parts.len() >= 3 {
                                if let Ok(temp) = cmd_parts[2].parse::<f64>() {
                                    if Self::validate_temperature(temp) {
                                        if let Some(ref state_manager) = self.state_manager {
                                            match state_manager.init_temperature(temp) {
                                                Ok(_) => self.messages.push(format!("✓ Set initial temperature to {:.1}°C", temp)),
                                                Err(e) => self.messages.push(format!("✗ Failed to set temperature: {}", e)),
                                            }
                                        } else {
                                            self.messages.push("✗ State manager not available".to_string());
                                        }
                                    } else {
                                        self.messages.push("Temperature must be between -10.0 and 50.0°C".to_string());
                                    }
                                } else {
                                    self.messages.push("Invalid temperature value".to_string());
                                }
                            } else {
                                self.messages.push("Usage: init temp <celsius>".to_string());
                            }
                        }
                        "soh" => {
                            if cmd_parts.len() >= 3 {
                                if let Ok(soh) = cmd_parts[2].parse::<f64>() {
                                    if Self::validate_soh(soh) {
                                        if let Some(ref state_manager) = self.state_manager {
                                            match state_manager.init_soh(soh) {
                                                Ok(_) => self.messages.push(format!("✓ Set State of Health to {:.1}%", soh)),
                                                Err(e) => self.messages.push(format!("✗ Failed to set SOH: {}", e)),
                                            }
                                        } else {
                                            self.messages.push("✗ State manager not available".to_string());
                                        }
                                    } else {
                                        self.messages.push("SOH must be between 50.0 and 100.0%".to_string());
                                    }
                                } else {
                                    self.messages.push("Invalid SOH value".to_string());
                                }
                            } else {
                                self.messages.push("Usage: init soh <percentage>".to_string());
                            }
                        }
                        "resistance" => {
                            if cmd_parts.len() >= 3 {
                                if let Ok(resistance) = cmd_parts[2].parse::<f64>() {
                                    if Self::validate_resistance(resistance) {
                                        if let Some(ref state_manager) = self.state_manager {
                                            match state_manager.init_resistance(resistance) {
                                                Ok(_) => self.messages.push(format!("✓ Set internal resistance to {:.2} mΩ", resistance)),
                                                Err(e) => self.messages.push(format!("✗ Failed to set resistance: {}", e)),
                                            }
                                        } else {
                                            self.messages.push("✗ State manager not available".to_string());
                                        }
                                    } else {
                                        self.messages.push("Resistance must be between 0.1 and 10.0 mΩ".to_string());
                                    }
                                } else {
                                    self.messages.push("Invalid resistance value".to_string());
                                }
                            } else {
                                self.messages.push("Usage: init resistance <milliohms>".to_string());
                            }
                        }
                        "cycles" => {
                            if cmd_parts.len() >= 3 {
                                if let Ok(cycles) = cmd_parts[2].parse::<u32>() {
                                    if Self::validate_cycles(cycles) {
                                        if let Some(ref state_manager) = self.state_manager {
                                            match state_manager.init_cycles(cycles) {
                                                Ok(_) => self.messages.push(format!("✓ Set cycle count to {}", cycles)),
                                                Err(e) => self.messages.push(format!("✗ Failed to set cycles: {}", e)),
                                            }
                                        } else {
                                            self.messages.push("✗ State manager not available".to_string());
                                        }
                                    } else {
                                        self.messages.push("Cycles must be between 0 and 10000".to_string());
                                    }
                                } else {
                                    self.messages.push("Invalid cycles value".to_string());
                                }
                            } else {
                                self.messages.push("Usage: init cycles <count>".to_string());
                            }
                        }
                        _ => {
                            self.messages.push("Available init commands: soc, capacity, voltage, temp, soh, resistance, cycles, reset".to_string());
                        }
                    }
                } else {
                    self.messages.push("Usage: init <parameter> <value> or init reset".to_string());
                }
            }
            "help" => {
                self.messages.push("Available commands:".to_string());
                self.messages.push("  start - Start battery system".to_string());
                self.messages.push("  stop - Stop battery system".to_string());
                self.messages.push("  set power <kW> - Set power setpoint".to_string());
                self.messages.push("  init soc <0-100> - Set initial SOC (when stopped)".to_string()); 
                self.messages.push("  init capacity <kWh> - Set battery capacity (when stopped)".to_string());
                self.messages.push("  init reset - Reset to defaults (when stopped)".to_string());
                self.messages.push("  emergency_stop, maintenance_mode".to_string());
            }
            _ => {
                self.messages.push(format!("Unknown command: {}", cmd_parts[0]));
            }
        }
        
        // Keep only last 100 messages
        if self.messages.len() > 100 {
            self.messages.drain(0..50);
        }
    }
}

pub fn draw_ui(f: &mut Frame, app: &App, battery_state: Option<BatteryState>) {
    // Create a fixed-width centered area (80 columns wide)
    let fixed_width_area = centered_rect_fixed(80, f.size());
    
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(0)
        .constraints([
            Constraint::Length(1), // Compact header
            Constraint::Min(0),    // Main content area
            Constraint::Length(4), // Compact messages panel
            Constraint::Length(1), // Compact status bar
        ])
        .split(fixed_width_area);

    // Compact header without borders
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("battsim2", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" - Battery Energy Storage System Simulator"),
        ])
    ])
    .alignment(Alignment::Center);
    f.render_widget(header, chunks[0]);

    // Main content area - split into three columns with fixed widths
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(24), // Left column - fixed width
            Constraint::Length(32), // Center column (battery gauge) - wider for gauge
            Constraint::Length(24), // Right column - fixed width
        ])
        .split(chunks[1]);

    // Left column - split into top and bottom (taller widgets like specification)
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Electrical parameters - taller
            Constraint::Percentage(40), // Cell monitoring - shorter
        ])
        .split(main_chunks[0]);

    // Right column - split into top and bottom (taller widgets like specification)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(40), // System info - shorter 
            Constraint::Percentage(60), // System status - taller
        ])
        .split(main_chunks[2]);

    // Render all widgets
    render_electrical_params(f, left_chunks[0], battery_state.as_ref());
    render_cell_monitoring(f, left_chunks[1], battery_state.as_ref());
    render_battery_gauge(f, main_chunks[1], battery_state.as_ref());
    render_system_info(f, right_chunks[0], battery_state.as_ref());
    render_system_status(f, right_chunks[1], battery_state.as_ref());

    // Status bar
    let status_text = match app.input_mode {
        InputMode::Normal => vec![
            Line::from(vec![
                Span::styled("Commands: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::styled("[Q]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("uit  "),
                Span::styled("[R]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("eset  "),
                Span::styled("[S]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("tart/Stop System  "),
                Span::styled("[C]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("ommand  "),
                Span::styled("[H]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw("elp"),
            ])
        ],
        InputMode::Command => vec![
            Line::from(vec![
                Span::styled("Command: ", Style::default().add_modifier(Modifier::BOLD)),
                Span::raw(&app.command_input),
                Span::styled("_", Style::default().add_modifier(Modifier::SLOW_BLINK)),
                Span::raw("  [Enter] Execute  [Esc] Cancel  [↑/↓] History"),
            ]),
        ],
        InputMode::Help => vec![
            Line::from(vec![
                Span::styled("Help Mode - Press H or Esc to close", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
        ],
    };
    
    // Messages panel
    let messages: Vec<Line> = app.messages.iter()
        .rev() // Show newest messages first
        .take(3) // Show last 3 messages for more compact layout
        .rev() // Reverse back to show in chronological order
        .map(|msg| Line::from(Span::raw(msg.clone())))
        .collect();
    
    // Compact messages without border
    let messages_widget = Paragraph::new(messages)
        .wrap(ratatui::widgets::Wrap { trim: true });
    f.render_widget(messages_widget, chunks[2]);

    // Compact status bar without border
    let status = Paragraph::new(status_text);
    f.render_widget(status, chunks[3]);
    
    // Help overlay
    if app.input_mode == InputMode::Help {
        let help_area = centered_rect(60, 70, f.size());
        f.render_widget(ratatui::widgets::Clear, help_area);
        
        let help_text = vec![
            Line::from(vec![
                Span::styled("Help - Battery Simulator Controls", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            Line::from("Keyboard Commands:"),
            Line::from(vec![
                Span::styled("  Q", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" - Quit application"),
            ]),
            Line::from(vec![
                Span::styled("  R", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" - Reset faults and warnings"),
            ]),
            Line::from(vec![
                Span::styled("  S", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" - Toggle system start/stop"),
            ]),
            Line::from(vec![
                Span::styled("  C", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" - Enter command mode"),
            ]),
            Line::from(vec![
                Span::styled("  H", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                Span::raw(" - Show this help"),
            ]),
            Line::from(""),
            Line::from("Command Mode (press C or :):"),
            Line::from(vec![
                Span::styled("System Control:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  start", Style::default().fg(Color::Green)),
                Span::raw(" - Start the battery system"),
            ]),
            Line::from(vec![
                Span::styled("  stop", Style::default().fg(Color::Green)),
                Span::raw(" - Stop the battery system"),
            ]),
            Line::from(vec![
                Span::styled("  emergency_stop", Style::default().fg(Color::Red)),
                Span::raw(" - Immediate system shutdown"),
            ]),
            Line::from(vec![
                Span::styled("  maintenance_mode", Style::default().fg(Color::Blue)),
                Span::raw(" - Enter maintenance state"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Power Control:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  set power <kW>", Style::default().fg(Color::Green)),
                Span::raw(" - Set power setpoint"),
            ]),
            Line::from(vec![
                Span::styled("  set soc_min <%>", Style::default().fg(Color::Green)),
                Span::raw(" - Set minimum SOC limit"),
            ]),
            Line::from(vec![
                Span::styled("  set soc_max <%>", Style::default().fg(Color::Green)),
                Span::raw(" - Set maximum SOC limit"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Init Commands (system must be stopped):", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(vec![
                Span::styled("  init soc <%>", Style::default().fg(Color::Yellow)),
                Span::raw(" - Set initial SOC (0-100%)"),
            ]),
            Line::from(vec![
                Span::styled("  init capacity <kWh>", Style::default().fg(Color::Yellow)),
                Span::raw(" - Set battery capacity (100-1000 kWh)"),
            ]),
            Line::from(vec![
                Span::styled("  init voltage <V>", Style::default().fg(Color::Yellow)),
                Span::raw(" - Set nominal voltage (350-450V)"),
            ]),
            Line::from(vec![
                Span::styled("  init temperature <°C>", Style::default().fg(Color::Yellow)),
                Span::raw(" - Set cell temperature (-10 to 50°C)"),
            ]),
            Line::from(vec![
                Span::styled("  init soh <%>", Style::default().fg(Color::Yellow)),
                Span::raw(" - Set State of Health (50-100%)"),
            ]),
            Line::from(vec![
                Span::styled("  init resistance <mΩ>", Style::default().fg(Color::Yellow)),
                Span::raw(" - Set internal resistance (0.1-10 mΩ)"),
            ]),
            Line::from(vec![
                Span::styled("  init cycles <count>", Style::default().fg(Color::Yellow)),
                Span::raw(" - Set cycle count (0-10000)"),
            ]),
            Line::from(vec![
                Span::styled("  init reset", Style::default().fg(Color::Yellow)),
                Span::raw(" - Reset all parameters to defaults"),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("Press H or Esc to close this help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
        ];
        
        let help_popup = Paragraph::new(help_text)
            .block(Block::default().title("Help").borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)))
            .wrap(ratatui::widgets::Wrap { trim: true });
        
        f.render_widget(help_popup, help_area);
    }
}

pub async fn run_ui(
    mut battery_rx: watch::Receiver<BatteryState>, 
    state_manager: BatteryStateManager
) -> Result<(), Box<dyn std::error::Error>> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app state
    let mut app = App::new();
    app.set_state_manager(state_manager.clone());
    
    let tick_rate = app.tick_rate;
    let mut last_tick = std::time::Instant::now();

    loop {
        // Get the latest battery state (non-blocking)
        let battery_state = battery_rx.borrow_and_update().clone();
        
        // Apply any pending setpoints directly to the battery simulation
        if let Some(setpoints) = app.pending_setpoints.take() {
            let power_value = setpoints.power_setpoint;
            match state_manager.update_setpoints(setpoints) {
                Ok(_) => {
                    app.messages.push(format!("✓ Applied setpoints: power={:.1}kW", power_value));
                }
                Err(e) => {
                    app.messages.push(format!("✗ Failed to apply setpoints: {}", e));
                }
            }
        }
        
        terminal.draw(|f| draw_ui(f, &app, Some(battery_state)))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if crossterm::event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                match app.input_mode {
                    InputMode::Normal => match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            app.quit();
                            break;
                        }
                        KeyCode::Char(':') => {
                            app.toggle_input_mode();
                        }
                        KeyCode::Char('c') | KeyCode::Char('C') => {
                            app.toggle_input_mode();
                        }
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app.reset_faults();
                        }
                        KeyCode::Char('s') | KeyCode::Char('S') => {
                            app.toggle_system_operation();
                        }
                        KeyCode::Char('h') | KeyCode::Char('H') => {
                            app.show_help();
                        }
                        _ => {}
                    },
                    InputMode::Command => match key.code {
                        KeyCode::Enter => {
                            app.submit_command();
                        }
                        KeyCode::Char(c) => {
                            app.enter_char(c);
                        }
                        KeyCode::Backspace => {
                            app.delete_char();
                        }
                        KeyCode::Esc => {
                            app.toggle_input_mode();
                        }
                        KeyCode::Up => {
                            app.navigate_history(1);
                        }
                        KeyCode::Down => {
                            app.navigate_history(-1);
                        }
                        _ => {}
                    },
                    InputMode::Help => match key.code {
                        KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('H') => {
                            app.toggle_input_mode();
                        }
                        _ => {}
                    },
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = std::time::Instant::now();
        }
        
        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

/// Helper function to center a popup in the given area
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

/// Create a centered rectangle with fixed width in columns
fn centered_rect_fixed(width: u16, r: Rect) -> Rect {
    let horizontal_margin = if r.width > width {
        (r.width - width) / 2
    } else {
        0
    };
    
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(horizontal_margin),
            Constraint::Length(width.min(r.width)),
            Constraint::Length(horizontal_margin),
        ])
        .split(r)[1]
}