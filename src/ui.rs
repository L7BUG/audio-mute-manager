//! egui 主界面:应用列表 + 勾选 + 启动/停止 + 状态显示
//!
//! 注意:eframe 0.36 的 App trait 方法为 `fn ui(&mut self, ui: &mut egui::Ui, frame)`,
//! 面板 API 统一为 `egui::containers::Panel`。

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use eframe::egui;

use crate::model::{SessionInfo, UiCmd, UiEvent};

/// 托盘命令(由 tray.rs 发送,UI 线程消费)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayCmd {
    Show,
    Quit,
    Refresh,
}

pub struct MuteApp {
    sessions: Vec<SessionInfo>,
    selected: HashSet<u32>,
    monitoring: bool,
    foreground: Option<u32>,
    error: Option<String>,
    quitting: bool,
    cmd_tx: Sender<UiCmd>,
    event_rx: Receiver<UiEvent>,
    tray_rx: Receiver<TrayCmd>,
    last_refresh: std::time::Instant,
}

impl MuteApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        cmd_tx: Sender<UiCmd>,
        event_rx: Receiver<UiEvent>,
        tray_rx: Receiver<TrayCmd>,
    ) -> Self {
        let _ = cmd_tx.send(UiCmd::Refresh);
        Self {
            sessions: Vec::new(),
            selected: HashSet::new(),
            monitoring: false,
            foreground: None,
            error: None,
            quitting: false,
            cmd_tx,
            event_rx,
            tray_rx,
            last_refresh: std::time::Instant::now(),
        }
    }

    fn pump_events(&mut self) {
        while let Ok(ev) = self.event_rx.try_recv() {
            match ev {
                UiEvent::Sessions(list) => {
                    self.sessions = list;
                    // 清理已不存在的会话勾选
                    let alive: HashSet<u32> = self.sessions.iter().map(|s| s.pid).collect();
                    self.selected.retain(|p| alive.contains(p));
                }
                UiEvent::Foreground(pid) => self.foreground = pid,
                UiEvent::Monitoring(on) => self.monitoring = on,
                UiEvent::Error(msg) => self.error = Some(msg),
            }
        }
        while let Ok(cmd) = self.tray_rx.try_recv() {
            match cmd {
                TrayCmd::Show => {
                    let _ = self.cmd_tx.send(UiCmd::Refresh);
                }
                TrayCmd::Quit => {
                    self.quitting = true;
                    let _ = self.cmd_tx.send(UiCmd::Quit);
                }
                TrayCmd::Refresh => {
                    let _ = self.cmd_tx.send(UiCmd::Refresh);
                }
            }
        }
    }

    fn send_selection(&self) {
        let pids: Vec<u32> = self.selected.iter().copied().collect();
        let _ = self.cmd_tx.send(UiCmd::SetSelection(pids));
    }
}

impl eframe::App for MuteApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.pump_events();

        // 关闭窗口 -> 隐藏到托盘(除非正在退出)
        if ctx.input(|i| i.viewport().close_requested()) && !self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }
        if self.quitting {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // 每 2 秒自动刷新会话列表
        if self.last_refresh.elapsed() >= Duration::from_secs(2) {
            self.last_refresh = std::time::Instant::now();
            let _ = self.cmd_tx.send(UiCmd::Refresh);
        }
        ctx.request_repaint_after(Duration::from_millis(500));

        // ---------- 顶部状态栏 ----------
        egui::containers::Panel::top("status").show(ui, |ui| {
            ui.horizontal(|ui| {
                if self.monitoring {
                    ui.colored_label(egui::Color32::from_rgb(76, 175, 80), "● 监控中");
                } else {
                    ui.colored_label(egui::Color32::from_rgb(158, 158, 158), "○ 未监控");
                }
                ui.separator();
                match self.foreground {
                    Some(pid) => {
                        let name = self
                            .sessions
                            .iter()
                            .find(|s| s.pid == pid)
                            .map(|s| s.process_name.clone())
                            .unwrap_or_else(|| "未知".into());
                        ui.label(format!("前台: {name} (PID {pid})"));
                    }
                    None => {
                        ui.label("前台: 无窗口");
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if self.monitoring {
                        if ui.button("⏹ 停止").clicked() {
                            let _ = self.cmd_tx.send(UiCmd::Stop);
                        }
                    } else if ui.button("▶ 开始监控").clicked() {
                        self.send_selection();
                        let _ = self.cmd_tx.send(UiCmd::Start);
                    }
                    if ui.button("🔄 刷新").clicked() {
                        let _ = self.cmd_tx.send(UiCmd::Refresh);
                    }
                });
            });
            if let Some(err) = &self.error {
                ui.colored_label(egui::Color32::from_rgb(244, 67, 54), err);
            }
        });

        // ---------- 中央应用列表 ----------
        egui::CentralPanel::default().show(ui, |ui| {
            ui.add_space(4.0);
            if self.sessions.is_empty() {
                ui.weak("未检测到正在发声的应用。请先让应用播放声音,再点击刷新。");
            }
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    for s in &self.sessions {
                        let mut checked = self.selected.contains(&s.pid);
                        let is_fg = self.foreground == Some(s.pid);
                        // 勾选变更记录在闭包外处理,避免闭包同时借用 self 的不同部分
                        let mut toggled = false;
                        ui.horizontal(|ui| {
                            if ui.checkbox(&mut checked, "").changed() {
                                toggled = true;
                            }
                            ui.label(egui::RichText::new(s.display_name()).strong());
                            ui.weak(format!("PID {}", s.pid));
                            if s.session_count > 1 {
                                ui.weak(format!("({} 会话)", s.session_count));
                            }
                            if is_fg {
                                ui.colored_label(
                                    egui::Color32::from_rgb(33, 150, 243),
                                    "前台",
                                );
                            } else if s.muted {
                                ui.colored_label(
                                    egui::Color32::from_rgb(244, 67, 54),
                                    "静音",
                                );
                            }
                            ui.add(
                                egui::ProgressBar::new(s.volume as f32)
                                    .desired_width(80.0)
                                    .text(format!("{:.0}%", s.volume * 100.0)),
                            );
                        });
                        if toggled {
                            if checked {
                                self.selected.insert(s.pid);
                            } else {
                                self.selected.remove(&s.pid);
                            }
                            self.send_selection();
                        }
                    }
                });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.weak(format!("已选择 {} 个应用", self.selected.len()));
                ui.separator();
                ui.weak("关闭窗口将最小化到托盘,不会退出。");
            });
        });
    }
}
