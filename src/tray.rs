//! 系统托盘(Windows):常驻托盘图标,菜单含 显示/刷新/退出。
//! 菜单事件通过 channel 转发给 UI 线程处理(不直接操作窗口)。

use std::sync::mpsc::Sender;
use std::sync::OnceLock;

use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

use crate::ui::TrayCmd;

static TRAY_TX: OnceLock<Sender<TrayCmd>> = OnceLock::new();

/// 生成一个简单的喇叭图标(32x32 RGBA,image crate 绘制)
fn build_icon() -> Icon {
    let size = 32usize;
    let mut img = image::RgbaImage::new(size as u32, size as u32);
    // 背景透明
    // 画一个简单的喇叭:蓝色圆角方块 + 白色三角
    for y in 0..size {
        for x in 0..size {
            let px = image::Rgba([0u8, 0u8, 0u8, 0u8]);
            img.put_pixel(x as u32, y as u32, px);
        }
    }
    // 喇叭主体(蓝色)
    for y in 8..20 {
        for x in 6..16 {
            img.put_pixel(x as u32, y as u32, image::Rgba([33, 150, 243, 255]));
        }
    }
    // 喇叭嘴(三角)
    for i in 0..8 {
        for y in (10 + i)..(18 - i) {
            let x = 16 + i;
            if x < size && y < size {
                img.put_pixel(x as u32, y as u32, image::Rgba([33, 150, 243, 255]));
            }
        }
    }
    let rgba = img.into_raw();
    Icon::from_rgba(rgba, size as u32, size as u32).expect("build tray icon")
}

/// 创建托盘图标与菜单。
/// 菜单事件会通过 `tx` 发送 TrayCmd 到 UI 线程。
pub fn setup_tray(tx: Sender<TrayCmd>) -> Result<TrayIcon, Box<dyn std::error::Error>> {
    let _ = TRAY_TX.set(tx.clone());

    let show_item = MenuItem::new("显示主窗口", true, None);
    let refresh_item = MenuItem::new("刷新列表", true, None);
    let quit_item = MenuItem::new("退出(恢复所有音量)", true, None);

    // 提前取出菜单项 id 的字符串副本(闭包要求 Send,不能捕获 Rc<MenuItem>)
    let show_id = show_item.id().0.clone();
    let refresh_id = refresh_item.id().0.clone();
    let quit_id = quit_item.id().0.clone();

    let menu = Menu::new();
    menu.append(&show_item)?;
    menu.append(&refresh_item)?;
    menu.append(&quit_item)?;

    // 菜单事件 -> channel
    tray_icon::menu::MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
        if let Some(tx) = TRAY_TX.get() {
            let id = &event.id.0;
            if id == &show_id {
                let _ = tx.send(TrayCmd::Show);
            } else if id == &refresh_id {
                let _ = tx.send(TrayCmd::Refresh);
            } else if id == &quit_id {
                let _ = tx.send(TrayCmd::Quit);
            }
        }
    }));

    // 托盘图标点击 -> 显示窗口
    tray_icon::TrayIconEvent::set_event_handler(Some(|_event: tray_icon::TrayIconEvent| {
        if let Some(tx) = TRAY_TX.get() {
            let _ = tx.send(TrayCmd::Show);
        }
    }));

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Audio Mute Manager - 后台应用自动静音")
        .with_icon(build_icon())
        .build()?;

    Ok(tray)
}
