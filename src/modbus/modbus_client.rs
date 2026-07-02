use std::{ sync::{Arc, RwLock}, time::Duration};
use tokio_modbus::{Slave, client::{Context, Reader, rtu, tcp, }};
use tokio::sync::{Mutex, mpsc, watch};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio_modbus::client::*; 

use tokio_serial::SerialStream;

use crate::modbus::share_data:: ShareDataRef;

pub type ModbusConfigRef = Arc<RwLock<ModbusConfig>>;

const CHUNK_SIZE: usize = 32;       


#[derive(Debug, thiserror::Error)]
enum ChunkedError {
    #[error("Modbus transport error: {0}")]
    Transport(#[from] tokio_modbus::Error),

    #[error("Business logic error: {0}")]
    Business(#[from] tokio_modbus::ExceptionCode),
}


/// Modbus 消息
#[derive(Debug, Clone)]
pub enum ModbusCommand {
    Disconnect,     // 断开连接
    Connect,        // 连接
    WriteCoil(u16, bool),
    WriteHoldRegister(u16, Vec<u16>),
}

pub struct ModbusConfig {
    pub protocol: Protocal,             // 协议
    pub tcp_config: TcpConfig,          // TCP配置
    pub rtu_config: RtuConfig,          // RTU配置
    pub poll_ms: u64,                   // 采样周期
    pub timeout: u64,                // 超时
}

impl ModbusConfig {
    pub fn new() -> Self {
        Self {
            protocol: Protocal::Tcp,
            tcp_config: TcpConfig::new(),
            rtu_config: RtuConfig::new(),
            poll_ms: 100,
            timeout: 500,
        }
    }
}

/// 协议抽象
#[derive(Clone, Debug, PartialEq)]
pub enum Protocal {
    Tcp,
    Rtu,
}

impl Protocal {
    pub fn as_str(
        &self,
    ) -> &'static str {
        match self {
            Protocal::Tcp => "TCP",
            Protocal::Rtu => "RTU",
        }
    }
}

/// modbus tcp config
#[derive(Clone, Debug, PartialEq)]
pub struct TcpConfig {
    pub ip: String,
    pub port: u16,
}
impl TcpConfig {
    pub fn new() -> Self {
        Self {
            ip: "127.0.0.1".to_string(),
            port: 502,
        }
    }
}

/// modbus rtu config
#[derive(Clone, Debug, PartialEq)]
pub struct RtuConfig {
    pub port: Option<String>,
    pub baudrate: u32,
    pub parity: char,
    pub slave: u8,
}
impl RtuConfig {
    pub fn new() -> Self {
        Self {
            port: None,
            baudrate: 115200,
            parity: 'N',
            slave: 1,
        }
    }
}

pub struct  ModbusClient {
    config: Arc<RwLock<ModbusConfig>>,
    share_data: ShareDataRef,
    modbus_ctx: Arc<Mutex<Option<Context>>>, 
    cmd_rx: Option<mpsc::Receiver<ModbusCommand>>,
    error_msg: watch::Sender<Option<String>>,    // Error message
    egui_ctx: Arc<OnceLock<eframe::egui::Context>>,
    is_connect: Arc<AtomicBool>,
}

impl ModbusClient {
    pub fn new(
        cmd_rx: mpsc::Receiver<ModbusCommand>,
        error_msg: watch::Sender<Option<String>>,
        config: Arc<RwLock<ModbusConfig>>,
        share_data: ShareDataRef,
        egui_ctx: Arc<OnceLock<eframe::egui::Context>>,
        is_connect: Arc<AtomicBool>,
    ) -> Self {
        Self { 
            cmd_rx: Some(cmd_rx),
            config: config,
            share_data: share_data,
            modbus_ctx: Arc::new(Mutex::new(None)),
            error_msg: error_msg,
            egui_ctx: egui_ctx,
            is_connect: is_connect,
         }
    }

