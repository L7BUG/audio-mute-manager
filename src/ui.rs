//! egui 主界面:应用列表 + 勾选 + 启动/停止 + 状态显示
//!
//! 注意:eframe 0.36 的 App trait 方法为 `fn ui(&mut self, ui: &mut egui::Ui, frame)`,
//! 面板 API 统一为 `egui::containers::Panel`。

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use eframe::egui;

use crate::config::Config;
use crate::model::{SessionInfo, UiCmd, UiEvent};

/// 嵌入思源黑体子集(Noto Sans SC Subset),支持中文显示
/// 子集包含:UI 文字 + GB2312 一级字(3755 常用汉字)+ 常用标点,
/// 体积 1.7MB(全量 16MB 的 1/10),编译期通过 include_bytes! 打进 exe
fn install_cjk_font(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "noto_sc".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/NotoSansSC-Subset.otf"
        ))),
    );
    // 优先使用中文字体,回退到默认字体(缺字形时)
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "noto_sc".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "noto_sc".to_owned());
    ctx.set_fonts(fonts);
}

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
    /// 持久化配置(按进程名恢复勾选)
    config: Config,
    config_path: PathBuf,
    /// 首次收到会话列表时是否已按配置预勾选
    config_loaded: bool,
}

impl MuteApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        cmd_tx: Sender<UiCmd>,
        event_rx: Receiver<UiEvent>,
        tray_rx: Receiver<TrayCmd>,
        config_path: PathBuf,
    ) -> Self {
        // 嵌入中文字体(egui 默认字体不含 CJK,否则中文显示为方块)
        install_cjk_font(&cc.egui_ctx);
        let config = Config::load(&config_path);
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
            config,
            config_path,
            config_loaded: false,
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
                    // 首次收到会话时,按配置中的进程名预勾选(重启恢复)
                    if !self.config_loaded {
                        self.config_loaded = true;
                        let names: HashSet<&str> =
                            self.config.managed().iter().map(|s| s.as_str()).collect();
                        for s in &self.sessions {
                            if names.contains(s.process_name.as_str()) {
                                self.selected.insert(s.pid);
                            }
                        }
                        self.send_selection();
                    }
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

    /// 按当前勾选的进程名更新配置并写盘
    fn persist_selection(&mut self) {
        let names: Vec<String> = self
            .sessions
            .iter()
            .filter(|s| self.selected.contains(&s.pid) && !s.process_name.is_empty())
            .map(|s| s.process_name.clone())
            .collect();
        self.config.set_managed(names);
        self.config.save(&self.config_path);
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
                    let mut selection_changed = false;
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
                            selection_changed = true;
                            if checked {
                                self.selected.insert(s.pid);
                            } else {
                                self.selected.remove(&s.pid);
                            }
                        }
                    }
                    // 循环外统一持久化与同步,避免闭包内 &mut self 与 &self.sessions 借用冲突
                    if selection_changed {
                        self.persist_selection();
                        self.send_selection();
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
