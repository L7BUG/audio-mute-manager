//! 监控线程:持有音频 API(COM),处理 UI 命令与前台切换事件,执行静音策略。
//!
//! 线程内职责:
//! - 初始化 COM(STA)与 AudioApi
//! - 维护被选中 PID 集合与监控开关
//! - 收到前台变化事件 -> 对选中集合应用 decide() 策略
//! - 收到新会话通知 -> 刷新列表并应用策略
//! - 收到 UI 命令 -> 增删选择 / 启停 / 退出(退出前恢复所有音量)

use std::collections::HashSet;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};

use crate::audio::AudioApi;
use crate::foreground;
use crate::model::{UiCmd, UiEvent};
use crate::policy::{decide, Action};

pub struct MonitorHandle {
    /// 保留句柄以延长线程生命周期(进程退出时线程随进程终止)
    pub _thread: JoinHandle<()>,
}

/// 启动监控线程。
/// - `rx_cmd`: 接收 UI 命令
/// - `tx_event`: 向 UI 发送状态事件
/// 返回句柄与前台事件发送端。
pub fn spawn_monitor(rx_cmd: Receiver<UiCmd>, tx_event: Sender<UiEvent>) -> MonitorHandle {
    let (fg_tx, fg_rx) = std::sync::mpsc::channel::<u32>();
    let (notif_tx, notif_rx) = std::sync::mpsc::channel::<u32>();

    // 前台钩子:独立线程跑消息泵,事件发往 fg_rx。
    // 必须在闭包外启动,避免 fg_tx 被 move 进 monitor 线程。
    let _hook = foreground::spawn_foreground_hook(fg_tx);

    let thread = std::thread::Builder::new()
        .name("monitor".into())
        .spawn(move || {
            // COM 必须在本线程初始化,音频接口不跨线程
            let com_ok = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
            let mut audio: Option<AudioApi> = None;
            if com_ok {
                match AudioApi::init() {
                    Ok(api) => {
                        if let Err(e) = api.register_notifier(notif_tx.clone()) {
                            let _ = tx_event.send(UiEvent::Error(format!("注册会话通知失败: {e}")));
                        }
                        audio = Some(api);
                    }
                    Err(e) => {
                        let _ = tx_event.send(UiEvent::Error(format!("音频初始化失败: {e}")));
                    }
                }
            } else {
                let _ = tx_event.send(UiEvent::Error("COM 初始化失败".into()));
            }

            // 前台钩子线程已在 spawn_monitor 中启动,事件到达 fg_rx

            let mut selected: HashSet<u32> = HashSet::new();
            let mut monitoring = false;
            let mut foreground_pid: Option<u32> = None;
            let mut running = true;

            while running {
                // 优先处理命令;事件用 try_recv 轮询,避免阻塞命令处理
                let mut cmd_processed = false;
                while let Ok(cmd) = rx_cmd.try_recv() {
                    cmd_processed = true;
                    match cmd {
                        UiCmd::Refresh => {
                            if let Some(api) = &audio {
                                let list = api.enumerate();
                                let _ = tx_event.send(UiEvent::Sessions(list));
                            }
                        }
                        UiCmd::SetSelection(pids) => {
                            selected = pids.into_iter().collect();
                            if monitoring {
                                apply_policy(&audio, &selected, foreground_pid);
                            }
                        }
                        UiCmd::Start => {
                            monitoring = true;
                            let _ = tx_event.send(UiEvent::Monitoring(true));
                            if let Some(api) = &audio {
                                let list = api.enumerate();
                                let _ = tx_event.send(UiEvent::Sessions(list));
                            }
                            apply_policy(&audio, &selected, foreground_pid);
                        }
                        UiCmd::Stop => {
                            monitoring = false;
                            if let Some(api) = &audio {
                                api.unmute_all();
                            }
                            let _ = tx_event.send(UiEvent::Monitoring(false));
                            let _ = tx_event.send(UiEvent::Foreground(foreground_pid));
                        }
                        UiCmd::Quit => {
                            running = false;
                        }
                    }
                }

                if cmd_processed {
                    continue;
                }

                // 前台变化事件
                if let Ok(pid) = fg_rx.try_recv() {
                    foreground_pid = Some(pid);
                    let _ = tx_event.send(UiEvent::Foreground(Some(pid)));
                    if monitoring {
                        apply_policy(&audio, &selected, foreground_pid);
                    }
                    continue;
                }

                // 新会话通知
                if let Ok(_pid) = notif_rx.try_recv() {
                    if monitoring {
                        if let Some(api) = &audio {
                            let list = api.enumerate();
                            let _ = tx_event.send(UiEvent::Sessions(list));
                        }
                        apply_policy(&audio, &selected, foreground_pid);
                    }
                    continue;
                }

                std::thread::sleep(std::time::Duration::from_millis(30));
            }

            // 退出:恢复所有音量
            if let Some(api) = &audio {
                api.unmute_all();
            }
            if com_ok {
                unsafe { CoUninitialize() };
            }
            let _ = tx_event.send(UiEvent::Monitoring(false));
        })
        .expect("spawn monitor thread");

    MonitorHandle { _thread: thread }
}

/// 对选中集合中的每个 PID 应用策略
fn apply_policy(
    audio: &Option<AudioApi>,
    selected: &HashSet<u32>,
    foreground_pid: Option<u32>,
) {
    let Some(api) = audio else { return };
    let sel: Vec<u32> = selected.iter().copied().collect();
    for &pid in &sel {
        match decide(&sel, foreground_pid, pid) {
            Action::Mute => {
                let _ = api.set_mute_for_pid(pid, true);
            }
            Action::Unmute => {
                let _ = api.set_mute_for_pid(pid, false);
            }
            Action::None => {}
        }
    }
}