    pub async fn start(
        &mut self,
    ) {
        
        let rx_handle= self.rx_handler();
        let read_data = self.read_data();
        let _ = tokio::join!(
            rx_handle,
            read_data,
        );
    }
    // 读取线程
    fn read_data(
        &self,
    ) -> tokio::task::JoinHandle<()> {
        let ctx_arc = self.modbus_ctx.clone();
        let share_data = self.share_data.clone();
        let config = self.config.clone();
        let is_connect = self.is_connect.clone();
        let egui_ctx = self.egui_ctx.clone();
        let error_msg = self.error_msg.clone();
        let mut err_current = 0;
        tokio::spawn(async move {
            loop {
                
                if ctx_arc.lock().await.is_none() {
                    tokio::time::sleep(std::time::Duration::from_millis(1000)).await;        //1000ms后重试
                    continue;
                }
                let poll_ms = config.read().expect("modbus config read 'poll_ms' lock error").poll_ms;
                let timeout = config.read().expect("modbus config read 'timeout' lock error").timeout;
                tokio::time::sleep(std::time::Duration::from_millis(poll_ms)).await;
                {
                    let mut guard = ctx_arc.lock().await;
                    if let Some(ref mut ctx) = guard.as_mut() {
                        let cnt = share_data.coils.load().len() as u16;
                        let offset = share_data.coils_offset.load(Ordering::Relaxed);
                        let result = match tokio::time::timeout(
                            Duration::from_millis(timeout),
                            chunked_read_bools(
                                ctx,
                                true,
                                offset,
                                cnt,
                            )
                        ).await {
                            Ok(Ok(result)) => {
                                if err_current == 1 {
                                    let _ = error_msg.send(None);
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 0;
                                }
                                result
                            }
                            Ok(Err(err)) => {
                                let msg = match err {
                                    ChunkedError::Business(_) => {
                                        format!("{}, coils", err)
                                    }
                                    ChunkedError::Transport(_) => {
                                        is_connect.store(false, Ordering::Relaxed);
                                        *guard = None;
                                        format!("{}, coils", err)
                                    }
                                };
                                if err_current == 0 {
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 1;
                                }
                                continue;
                            }
                            Err(_) => {
                                // timeout
                                let msg = "Modbus read coils timeout".to_string();
                                eprintln!("{}", msg);
                                if err_current == 0 {
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 1;
                                }
                                continue;
                            }
                        };
                        share_data.coils.store(Arc::new(result));

                        // read discrete_inputs
                        let cnt = share_data.discrete_inputs.load().len() as u16;
                        let offset = share_data.discrete_inputs_offset.load(Ordering::Relaxed);
                        let result = match tokio::time::timeout(
                            Duration::from_millis(timeout),
                            chunked_read_bools(
                                ctx,
                                false,
                                offset,
                                cnt
                            )
                        ).await {
                            Ok(Ok(result)) => {
                                if err_current == 2 {
                                    let _ = error_msg.send(None);
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 0;
                                }
                                result
                            }
                            Ok(Err(err)) => {
                                let msg = match err {
                                    ChunkedError::Business(_) => {
                                        format!("{}, discrete_inputs", err)
                                    }
                                    ChunkedError::Transport(_) => {
                                        is_connect.store(false, Ordering::Relaxed);
                                        *guard = None;
                                        format!("{}, discrete_inputs", err)
                                    }
                                };
                                if err_current == 0 {
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 2;
                                }
                                continue;

                            }
                            Err(_) => {
                                // timeout
                                let msg = "Modbus read discrete_input timeout".to_string();
                                eprintln!("{}", msg);
                                if err_current == 0 {
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                }
                                continue;
                            }
                        };
                        share_data.discrete_inputs.store(Arc::new(result));
                        
                        // read input_registers
                        let cnt = share_data.input_registers.load().len() as u16;
                        let offset = share_data.input_registers_offset.load(Ordering::Relaxed);
                        let result = match tokio::time::timeout(
                            Duration::from_millis(timeout),
                            chunked_read_words(
                                ctx,
                                true,
                                offset,
                                cnt,
                            )
                        ).await {
                            Ok(Ok(result)) => {
                                if err_current == 3 {
                                    let _ = error_msg.send(None);
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 0;
                                }
                                result
                            }
                            Ok(Err(err)) => {
                                let msg = match err {
                                    ChunkedError::Business(_) => {
                                        format!("{}, input_registers", err)
                                    }
                                    ChunkedError::Transport(_) => {
                                        is_connect.store(false, Ordering::Relaxed);
                                        *guard = None;
                                        format!("{}, input_registers", err)
                                    }
                                };
                                if err_current == 0 {
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 3;
                                }
                                continue;
                            }
                            Err(_) => {
                                // timeout
                                let msg = "Modbus read input_registers timeout".to_string();
                                eprintln!("{}", msg);
                                if err_current == 0 {
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 3;
                                }
                                continue;
                            }
                        };
                        share_data.input_registers.store(Arc::new(result));
                        
                        // read holding_registers
                        let cnt = share_data.holding_registers.load().len() as u16;
                        let offset = share_data.holding_registers_offset.load(Ordering::Relaxed);
                        let result = match tokio::time::timeout(
                            Duration::from_millis(timeout),
                            chunked_read_words(
                                ctx,
                                false,
                                offset,
                                cnt,
                            ),
                        ).await {
                            Ok(Ok(result)) => {
                                if err_current == 4 {
                                    let _ = error_msg.send(None);
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 0;
                                }
                                result
                            }
                            Ok(Err(err)) => {
                                let msg = match err {
                                    ChunkedError::Business(_) => {
                                        format!("{}, holding_registers", err)
                                    }
                                    ChunkedError::Transport(_) => {
                                        is_connect.store(false, Ordering::Relaxed);
                                        *guard = None;
                                        format!("{}, holding_registers", err)
                                    }
                                };
                                if err_current == 0 {
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 4;
                                }
                                continue;
                            }
                            Err(_) => {
                                // timeout
                                let msg = "Modbus read holding_registers timeout".to_string();
                                eprintln!("{}", msg);
                                if err_current == 0 {
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    err_current = 4;
                                }
                                continue;
                            }
                        };
                        share_data.holding_registers.store(Arc::new(result)); 
                    }
                }
                if err_current == 0 {
                    if let Some(egui_ctx) = egui_ctx.get() {
                        egui_ctx.request_repaint();
                    }
                }
                
            }
        })
    }


