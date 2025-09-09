use std::sync::Arc;
use tokio::sync::{RwLock, watch};
use tokio::net::{TcpListener, TcpStream};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::time::{timeout, Duration};
use anyhow::{Result, Context};
use tracing::{info, warn, error, debug};

use crate::battery::types::*;
use super::registers::ModbusRegisterMap;

/// Modbus function codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FunctionCode {
    ReadHoldingRegisters = 0x03,
    ReadInputRegisters = 0x04,
    WriteSingleRegister = 0x06,
    WriteMultipleRegisters = 0x10,
}

/// Modbus exception codes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExceptionCode {
    IllegalFunction = 0x01,
    IllegalDataAddress = 0x02,
    IllegalDataValue = 0x03,
    SlaveDeviceFailure = 0x04,
}

/// Modbus request frame structure
#[derive(Debug)]
pub struct ModbusRequest {
    pub transaction_id: u16,
    pub protocol_id: u16,
    pub length: u16,
    pub unit_id: u8,
    pub function_code: u8,
    pub data: Vec<u8>,
}

/// Modbus response frame structure
#[derive(Debug)]
pub struct ModbusResponse {
    pub transaction_id: u16,
    pub protocol_id: u16,
    pub length: u16,
    pub unit_id: u8,
    pub function_code: u8,
    pub data: Vec<u8>,
}

/// Modbus TCP server
pub struct ModbusTcpServer {
    register_map: Arc<RwLock<ModbusRegisterMap>>,
    battery_state_rx: watch::Receiver<BatteryState>,
    setpoints_tx: tokio::sync::mpsc::Sender<ControlSetpoints>,
    config_tx: tokio::sync::mpsc::Sender<BatteryConfig>,
    client_count: Arc<RwLock<u32>>,
    unit_id: u8,
}

impl ModbusTcpServer {
    /// Create a new Modbus TCP server
    pub fn new(
        battery_state_rx: watch::Receiver<BatteryState>,
        setpoints_tx: tokio::sync::mpsc::Sender<ControlSetpoints>,
        config_tx: tokio::sync::mpsc::Sender<BatteryConfig>,
    ) -> Self {
        Self {
            register_map: Arc::new(RwLock::new(ModbusRegisterMap::new())),
            battery_state_rx,
            setpoints_tx,
            config_tx,
            client_count: Arc::new(RwLock::new(0)),
            unit_id: 1, // Default unit ID
        }
    }

    /// Start the Modbus TCP server
    pub async fn start(&self, bind_address: &str) -> Result<()> {
        let listener = TcpListener::bind(bind_address).await
            .with_context(|| format!("Failed to bind to {}", bind_address))?;
        
        info!("Modbus TCP server listening on {}", bind_address);

        // Start register update task
        let register_map = self.register_map.clone();
        let mut battery_rx = self.battery_state_rx.clone();
        tokio::spawn(async move {
            while battery_rx.changed().await.is_ok() {
                let state = battery_rx.borrow().clone();
                let mut map = register_map.write().await;
                if let Err(e) = map.update_input_registers(&state) {
                    error!("Failed to update input registers: {}", e);
                }
                if let Err(e) = map.update_holding_registers(&state) {
                    error!("Failed to update holding registers: {}", e);
                }
            }
        });

        // Accept client connections
        while let Ok((stream, addr)) = listener.accept().await {
            info!("New Modbus client connected from {}", addr);
            
            // Increment client count
            {
                let mut count = self.client_count.write().await;
                *count += 1;
            }

            // Handle client in separate task
            let server = self.clone();
            tokio::spawn(async move {
                if let Err(e) = server.handle_client(stream).await {
                    warn!("Client {} disconnected with error: {}", addr, e);
                } else {
                    info!("Client {} disconnected cleanly", addr);
                }
                
                // Decrement client count
                let mut count = server.client_count.write().await;
                *count = count.saturating_sub(1);
            });
        }

        Ok(())
    }

    /// Get current client count
    pub async fn get_client_count(&self) -> u32 {
        *self.client_count.read().await
    }

