//! Windows 后台应用自动静音工具
//! 主界面为 egui GUI,不需要控制台窗口
#![cfg_attr(windows, windows_subsystem = "windows")]

mod model;
mod policy;

#[cfg(windows)]
mod audio;
#[cfg(windows)]
mod foreground;
#[cfg(windows)]
mod monitor;
#[cfg(windows)]
mod process_name;
#[cfg(windows)]
mod tray;
#[cfg(windows)]
mod ui;
#[cfg(windows)]
mod windows_main;
#[cfg(windows)]
use windows_main::run;

#[cfg(not(windows))]
fn main() {
    println!("audio-mute-manager 是 Windows 应用,请在 Windows 上构建运行(cargo build --release)。");
}

#[cfg(windows)]
fn main() {
    if let Err(e) = run() {
        eprintln!("[audio-mute-manager] 致命错误: {e}");
        std::process::exit(1);
    }
}
