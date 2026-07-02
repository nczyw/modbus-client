use core::fmt;
use eframe::egui;
use tokio::sync::watch;
use egui_extras::{TableBuilder, Column};
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;

use tokio_serial::available_ports;

use crate::modbus::modbus_client::Protocal;
use crate::modbus::{
    modbus_client::{
        ModbusCommand, 
        ModbusConfigRef,
    }, 
    share_data::{
        ShareDataRef, 
        RegType, 
        DisplayFormat,
    }
};

#[derive(Debug, Clone, Copy)]
enum DialogTarget {
    InputRegister(usize),
    HoldingRegister(usize),
}

impl fmt::Display for DialogTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DialogTarget::InputRegister(i) => write!(f, "InputRegister({})", i),
            DialogTarget::HoldingRegister(i) => write!(f, "HoldingRegister({})", i),
        }
    }
}

/// Open the display format modification dialog
struct DialogFormat {
    target: DialogTarget,
    temp_reg_type: RegType,
    temp_display_format: DisplayFormat,
    open_pos: egui::Pos2,
}

/// Edit value dialog
struct DialogValueEdit {
    target: DialogTarget,
    temp_value: String,
    open_pos: egui::Pos2,
    is_first_open: bool,          
}

pub struct AppUi {
    scale_factor: f32,          // Scale factor
    skin_dark: bool,            // Dark skin

    share_data: ShareDataRef,   // Shared data
    modbus_config: ModbusConfigRef,
    
    max_len: usize,             // Max data size
    last_table_width : f32,     // Previous table width, used to check if a reset is needed
    
    dialog_format: Option<DialogFormat>,    // Modify display format dialog
    dialog_value_edit: Option<DialogValueEdit>,   // Modify value dialog

    error_rx: watch::Receiver<Option<String>>,  // Receive error message
    cmd_tx: mpsc::Sender<ModbusCommand>,
    is_connect: Arc<AtomicBool>,

    serial_ports: Vec<String>,          // 串口列表
    serial_baudrate: Vec<u32>,       // 波特率列表
    serial_parity: Vec<char>,         // 校验
}

impl AppUi {
    pub fn new(
        share_data: ShareDataRef,
        modbus_config: ModbusConfigRef,
        scale_factor: f32,
        error_rx: watch::Receiver<Option<String>>,
        cmd_tx: mpsc::Sender<ModbusCommand>,
        is_connect: Arc<AtomicBool>,
    ) -> Self {
        let max_len = (share_data.coils.load().len() as usize)
            .max(share_data.discrete_inputs.load().len() as usize)
            .max(share_data.input_registers.load().len() as usize)
            .max(share_data.holding_registers.load().len() as usize);

        let serial_ports = available_ports()
            .unwrap()
            .into_iter()
            .map(|p| p.port_name)
            .collect::<Vec<_>>();
        Self {
            scale_factor: scale_factor,
            skin_dark: true,
            share_data: share_data,
            modbus_config: modbus_config,
            max_len: max_len,
            last_table_width: 0.0,
            dialog_format: None,
            dialog_value_edit: None,
            error_rx: error_rx,
            cmd_tx: cmd_tx,
            is_connect: is_connect,
            serial_ports: serial_ports,
            serial_baudrate: vec![
                9600,
                19200,
                38400,
                57600,
                115200,
                230400,
                460800,
                921600,
            ],
            serial_parity: vec![
                'N',
                'O',
                'E',
            ]
        }
    }

    fn apply_dialog_type_format(
        &self, 
        dialog: &DialogFormat, 
    ) {
        let data = self.share_data.as_ref();
        match dialog.target {
            DialogTarget::InputRegister(i) => {
                data.write_reg_type(true, i, dialog.temp_reg_type);
                data.write_display_format(true, i, dialog.temp_display_format);
            }
            DialogTarget::HoldingRegister(i) => {
                data.write_reg_type(false, i, dialog.temp_reg_type);
                data.write_display_format(false, i, dialog.temp_display_format);
            }
        }
        
    }

    fn apply_dialog_value_edit(
        &self,
        dialog: &DialogValueEdit,
    ) {
        let data = self.share_data.as_ref();
        match dialog.target {
            DialogTarget::HoldingRegister(i) => {
                if let Some(value) = data.parse_string_to_dvalue(i, &dialog.temp_value) {
                    let cmd = ModbusCommand::WriteHoldRegister(i as u16, value);
                    let _ = self.cmd_tx.blocking_send(cmd);
                }
            }
            _ => {}
        }
    }