    /// Handle a single client connection
    async fn handle_client(&self, mut stream: TcpStream) -> Result<()> {
        let mut buffer = vec![0u8; 1024];
        
        loop {
            // Read request with timeout
            let bytes_read = match timeout(Duration::from_secs(30), stream.read(&mut buffer)).await {
                Ok(Ok(0)) => break, // Client disconnected
                Ok(Ok(n)) => n,
                Ok(Err(e)) => return Err(e.into()),
                Err(_) => {
                    warn!("Client read timeout");
                    break;
                }
            };

            debug!("Received {} bytes from client", bytes_read);

            // Parse request
            let request = match self.parse_request(&buffer[..bytes_read]) {
                Ok(req) => req,
                Err(e) => {
                    error!("Failed to parse Modbus request: {}", e);
                    continue;
                }
            };

            // Process request and generate response
            let response = self.process_request(request).await;
            
            // Send response
            let response_bytes = self.serialize_response(&response)?;
            if let Err(e) = stream.write_all(&response_bytes).await {
                error!("Failed to send response: {}", e);
                break;
            }

            debug!("Sent {} bytes response to client", response_bytes.len());
        }

        Ok(())
    }

    /// Parse Modbus TCP request
    fn parse_request(&self, data: &[u8]) -> Result<ModbusRequest> {
        if data.len() < 8 {
            return Err(anyhow::anyhow!("Request too short"));
        }

        let transaction_id = u16::from_be_bytes([data[0], data[1]]);
        let protocol_id = u16::from_be_bytes([data[2], data[3]]);
        let length = u16::from_be_bytes([data[4], data[5]]);
        let unit_id = data[6];
        let function_code = data[7];

        if protocol_id != 0 {
            return Err(anyhow::anyhow!("Invalid protocol ID: {}", protocol_id));
        }

        if data.len() < (6 + length as usize) {
            return Err(anyhow::anyhow!("Incomplete request"));
        }

        let request_data = data[8..(6 + length as usize)].to_vec();

        Ok(ModbusRequest {
            transaction_id,
            protocol_id,
            length,
            unit_id,
            function_code,
            data: request_data,
        })
    }

    /// Process Modbus request and generate response
    async fn process_request(&self, request: ModbusRequest) -> ModbusResponse {
        // Check unit ID
        if request.unit_id != self.unit_id {
            return self.create_exception_response(
                request.transaction_id, 
                request.unit_id, 
                request.function_code, 
                ExceptionCode::SlaveDeviceFailure
            );
        }

        match request.function_code {
            0x03 => self.handle_read_holding_registers(request).await,
            0x04 => self.handle_read_input_registers(request).await,
            0x06 => self.handle_write_single_register(request).await,
            0x10 => self.handle_write_multiple_registers(request).await,
            _ => self.create_exception_response(
                request.transaction_id,
                request.unit_id,
                request.function_code,
                ExceptionCode::IllegalFunction
            ),
        }
    }

    /// Handle read holding registers (function code 03)
    async fn handle_read_holding_registers(&self, request: ModbusRequest) -> ModbusResponse {
        if request.data.len() != 4 {
            return self.create_exception_response(
                request.transaction_id, 
                request.unit_id, 
                request.function_code,
                ExceptionCode::IllegalDataValue
            );
        }

        let start_address = u16::from_be_bytes([request.data[0], request.data[1]]);
        let register_count = u16::from_be_bytes([request.data[2], request.data[3]]);

        if register_count > 125 || register_count == 0 {
            return self.create_exception_response(
                request.transaction_id,
                request.unit_id, 
                request.function_code,
                ExceptionCode::IllegalDataValue
            );
        }

        let register_map = self.register_map.read().await;
        let register_values = register_map.read_holding_registers(start_address, register_count);
        
        let mut response_data = vec![register_count as u8 * 2]; // Byte count
        for value in register_values {
            response_data.extend_from_slice(&value.to_be_bytes());
        }

        ModbusResponse {
            transaction_id: request.transaction_id,
            protocol_id: 0,
            length: (3 + response_data.len()) as u16,
            unit_id: request.unit_id,
            function_code: request.function_code,
            data: response_data,
        }
    }

