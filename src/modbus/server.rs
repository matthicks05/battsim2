use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, RwLock};
use tokio_modbus::server::tcp::Server;
use tokio_modbus::server::Service;
use tokio_modbus::{Exception, Request, Response};
use tokio_modbus::prelude::SlaveRequest;
use tracing::{error, info, warn};

use crate::battery::types::*;
use crate::battery::BatteryStateManager;
use super::registers::addresses::{HOLDING_REGISTER_BASE, INPUT_REGISTER_BASE};
use super::registers::ModbusRegisterMap;

/// Applies a holding-register write to the register map, then forwards a
/// setpoints/config update to the simulation if the write landed in one of
/// those ranges (mirrors the pre-migration hand-rolled dispatch).
fn send_register_updates(
    register_map: &ModbusRegisterMap,
    changed_address: u16,
    setpoints_tx: &tokio::sync::mpsc::Sender<ControlSetpoints>,
    config_tx: &tokio::sync::mpsc::Sender<BatteryConfig>,
) {
    if (40001..=40100).contains(&changed_address) {
        if let Ok(setpoints) = register_map.extract_setpoints_from_registers() {
            if setpoints_tx.try_send(setpoints).is_err() {
                tracing::debug!("Setpoints channel full, skipping update");
            }
        }
    } else if (40101..=40200).contains(&changed_address) {
        if let Ok(config) = register_map.extract_config_from_registers() {
            if config_tx.try_send(config).is_err() {
                tracing::debug!("Config channel full, skipping update");
            }
        }
    }
}

/// The actual Modbus request handler, shared (via `Arc`) across every client connection.
struct BattsimService {
    register_map: Arc<RwLock<ModbusRegisterMap>>,
    setpoints_tx: tokio::sync::mpsc::Sender<ControlSetpoints>,
    config_tx: tokio::sync::mpsc::Sender<BatteryConfig>,
    unit_id: u8,
}

impl Service for BattsimService {
    type Request = SlaveRequest<'static>;
    type Future = Pin<Box<dyn Future<Output = Result<Response, Exception>> + Send>>;

    fn call(&self, req: Self::Request) -> Self::Future {
        let register_map = self.register_map.clone();
        let setpoints_tx = self.setpoints_tx.clone();
        let config_tx = self.config_tx.clone();
        let unit_id = self.unit_id;

        Box::pin(async move {
            if req.slave != unit_id {
                return Err(Exception::ServerDeviceFailure);
            }

            match req.request {
                Request::ReadHoldingRegisters(addr, count) => {
                    if count == 0 || count > 125 {
                        return Err(Exception::IllegalDataValue);
                    }
                    // Wire addresses are 0-based per Modbus TCP; translate to this map's internal 40001+ addressing.
                    let internal_addr = addr.saturating_add(HOLDING_REGISTER_BASE);
                    let map = register_map.read().await;
                    let values = map.read_holding_registers(internal_addr, count);
                    Ok(Response::ReadHoldingRegisters(values))
                }
                Request::ReadInputRegisters(addr, count) => {
                    if count == 0 || count > 125 {
                        return Err(Exception::IllegalDataValue);
                    }
                    // Wire addresses are 0-based per Modbus TCP; translate to this map's internal 30001+ addressing.
                    let internal_addr = addr.saturating_add(INPUT_REGISTER_BASE);
                    let map = register_map.read().await;
                    let values = map.read_input_registers(internal_addr, count);
                    Ok(Response::ReadInputRegisters(values))
                }
                Request::WriteSingleRegister(addr, value) => {
                    let internal_addr = addr.saturating_add(HOLDING_REGISTER_BASE);
                    {
                        let mut map = register_map.write().await;
                        let _ = map.write_holding_register(internal_addr, value);
                        send_register_updates(&map, internal_addr, &setpoints_tx, &config_tx);
                    }
                    // Echo back the original wire address, not the translated one.
                    Ok(Response::WriteSingleRegister(addr, value))
                }
                Request::WriteMultipleRegisters(addr, values) => {
                    if values.is_empty() || values.len() > 123 {
                        return Err(Exception::IllegalDataValue);
                    }
                    let internal_addr = addr.saturating_add(HOLDING_REGISTER_BASE);
                    let count = values.len() as u16;
                    {
                        let mut map = register_map.write().await;
                        let _ = map.write_holding_registers(internal_addr, &values);
                        for i in 0..count {
                            send_register_updates(&map, internal_addr + i, &setpoints_tx, &config_tx);
                        }
                    }
                    Ok(Response::WriteMultipleRegisters(addr, count))
                }
                _ => Err(Exception::IllegalFunction),
            }
        })
    }
}

/// Per-connection wrapper: delegates to the shared `BattsimService` and tracks
/// the live client count for the lifetime of this TCP connection (decrementing
/// on drop, i.e. when the connection closes for any reason), pushing the
/// updated count into the shared battery state so it's visible on the
/// MODBUS_CLIENT_COUNT register and the TUI.
struct ConnectionService {
    inner: Arc<BattsimService>,
    client_count: Arc<AtomicU32>,
    state_manager: BatteryStateManager,
    peer_addr: SocketAddr,
}

