//! Windows 入口:组装 监控线程 + 托盘 + egui 主界面

use std::sync::mpsc;

use crate::model::UiCmd;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // 命令/事件通道:UI <-> monitor
    let (cmd_tx, cmd_rx) = mpsc::channel::<UiCmd>();
    let (evt_tx, evt_rx) = mpsc::channel();

    // 启动监控线程(内部:COM + 音频 + 前台钩子 + 策略执行)
    let _monitor = crate::monitor::spawn_monitor(cmd_rx, evt_tx);

    // 托盘(事件转发到 UI 线程)
    let (tray_tx, tray_rx) = mpsc::channel();
    let _tray = crate::tray::setup_tray(tray_tx)?;

    // egui 主界面
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([760.0, 520.0])
            .with_min_inner_size([560.0, 360.0])
            .with_title("Audio Mute Manager - 后台应用自动静音"),
        ..Default::default()
    };

    eframe::run_native(
        "Audio Mute Manager",
        options,
        Box::new(move |cc| Ok(Box::new(crate::ui::MuteApp::new(cc, cmd_tx, evt_rx, tray_rx)))),
    )?;

    Ok(())
}