    /// Handle read input registers (function code 04)  
    async fn handle_read_input_registers(&self, request: ModbusRequest) -> ModbusResponse {
        if request.data.len() != 4 {
            return self.create_exception_response(
                request.transaction_id,
                request.unit_id,
                request.function_code,
                ExceptionCode::IllegalDataValue
            );
        }

        let start_address = u16::from_be_bytes([request.data[0], request.data[1]]);
        let register_count = u16::from_be_bytes([request.data[2], request.data[3]]);

        if register_count > 125 || register_count == 0 {
            return self.create_exception_response(
                request.transaction_id,
                request.unit_id,
                request.function_code, 
                ExceptionCode::IllegalDataValue
            );
        }

        let register_map = self.register_map.read().await;
        let register_values = register_map.read_input_registers(start_address, register_count);
        
        let mut response_data = vec![register_count as u8 * 2]; // Byte count
        for value in register_values {
            response_data.extend_from_slice(&value.to_be_bytes());
        }

        ModbusResponse {
            transaction_id: request.transaction_id,
            protocol_id: 0,
            length: (3 + response_data.len()) as u16,
            unit_id: request.unit_id,
            function_code: request.function_code,
            data: response_data,
        }
    }

    /// Handle write single register (function code 06)
    async fn handle_write_single_register(&self, request: ModbusRequest) -> ModbusResponse {
        if request.data.len() != 4 {
            return self.create_exception_response(
                request.transaction_id,
                request.unit_id,
                request.function_code,
                ExceptionCode::IllegalDataValue
            );
        }

        let register_address = u16::from_be_bytes([request.data[0], request.data[1]]);
        let register_value = u16::from_be_bytes([request.data[2], request.data[3]]);

        // Write to register map
        {
            let mut register_map = self.register_map.write().await;
            if register_map.write_holding_register(register_address, register_value).is_err() {
                return self.create_exception_response(
                    request.transaction_id,
                    request.unit_id,
                    request.function_code,
                    ExceptionCode::IllegalDataAddress
                );
            }

            // Extract and send setpoints/config changes
            if let Err(e) = self.send_register_updates(&register_map, register_address).await {
                error!("Failed to send register updates: {}", e);
            }
        }

        // Echo back the request (standard Modbus response for function 06)
        ModbusResponse {
            transaction_id: request.transaction_id,
            protocol_id: 0,
            length: 6,
            unit_id: request.unit_id,
            function_code: request.function_code,
            data: request.data,
        }
    }

    /// Handle write multiple registers (function code 16)
    async fn handle_write_multiple_registers(&self, request: ModbusRequest) -> ModbusResponse {
        if request.data.len() < 5 {
            return self.create_exception_response(
                request.transaction_id,
                request.unit_id,
                request.function_code,
                ExceptionCode::IllegalDataValue
            );
        }

        let start_address = u16::from_be_bytes([request.data[0], request.data[1]]);
        let register_count = u16::from_be_bytes([request.data[2], request.data[3]]);
        let byte_count = request.data[4] as usize;

        if register_count > 123 || register_count == 0 || byte_count != register_count as usize * 2 {
            return self.create_exception_response(
                request.transaction_id,
                request.unit_id,
                request.function_code,
                ExceptionCode::IllegalDataValue
            );
        }

        if request.data.len() != (5 + byte_count) {
            return self.create_exception_response(
                request.transaction_id,
                request.unit_id,
                request.function_code,
                ExceptionCode::IllegalDataValue
            );
        }

        // Parse register values
        let mut register_values = Vec::new();
        for i in 0..register_count {
            let offset = 5 + (i as usize * 2);
            let value = u16::from_be_bytes([request.data[offset], request.data[offset + 1]]);
            register_values.push(value);
        }

        // Write to register map
        {
            let mut register_map = self.register_map.write().await;
            if register_map.write_holding_registers(start_address, &register_values).is_err() {
                return self.create_exception_response(
                    request.transaction_id,
                    request.unit_id,
                    request.function_code,
                    ExceptionCode::IllegalDataAddress
                );
            }

            // Extract and send setpoints/config changes
            for i in 0..register_count {
                if let Err(e) = self.send_register_updates(&register_map, start_address + i).await {
                    error!("Failed to send register updates: {}", e);
                }
            }
        }

        // Response contains start address and register count
        let response_data = vec![
            request.data[0], request.data[1], // Start address  
            request.data[2], request.data[3], // Register count
        ];

        ModbusResponse {
            transaction_id: request.transaction_id,
            protocol_id: 0,
            length: 6,
            unit_id: request.unit_id,
            function_code: request.function_code,
            data: response_data,
        }
    }