    fn rx_handler(
        &mut self,
    ) -> tokio::task::JoinHandle<()> {
        let mut cmd_rx = self.cmd_rx.take().expect("ModbusClient already started");
        let config = self.config.clone();
        let modbus_ctx = self.modbus_ctx.clone();
        let is_connect = self.is_connect.clone();
        let error_msg = self.error_msg.clone();
        let egui_ctx = self.egui_ctx.clone();
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                match cmd {
                    ModbusCommand::Disconnect => {
                        println!("ModbusClient Disconnect");
                        let mut guard = modbus_ctx.lock().await;
                        *guard = None;
                        is_connect.store(false, Ordering::Relaxed);
                        let _ = error_msg.send(None);
                        if let Some(egui_ctx) = egui_ctx.get() {
                            egui_ctx.request_repaint();
                        }
                    }
                    ModbusCommand::Connect => {
                        
                        let (protocol, tcp_config, rtu_config) = {
                            let config_guard = config.read().expect("modbus config read lock error");
                            (
                                config_guard.protocol.clone(),
                                config_guard.tcp_config.clone(),
                                config_guard.rtu_config.clone(),
                            )
                        };
                        match protocol {
                            Protocal::Tcp => {
                                match connect_tcp(tcp_config).await {
                                    Ok(ctx) => {
                                        let mut guard = modbus_ctx.lock().await;
                                        *guard = Some(ctx);
                                        is_connect.store(true, Ordering::Relaxed);
                                        let _ = error_msg.send(None);
                                        if let Some(egui_ctx) = egui_ctx.get() {
                                            egui_ctx.request_repaint();
                                        }
                                    }
                                    Err(err) => {
                                        println!("ModbusClient connect error: {}", err);
                                        let mut guard = modbus_ctx.lock().await;
                                        *guard = None;
                                        is_connect.store(false, Ordering::Relaxed);
                                        let _ = error_msg.send(Some(err.to_string()));
                                        if let Some(egui_ctx) = egui_ctx.get() {
                                            egui_ctx.request_repaint();
                                        }
                                    }
                                }
                            }
                            Protocal::Rtu => {
                                match connect_rtu(rtu_config).await {
                                    Ok(ctx) => {
                                        let mut guard = modbus_ctx.lock().await;
                                        *guard = Some(ctx);
                                        is_connect.store(true, Ordering::Relaxed);
                                        let _ = error_msg.send(None);
                                        if let Some(egui_ctx) = egui_ctx.get() {
                                            egui_ctx.request_repaint();
                                        }
                                    }
                                    Err(err) => {
                                        println!("ModbusClient connect error: {}", err);
                                        let mut guard = modbus_ctx.lock().await;
                                        *guard = None;
                                        is_connect.store(false, Ordering::Relaxed);
                                        let _ = error_msg.send(Some(err.to_string()));
                                        if let Some(egui_ctx) = egui_ctx.get() {
                                            egui_ctx.request_repaint();
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ModbusCommand::WriteCoil(address, value) => {
                        println!("ModbusClient WriteCoil address: {}, value: {}", address, value);
                        let timeout = {
                            let config_guard = config.read().expect("modbus config read lock error");
                            config_guard.timeout
                        };
                        let mut guard = modbus_ctx.lock().await;
                        if let Some(ref mut ctx) = guard.as_mut() {
                            match tokio::time::timeout(Duration::from_millis(timeout), ctx.write_single_coil(address, value)).await {
                                Ok(Ok(Ok(_))) => {
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                },
                                Ok(Ok(Err(err))) => {
                                    let msg = format!("Modbus write coil error: {}", err);
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    continue;
                                }
                                Ok(Err(err)) => {
                                    let msg = format!("ModbusClient write coil error: {}", err);
                                    eprintln!("{}", msg);
                                    *guard = None;
                                    is_connect.store(false, Ordering::Relaxed);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    continue;
                                }
                                Err(_) => {
                                    let msg = format!("ModbusClient write coil timeout");
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                }
                            }
                        } else {
                            let msg = "ModbusClient write coil error: not connected".to_string();
                            eprintln!("{}", msg);
                            let _ = error_msg.send(Some(msg));
                            if let Some(egui_ctx) = egui_ctx.get() {
                                egui_ctx.request_repaint();
                            }
                        }
                    }
                    ModbusCommand::WriteHoldRegister(address, value) => { 
                        let timeout = {
                            let config_guard = config.read().expect("modbus config read lock error");
                            config_guard.timeout
                        };
                        println!("ModbusClient WriteHoldRegister address: {}, value: {:?}", address, value);
                        let mut guard = modbus_ctx.lock().await;
                        if let Some(ref mut ctx) = guard.as_mut() {
                            match tokio::time::timeout(Duration::from_millis(timeout), ctx.write_multiple_registers(address, &value)).await {
                                Ok(Ok(Ok(_))) => {
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }  
                                }
                                Ok(Ok(Err(err))) => {
                                    let msg = format!("Modbus write hold register error: {}", err);
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    continue;
                                }
                                Ok(Err(err)) => {
                                    let msg = format!("ModbusClient write hold register error: {}", err);
                                    eprintln!("{}", msg);
                                    *guard = None;
                                    is_connect.store(false, Ordering::Relaxed);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                    continue;
                                }
                                Err(_) => {
                                    let msg = "modbus write hold register timeout".to_string();
                                    eprintln!("{}", msg);
                                    let _ = error_msg.send(Some(msg));
                                    if let Some(egui_ctx) = egui_ctx.get() {
                                        egui_ctx.request_repaint();
                                    }
                                }
                            }
                        } else {
                            let msg = "ModbusClient write hold register error: not connected".to_string();
                            eprintln!("{}", msg);
                            let _ = error_msg.send(Some(msg));
                            if let Some(egui_ctx) = egui_ctx.get() {
                                egui_ctx.request_repaint();
                            }

                        }
                    }
                }
            }
        })
    }
}


async fn connect_tcp(
    tcp_config: TcpConfig,
) -> anyhow::Result<Context> {
    let socket_addr = format!("{}:{}", tcp_config.ip, tcp_config.port).parse()?;
    let ctx = tcp::connect(socket_addr).await?;
    Ok(ctx)
}
async fn connect_rtu(
    rtu_config: RtuConfig,
) -> anyhow::Result<Context> {
    let com_port = rtu_config.port.ok_or_else(|| {
        anyhow::anyhow!("modbus rtu config error: com port is None")
    })?;
    let slave = Slave(rtu_config.slave);
    let parity = match rtu_config.parity {
        'E' | 'e' => tokio_serial::Parity::Even,
        'O' | 'o' => tokio_serial::Parity::Odd,
        _ => tokio_serial::Parity::None,
    };
    let builder = tokio_serial::new(com_port, rtu_config.baudrate)
        .parity(parity);
    let port = SerialStream::open(&builder)?;
    Ok(rtu::attach_slave(port, slave))
}

async fn chunked_read_bools(
    ctx: &mut Context,
    is_coils: bool,
    start: u16,
    cnt: u16,
) -> Result<Vec<bool>, ChunkedError> 
{
    let mut result = Vec::with_capacity(cnt as usize);
    let mut offset = start as usize;
    let end = offset + cnt as usize;
    while offset < end {
        let chunk = (end - offset).min(CHUNK_SIZE);
        let mut data = if is_coils {
            ctx.read_coils(offset as u16, chunk as u16).await??
        } else {
            ctx.read_discrete_inputs(offset as u16, chunk as u16).await??
        };
        result.append(&mut data);
        offset += chunk;
    }
    Ok(result)
}

async fn chunked_read_words(
    ctx: &mut Context,
    is_inreg: bool,
    start: u16,
    cnt: u16,
) -> Result<Vec<u16>, ChunkedError> { 
    let mut result = Vec::with_capacity(cnt as usize);
    let mut offset = start as usize;
    let end = offset + cnt as usize;
    while offset < end {
        let chunk = (end - offset).min(CHUNK_SIZE);
        let mut data = if is_inreg {
            ctx.read_input_registers(offset as u16, chunk as u16).await??
        } else {
            ctx.read_holding_registers(offset as u16, chunk as u16).await??
        };
        result.append(&mut data);
        offset += chunk;
    }
    Ok(result)
}