impl Drop for ConnectionService {
    fn drop(&mut self) {
        let count = self.client_count.fetch_sub(1, Ordering::SeqCst) - 1;
        if let Err(e) = self.state_manager.set_modbus_clients(count) {
            error!("Failed to update Modbus client count: {}", e);
        }
        info!("Client {} disconnected", self.peer_addr);
    }
}

impl Service for ConnectionService {
    type Request = <BattsimService as Service>::Request;
    type Future = <BattsimService as Service>::Future;

    fn call(&self, req: Self::Request) -> Self::Future {
        self.inner.call(req)
    }
}

/// Modbus TCP server
pub struct ModbusTcpServer {
    register_map: Arc<RwLock<ModbusRegisterMap>>,
    battery_state_rx: watch::Receiver<BatteryState>,
    setpoints_tx: tokio::sync::mpsc::Sender<ControlSetpoints>,
    config_tx: tokio::sync::mpsc::Sender<BatteryConfig>,
    client_count: Arc<AtomicU32>,
    state_manager: BatteryStateManager,
    unit_id: u8,
}

impl ModbusTcpServer {
    /// Create a new Modbus TCP server
    pub fn new(
        battery_state_rx: watch::Receiver<BatteryState>,
        setpoints_tx: tokio::sync::mpsc::Sender<ControlSetpoints>,
        config_tx: tokio::sync::mpsc::Sender<BatteryConfig>,
        state_manager: BatteryStateManager,
    ) -> Self {
        Self {
            register_map: Arc::new(RwLock::new(ModbusRegisterMap::new())),
            battery_state_rx,
            setpoints_tx,
            config_tx,
            client_count: Arc::new(AtomicU32::new(0)),
            state_manager,
            unit_id: 1, // Default unit ID
        }
    }

    /// Start the Modbus TCP server
    pub async fn start(&self, bind_address: &str) -> Result<()> {
        let listener = TcpListener::bind(bind_address)
            .await
            .with_context(|| format!("Failed to bind to {}", bind_address))?;

        info!("Modbus TCP server listening on {}", bind_address);

        // Start register update task: keeps the register map in sync with the
        // live battery state, independent of any client connection.
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

        let service = Arc::new(BattsimService {
            register_map: self.register_map.clone(),
            setpoints_tx: self.setpoints_tx.clone(),
            config_tx: self.config_tx.clone(),
            unit_id: self.unit_id,
        });
        let client_count = self.client_count.clone();
        let state_manager = self.state_manager.clone();

        let on_connected = move |_stream: TcpStream, peer_addr: SocketAddr| {
            let inner = service.clone();
            let client_count = client_count.clone();
            let state_manager = state_manager.clone();
            async move {
                let count = client_count.fetch_add(1, Ordering::SeqCst) + 1;
                if let Err(e) = state_manager.set_modbus_clients(count) {
                    error!("Failed to update Modbus client count: {}", e);
                }
                info!("New Modbus client connected from {}", peer_addr);
                Ok(Some((
                    ConnectionService {
                        inner,
                        client_count,
                        state_manager,
                        peer_addr,
                    },
                    _stream,
                )))
            }
        };

        let server = Server::new(listener);
        server
            .serve(&on_connected, |e| {
                warn!("Modbus connection error: {}", e);
            })
            .await?;

        Ok(())
    }

    /// Get current client count
    pub async fn get_client_count(&self) -> u32 {
        self.client_count.load(Ordering::SeqCst)
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
            state_manager: self.state_manager.clone(),
            unit_id: self.unit_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use tokio_modbus::slave::SlaveId;

    fn make_service() -> BattsimService {
        BattsimService {
            register_map: Arc::new(RwLock::new(ModbusRegisterMap::new())),
            setpoints_tx: mpsc::channel(10).0,
            config_tx: mpsc::channel(10).0,
            unit_id: 1,
        }
    }

    fn slave_request(request: Request<'static>) -> SlaveRequest<'static> {
        SlaveRequest {
            slave: 1 as SlaveId,
            request,
        }
    }

    #[tokio::test]
    async fn test_read_input_registers_roundtrip() {
        let service = make_service();
        let state = BatteryState::default();
        {
            let mut map = service.register_map.write().await;
            map.update_input_registers(&state).unwrap();
        }

        // SOC lives at internal address 30201 -> wire offset 200
        let response = service
            .call(slave_request(Request::ReadInputRegisters(200, 1)))
            .await
            .unwrap();

        match response {
            Response::ReadInputRegisters(values) => {
                assert_eq!(values, vec![850]); // 85.0% * 10, per ElectricalParams default
            }
            other => panic!("unexpected response: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_write_single_register_roundtrip() {
        let service = make_service();

        // COMMAND lives at internal address 40001 -> wire offset 0
        let response = service
            .call(slave_request(Request::WriteSingleRegister(0, 1)))
            .await
            .unwrap();
        assert_eq!(response, Response::WriteSingleRegister(0, 1));

        let map = service.register_map.read().await;
        assert_eq!(map.read_holding_register(addresses_command()), Some(1));
    }

    fn addresses_command() -> u16 {
        super::super::registers::addresses::COMMAND
    }

    #[tokio::test]
    async fn test_wrong_unit_id_rejected() {
        let service = make_service();
        let mut req = slave_request(Request::ReadInputRegisters(200, 1));
        req.slave = 99;
        let result = service.call(req).await;
        assert_eq!(result, Err(Exception::ServerDeviceFailure));
    }
}