    /// Send register updates to battery simulation
    async fn send_register_updates(&self, register_map: &ModbusRegisterMap, changed_address: u16) -> Result<()> {
        // Check if this is a setpoint or config register
        if (40001..=40100).contains(&changed_address) {
            // Control setpoints range
            let setpoints = register_map.extract_setpoints_from_registers()?;
            if self.setpoints_tx.try_send(setpoints).is_err() {
                debug!("Setpoints channel full, skipping update");
            }
        } else if (40101..=40200).contains(&changed_address) {
            // Configuration range  
            let config = register_map.extract_config_from_registers()?;
            if self.config_tx.try_send(config).is_err() {
                debug!("Config channel full, skipping update");
            }
        }

        Ok(())
    }

    /// Create exception response
    fn create_exception_response(
        &self,
        transaction_id: u16,
        unit_id: u8,
        function_code: u8,
        exception_code: ExceptionCode,
    ) -> ModbusResponse {
        ModbusResponse {
            transaction_id,
            protocol_id: 0,
            length: 3,
            unit_id,
            function_code: function_code | 0x80, // Set exception bit
            data: vec![exception_code as u8],
        }
    }

    /// Serialize Modbus response to bytes
    fn serialize_response(&self, response: &ModbusResponse) -> Result<Vec<u8>> {
        let mut bytes = Vec::new();
        
        bytes.extend_from_slice(&response.transaction_id.to_be_bytes());
        bytes.extend_from_slice(&response.protocol_id.to_be_bytes());
        bytes.extend_from_slice(&response.length.to_be_bytes());
        bytes.push(response.unit_id);
        bytes.push(response.function_code);
        bytes.extend_from_slice(&response.data);
        
        Ok(bytes)
    }
}

impl Clone for ModbusTcpServer {
    fn clone(&self) -> Self {
        Self {
            register_map: self.register_map.clone(),
            battery_state_rx: self.battery_state_rx.clone(),
            setpoints_tx: self.setpoints_tx.clone(),
            config_tx: self.config_tx.clone(),
            client_count: self.client_count.clone(),
            unit_id: self.unit_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[test]
    fn test_request_parsing() {
        // Create mock channels for testing
        let (state_tx, state_rx) = watch::channel(BatteryState::default());
        let (setpoints_tx, _) = mpsc::channel(10);
        let (config_tx, _) = mpsc::channel(10);
        
        let server = ModbusTcpServer::new(state_rx, setpoints_tx, config_tx);

        // Test read holding registers request
        let request_bytes = vec![
            0x00, 0x01, // Transaction ID
            0x00, 0x00, // Protocol ID  
            0x00, 0x06, // Length
            0x01,       // Unit ID
            0x03,       // Function code (read holding registers)
            0x9C, 0x41, // Start address (40001)
            0x00, 0x02, // Register count (2)
        ];

        let request = server.parse_request(&request_bytes).unwrap();
        assert_eq!(request.transaction_id, 1);
        assert_eq!(request.function_code, 0x03);
        assert_eq!(request.unit_id, 1);
        assert_eq!(request.data, vec![0x9C, 0x41, 0x00, 0x02]);
    }

    #[test]
    fn test_response_serialization() {
        let (state_tx, state_rx) = watch::channel(BatteryState::default());
        let (setpoints_tx, _) = mpsc::channel(10);
        let (config_tx, _) = mpsc::channel(10);
        
        let server = ModbusTcpServer::new(state_rx, setpoints_tx, config_tx);

        let response = ModbusResponse {
            transaction_id: 1,
            protocol_id: 0,
            length: 5,
            unit_id: 1,
            function_code: 0x03,
            data: vec![0x02, 0x12, 0x34], // 2 bytes of data: 0x1234
        };

        let serialized = server.serialize_response(&response).unwrap();
        let expected = vec![
            0x00, 0x01, // Transaction ID
            0x00, 0x00, // Protocol ID
            0x00, 0x05, // Length
            0x01,       // Unit ID
            0x03,       // Function code
            0x02, 0x12, 0x34, // Data
        ];

        assert_eq!(serialized, expected);
    }
}