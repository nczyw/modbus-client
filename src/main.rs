
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use std::sync::{Arc, RwLock, atomic::AtomicBool};
use tokio::sync::watch;
use std::sync::OnceLock;
use clap::Parser;

mod modbus;
use crate::modbus::{share_data::ShareData, modbus_client::{ModbusClient, ModbusConfig}};

mod ui;
use crate::ui::app_ui::AppUi;

/// Adjust the UI display scale
#[derive(Parser, Debug)]
#[command(author, version, about)]
struct Args {
    /// UI display scale factor (e.g., 1.0, 1.5, 2.0)
    #[arg(short = 's', long = "scale", default_value_t = 1.0)]
    scale: f32,
}


fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let is_connect = Arc::new(AtomicBool::new(false));
    let egui_ctx = Arc::new(OnceLock::<eframe::egui::Context>::new());
    let share_data = Arc::new(ShareData::new());
    let modbus_config = Arc::new(RwLock::new(ModbusConfig::new()));
    let (error_tx, error_rx) = watch::channel(None);
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(100);
    let mut modbus_client = ModbusClient::new(
        cmd_rx,
        error_tx,
        modbus_config.clone(), 
        share_data.clone(),
        egui_ctx.clone(),
        is_connect.clone(),
    );
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async move {
            modbus_client.start().await;
        });
    });
    let nativeoptions = eframe::NativeOptions {
        run_and_return: true,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title(format!("Modbus Client  v{}", env!("CARGO_PKG_VERSION"))),
            ..eframe::NativeOptions::default()
    };

    eframe::run_native(
        "Modbus Client",
        nativeoptions,
        Box::new(|cc| {
            let fonts = load_fonts_from_dir("fonts");
            cc.egui_ctx.set_fonts(fonts);
            {
                cc.egui_ctx.set_theme(eframe::egui::Theme::Dark);
                egui_ctx.set(cc.egui_ctx.clone()).ok();
            }
            Ok(Box::new(AppUi::new(
                share_data, 
                modbus_config,
                args.scale,
                error_rx,
                cmd_tx,
                is_connect,
            )))
        }),
    )
}

fn load_fonts_from_dir(dir: &str) -> eframe::egui::FontDefinitions {
    let mut fonts = eframe::egui::FontDefinitions::default();
    let default_font = include_bytes!("../fonts/AlibabaPuHuiTi-3-55-Regular.ttf");
    let emoji_font = include_bytes!("../fonts/NotoEmoji-VariableFont_wght.ttf");
    fonts.font_data.insert("AlibabaPuHuiTi".to_owned(), Arc::new(eframe::egui::FontData::from_static(default_font)));
    fonts.font_data.insert("NotoEmoji".to_owned(), Arc::new(eframe::egui::FontData::from_static(emoji_font)));
    
    let mut family_order = Vec::new();
    
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries {
            if let Ok(entry) = entry {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "ttf" || ext == "otf" {
                        let font_name = path.file_stem().unwrap().to_string_lossy().to_string();
                        if font_name == "AlibabaPuHuiTi-3-55-Regular" {
                            continue;
                        }
                        let data = std::fs::read(&path).unwrap();
                        fonts.font_data.insert(font_name.clone(), Arc::new(eframe::egui::FontData::from_owned(data)));
                        family_order.push(font_name);
                    }
                }
            }
        }
    }
    
    family_order.sort();
    
    let proportional = fonts.families.get_mut(&eframe::egui::FontFamily::Proportional).unwrap();
    proportional.insert(1, "NotoEmoji".to_owned());
    proportional.insert(0, "AlibabaPuHuiTi".to_owned());
    
    for name in family_order {
        proportional.push(name);
    }
    
    fonts
}