    fn refresh_serial_ports(
        & self,
    ) -> Vec<String> {
        let serial_ports = available_ports()
            .unwrap()
            .into_iter()
            .map(|p| p.port_name)
            .collect::<Vec<_>>();
        serial_ports
    }
}

impl eframe::App for AppUi {
    fn ui(
        &mut self, 
        ui: &mut eframe::egui::Ui, 
        _frame: &mut eframe::Frame
    ) {
        //println!("refresh ui - Frame: {}", ui.ctx().input(|i| i.time)); 
        egui::Panel::bottom("status_bar")
            .resizable(false)
            .show_separator_line(true)
            .max_size(24.0)
            .show(ui, |ui| {
                let is_running = self.is_connect.load(Ordering::Relaxed);
                let status_text = if is_running { "Disconnect" } else { "Connect" };
                let dot_color = if is_running {
                    egui::Color32::from_rgb(0, 180, 0)   // Dark green
                } else {
                    egui::Color32::from_rgb(180, 0, 0)   // Dark red
                };
                
                ui.horizontal(|ui| {
                    ui.colored_label(dot_color, "●");
                    if ui.add(
                        egui::Button::new(status_text).fill(egui::Color32::TRANSPARENT)
                    ).clicked() {
                        self.max_len = (self.share_data.coils.load().len() as usize)
                            .max(self.share_data.discrete_inputs.load().len() as usize)
                            .max(self.share_data.input_registers.load().len() as usize)
                            .max(self.share_data.holding_registers.load().len() as usize);
                        let command = match is_running {
                            true => ModbusCommand::Disconnect,
                            false => ModbusCommand::Connect,
                        };
                        if let Err(e) = self.cmd_tx.blocking_send(command) {
                            eprintln!("Error sending command: {:?}", e)
                        }

                    }
                    let mut word_swap = self.share_data.word_swap.load(Ordering::Relaxed);
                    if ui.checkbox(&mut word_swap, "word-swap").changed() {
                        self.share_data.word_swap.store(word_swap, Ordering::Relaxed);
                    }
                    let mut byte_swap = self.share_data.byte_swap.load(Ordering::Relaxed);
                    if ui.checkbox(&mut byte_swap, "byte-swap").changed() {
                        self.share_data.byte_swap.store(byte_swap, Ordering::Relaxed);
                    }
                    
                    ui.add(egui::Separator::default().vertical());
                    if self.error_rx.has_changed().unwrap_or(false) {
                        if let Some(err) = self.error_rx.borrow().clone() {
                            ui.colored_label(egui::Color32::RED, err);
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let theme_text = if self.skin_dark { "🌙" } else { "☀️" };
                        if ui.button(theme_text).clicked() {
                            self.skin_dark = !self.skin_dark; 
                            if self.skin_dark {
                                ui.ctx().set_theme(eframe::egui::Theme::Dark);
                            } else {
                                ui.ctx().set_theme(eframe::egui::Theme::Light);
                            }
                        }
                        
                        let fps = ui.ctx().input(|i| i.unstable_dt) * 1000.0;
                        ui.label(format!("{:.1}ms", fps));

                        /* 
                        if self.display_fps {
                            let fps = ui.ctx().input(|i| i.unstable_dt) * 1000.0;
                            ui.label(format!("{:.1}ms", fps));
                            self.display_fps = false;
                        } else {
                            self.display_fps = true;
                        }
                        */
                    });
                });
        });
        
        egui::CentralPanel::default().show(ui, |ui| {

            //let share_data = self.share_data;

            let coils = self.share_data.coils.load();   
            let coils_offset = self.share_data.coils_offset.load(Ordering::Relaxed);

            let discrete_inputs = self.share_data.discrete_inputs.load();
            let discrete_inputs_offset = self.share_data.discrete_inputs_offset.load(Ordering::Relaxed);

            

            let input_registers = self.share_data.input_registers.load();
            let input_registers_offset = self.share_data.input_registers_offset.load(Ordering::Relaxed);

            let holding_registers = self.share_data.holding_registers.load();
            let holding_registers_offset = self.share_data.holding_registers_offset.load(Ordering::Relaxed);

            ui.vertical_centered(|ui| {
                ui.heading("Modbus Client");
            });
            ui.add_space(10.0);
            egui::ScrollArea::both().auto_shrink([true; 2]).show(ui, |ui| {
                ui.horizontal(|ui| {
                    let mut modbus_config = self.modbus_config.write().unwrap();
                    ui.label("Type:");
                    egui::ComboBox::from_id_salt("connect_type")
                        .selected_text(modbus_config.protocol.as_str())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut modbus_config.protocol,
                                Protocal::Tcp,
                                "TCP"
                            );
                            ui.selectable_value(
                                &mut modbus_config.protocol,
                                Protocal::Rtu,
                                "RTU"
                            );
                        });
                        match &modbus_config.protocol {
                            Protocal::Tcp => {
                                ui.add(egui::Label::new("IP:"));
                                ui.add(
                                    egui::TextEdit::singleline(&mut modbus_config.tcp_config.ip).desired_width(150.0)
                                );
                                ui.add(
                                    egui::DragValue::new(&mut modbus_config.tcp_config.port)
                                        .range(1..=65535)
                                        .speed(0)
                                        .prefix("Port: ")
                                );
                            }
                            Protocal::Rtu => { 
                                ui.add(egui::Label::new("Name:"));
                                let refresh_serial = egui::ComboBox::from_id_salt("serial_port_list")
                                    .selected_text(modbus_config.rtu_config.port.as_deref().unwrap_or("Select"))
                                    .show_ui(ui, |ui| {
                                        for port in &self.serial_ports {
                                            ui.selectable_value(
                                                &mut modbus_config.rtu_config.port,
                                                Some(port.clone()),
                                                port
                                            );
                                        }
                                    })
                                    .response;

                                if refresh_serial.clicked() {
                                    self.serial_ports = self.refresh_serial_ports();
                                }

                                // baudrate
                                ui.add(egui::Label::new("Baudrate:"));
                                egui::ComboBox::from_id_salt("serial_port_baudrate")
                                    .selected_text(modbus_config.rtu_config.baudrate.to_string())
                                    .show_ui(ui, |ui| {
                                        for &baudrate in &self.serial_baudrate {
                                            ui.selectable_value(
                                                &mut modbus_config.rtu_config.baudrate,
                                                baudrate,
                                                baudrate.to_string()
                                            );
                                        }
                                    });

                                // parity
                                ui.add(egui::Label::new("Parity:"));
                                egui::ComboBox::from_id_salt("serial_port_parity")
                                    .selected_text(modbus_config.rtu_config.parity.to_string())
                                    .show_ui(ui, |ui| {
                                        for &parity in &self.serial_parity {
                                            ui.selectable_value(
                                                &mut modbus_config.rtu_config.parity,
                                                parity,
                                                 parity.to_string()
                                            );
                                        }
                                    });

                                // slave
                                ui.add(
                                    egui::DragValue::new(&mut modbus_config.rtu_config.slave)
                                        .range(1..=255)
                                        .speed(0)
                                        .prefix("Slave: ")
                                );
                            
                            }
                        }
                        ui.separator();
                        let mut poll_ms = modbus_config.poll_ms;
                        let response = ui.add(
                            egui::DragValue::new(&mut poll_ms)
                                .range(1..=10000)
                                .speed(0)
                                .prefix("Polling: ")
                                .suffix(" ms")
                        );
                        if response.changed() && poll_ms != modbus_config.poll_ms {
                            modbus_config.poll_ms = poll_ms;
                        }

                        let mut timeout = modbus_config.timeout;
                        let response = ui.add(
                            egui::DragValue::new(&mut timeout)
                                .range(1..=10000)
                                .speed(0)
                                .prefix("Timeout: ")
                                .suffix(" ms")
                        );
                        if response.changed() && response.lost_focus() {
                            modbus_config.timeout = timeout;
                        }
                });
                ui.separator();
                let current_width = ui.available_width();
                let table = TableBuilder::new(ui)
                    .id_salt("table")
                    .auto_shrink([true; 2])
                    .column(Column::exact(100.0))
                    .column(Column::exact(150.0))
                    .column(Column::remainder().at_least(120.0).resizable(true))
                    .column(Column::remainder().at_least(120.0));
                if (current_width - self.last_table_width).abs() > 0.01 {
                    //println!("{}", current_width - self.last_table_width);
                    table.reset();
                }
                self.last_table_width = current_width;
                
                table.header(50.0, |mut row| {
                    row.col(|ui| { 
                        let mut tmp = coils.len();
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Coils: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.resize_coils(tmp as u16);
                            let coils = self.share_data.coils.load();
                            self.max_len = (coils.len() as usize)
                                .max(discrete_inputs.len() as usize)
                                .max(input_registers.len() as usize)
                                .max(holding_registers.len() as usize);
                        }
                        tmp = coils_offset as usize;
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Offset: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.coils_offset.store(tmp as u16, Ordering::Relaxed);
                        }
                    });
                    row.col(|ui| { 
                        let mut tmp = discrete_inputs.len();
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Discrete Inputs: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.resize_discrete_inputs(tmp as u16);
                            let discrete_inputs = self.share_data.discrete_inputs.load();
                            self.max_len = (coils.len() as usize)
                                .max(discrete_inputs.len() as usize)
                                .max(input_registers.len() as usize)
                                .max(holding_registers.len() as usize);
                        }
                        tmp = discrete_inputs_offset as usize;
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Offset: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.discrete_inputs_offset.store(tmp as u16, Ordering::Relaxed);
                        }
                    });
                    row.col(|ui| { 
                        let mut tmp = input_registers.len();
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Input Registers: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.resize_input_registers(tmp as u16);
                            let input_registers = self.share_data.input_registers.load();
                            self.max_len = (coils.len() as usize)
                                .max(discrete_inputs.len() as usize)
                                .max(input_registers.len() as usize)
                                .max(holding_registers.len() as usize);
                        }
                        tmp = input_registers_offset as usize;
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Offset: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.input_registers_offset.store(tmp as u16, Ordering::Relaxed);
                        }
                    });
                    row.col(|ui| { 
                        let mut tmp = holding_registers.len();
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Holding Registers: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.resize_holding_registers(tmp as u16);
                            let holding_registers = self.share_data.holding_registers.load();
                            self.max_len = (coils.len() as usize)
                                .max(discrete_inputs.len() as usize)
                                .max(input_registers.len() as usize)
                                .max(holding_registers.len() as usize);
                        }
                        tmp = holding_registers_offset as usize;
                        let response = ui.add(
                            egui::DragValue::new(&mut tmp)
                                .range(0..=65535)
                                .speed(0)
                                .prefix("Offset: ")
                        );
                        if response.changed() && response.lost_focus() {
                            self.share_data.holding_registers_offset.store(tmp as u16, Ordering::Relaxed);
                        }
                    });
                })
                .body(|body| {
                    body.rows(20.0, self.max_len, |mut row| {
                        let i = row.index();
                        row.col(|ui| {
                            let coils_index = i + coils_offset as usize;
                            if let Some(val) = coils.get(i) {
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM), |ui| {
                                    ui.horizontal(|ui| {
                                        let mut tmp = *val;
                                        let response = ui.add(
                                            egui::Checkbox::new(&mut tmp, "")
                                        );
                                        let text = egui::RichText::new(format!("{:<5}", coils_index)).monospace();
                                        ui.label(text);
                                        if response.changed() {
                                            let cmd = ModbusCommand::WriteCoil(coils_index as u16, tmp);
                                            if let Err(e) = self.cmd_tx.blocking_send(cmd) {
                                                eprintln!("Error sending command: {:?}", e);
                                            }
                                        }
                                    });
                                });
                            }
                        });

                        row.col(|ui| {
                            let discrete_inputs_index = i + discrete_inputs_offset as usize;
                            if let Some(val) = discrete_inputs.get(i) {
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM), |ui| {
                                    ui.horizontal(|ui| {
                                        let mut tmp = *val;
                                         ui.add_enabled(
                                            false,
                                            egui::Checkbox::new(
                                                &mut tmp, "",
                                            ),
                                        );
                                        let text = egui::RichText::new(format!("{:<5}", discrete_inputs_index)).monospace();
                                        ui.label(text);
                                    });
                                });
                            }
                        });
                        row.col(|ui| { 
                            let reg_config = self.share_data.input_registers_config
                                .load()
                                .get(i)
                                .and_then(|v| *v);
                            let fmt = self.share_data.input_registers_display_format
                                .load()
                                .get(i)
                                .and_then(|v| *v);

                            let input_registers_index = i + input_registers_offset as usize;
                            if let Some(val) = self.share_data.read_registers(true, i) {
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM), |ui| {
                                    ui.horizontal(|ui| {
                                        let text = egui::RichText::new(format!("{:5}", input_registers_index)).monospace();
                                        ui.label(text);
                                        let resp = ui.add_sized(
                                            [
                                                ui.available_width().floor(), 
                                                ui.spacing().interact_size.y,
                                            ],
                                            egui::Button::new(&val),
                                        );
                                        if resp.secondary_clicked() {
                                            let pos = self.dialog_format
                                                .as_ref()
                                                .map(|d| d.open_pos)
                                                .unwrap_or(resp.rect.center());
                                            self.dialog_format = Some(
                                                DialogFormat {
                                                    target: DialogTarget::InputRegister(i),
                                                    temp_reg_type: reg_config.unwrap_or(RegType::I16),
                                                    temp_display_format: fmt.unwrap(),
                                                    open_pos: pos,
                                                }
                                            )
                                        }
                                    });
                                });
                            }
                        });

                        row.col(|ui| {
                            let reg_config = self.share_data.holding_registers_config
                                .load()
                                .get(i)
                                .and_then(|v| *v);
                            let fmt = self.share_data.holding_registers_display_format
                                .load()
                                .get(i)
                                .and_then(|v| *v);

                            let holding_registers_index = i + holding_registers_offset as usize;
                            if let Some(val) = self.share_data.read_registers(false, i) { 
                                ui.with_layout(egui::Layout::left_to_right(egui::Align::BOTTOM), |ui| {
                                    ui.horizontal(|ui| {
                                        let text = egui::RichText::new(format!("{:5}", holding_registers_index)).monospace();
                                        ui.label(text);
                                        let resp = ui.add_sized(
                                            [
                                                ui.available_width().floor(),
                                                ui.spacing().interact_size.y
                                            ],
                                            egui::Button::new(&val),
                                        );
                                        if resp.clicked() {
                                            self.dialog_value_edit = Some(
                                                DialogValueEdit {
                                                    target: DialogTarget::HoldingRegister(i),
                                                    temp_value: val,
                                                    open_pos: resp.rect.center(),
                                                    is_first_open: true,
                                                }
                                            )
                                        }
                                        if resp.secondary_clicked() {
                                            let pos = self.dialog_format
                                                .as_ref()
                                                .map(|d| d.open_pos)
                                                .unwrap_or(resp.rect.center());
                                            self.dialog_format = Some(
                                                DialogFormat {
                                                    target: DialogTarget::HoldingRegister(i),
                                                    temp_reg_type: reg_config.unwrap_or(RegType::I16),
                                                    temp_display_format: fmt.unwrap(),
                                                    open_pos: pos,
                                                }
                                            )
                                        }
                                    });
                                });
                            }

                        });
                    });
                });
            });
            
           
        });
        if let Some(mut dialog) = self.dialog_format.take() {

            let mut should_close = false;
            let mut should_apply = false;
            // Fullscreen overlay
            let screen_rect = ui.ctx().content_rect();

            let bg_response = egui::Area::new(egui::Id::new("dialog_mask"))
                .order(egui::Order::Background)
                .fixed_pos(screen_rect.min)
                .show(ui.ctx(), |ui| {
                
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        screen_rect.size(),
                    );
                
                    // Semi-transparent background (optional)
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        egui::Color32::from_black_alpha(64),
                    );
                
                    ui.allocate_rect(
                        rect,
                        egui::Sense::click(),
                    )
                })
                .inner;
            
            // Window
            
            let mut window_rect = None;
            let window_title = format!("{} Settings", dialog.target);
            let response = egui::Window::new(window_title)
                .id(egui::Id::new("reg_dialog"))
                .resizable(false)
                .movable(true)
                .default_pos(dialog.open_pos)
                .show(ui.ctx(), |ui| {
            
                    ui.label("Data Type:");
                
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::U16, "U16");
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::I16, "I16");
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::U32, "U32");
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::I32, "I32");
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::F32, "F32");
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::U64, "U64");
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::I64, "I64");
                        ui.selectable_value(&mut dialog.temp_reg_type, RegType::F64, "F64");
                    });
                
                    ui.separator();
                
                    // ================= Format =================
                
                    ui.label("Display Format:");
                
                    ui.horizontal(|ui| {
                        ui.selectable_value(
                            &mut dialog.temp_display_format,
                            DisplayFormat::Decimal,
                            "DEC",
                        );
                    
                        ui.selectable_value(
                            &mut dialog.temp_display_format,
                            DisplayFormat::Hexadecimal,
                            "HEX",
                        );
                    
                        ui.selectable_value(
                            &mut dialog.temp_display_format,
                            DisplayFormat::Binary,
                            "BIN",
                        );
                    
                        ui.selectable_value(
                            &mut dialog.temp_display_format,
                            DisplayFormat::Octal,
                            "OCT",
                        );
                    });
                
                    ui.separator();
                
                    ui.horizontal(|ui| {
                    
                        if ui.button("OK").clicked() {
                            should_apply = true;
                            should_close = true;
                        }
                    
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });
                    // Press Enter to click OK
                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        should_apply = true;
                        should_close = true;
                    }
                    // Press ESC to click Cancel
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        should_close = true;
                    }
                });
            
            // Window position
            
            if let Some(resp) = response {
                window_rect = Some(resp.response.rect);
            
                // Remember position after dragging
                dialog.open_pos = resp.response.rect.min;
            }
        
            // Close on overlay click
        
            if bg_response.clicked() {
            
                let mouse_pos =
                    ui.ctx().input(|i| i.pointer.interact_pos());
            
                if let (Some(rect), Some(pos)) =
                    (window_rect, mouse_pos)
                {
                    if !rect.contains(pos) {
                        should_close = true;
                    }
                }
            }
        
            // Apply
            if should_apply {
                self.apply_dialog_type_format(&dialog);
            }
        
            // Save current state
            if !should_close {
                self.dialog_format = Some(dialog);
            }
        }

        // Modify value
        if let Some(mut value_dialog) = self.dialog_value_edit.take() {
            
            
            let mut should_close = false;
            let mut should_apply =false;

            let screen_rect = ui.ctx().content_rect();

            let bg_response = egui::Area::new(egui::Id::new("dialog_mask"))
                .order(egui::Order::Background)
                .fixed_pos(screen_rect.min)
                .show(ui.ctx(), |ui| {
                
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        screen_rect.size(),
                    );
                
                    
                    ui.painter().rect_filled(
                        rect,
                        0.0,
                        egui::Color32::from_black_alpha(64),
                    );
                
                    ui.allocate_rect(
                        rect,
                        egui::Sense::click(),
                    )
                })
                .inner;
            
            let mut window_rect = None;

            let window_title = format!("Edit {}", value_dialog.target);

            let response = egui::Window::new(window_title)
                .id(egui::Id::new("value_edit_dialog"))
                .resizable(false)
                .default_pos(value_dialog.open_pos)
                .show(ui.ctx(), |ui| {
                    ui.label("New Value:");
                    let edit_text = ui.text_edit_singleline(&mut value_dialog.temp_value);
                    if value_dialog.is_first_open {
                        value_dialog.is_first_open = false;
                        edit_text.request_focus();

                    }

                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        should_apply = true;
                        should_close = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        should_close = true;
                    }
                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button("OK").clicked() {
                            should_apply = true;
                            should_close = true;
                        }
                        if ui.button("Cancel").clicked() {
                            should_close = true;
                        }
                    });

                    
                    
                });
                
            if let Some(resp) = response {
                window_rect = Some(resp.response.rect);
            
                
                value_dialog.open_pos = resp.response.rect.min;
            }
        
           
        
            if bg_response.clicked() {
            
                let mouse_pos =
                    ui.ctx().input(|i| i.pointer.interact_pos());
            
                if let (Some(rect), Some(pos)) =
                    (window_rect, mouse_pos)
                {
                    if !rect.contains(pos) {
                        should_close = true;
                    }
                }
            }


            if should_apply {
                self.apply_dialog_value_edit(&value_dialog);
            }
            if !should_close {
                self.dialog_value_edit = Some(value_dialog);
            }
        }
    }
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let target_scale = ctx.pixels_per_point() * self.scale_factor;
        if (ctx.pixels_per_point() - target_scale).abs() > 0.01 {
            ctx.set_pixels_per_point(self.scale_factor);
        }
    }
    fn on_exit(&mut self) {
       let _ = self.cmd_tx.blocking_send(ModbusCommand::Disconnect);
    }
}