//! 前台窗口检测(Windows):
//! - `foreground_pid()`: 获取当前前台窗口所属进程 PID
//! - `spawn_foreground_hook()`: 独立线程注册 EVENT_SYSTEM_FOREGROUND 钩子,
//!   前台切换时通过 channel 发送新前台 PID(内部跑消息泵驱动回调)

use std::sync::mpsc::Sender;
use std::sync::OnceLock;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowThreadProcessId,
    TranslateMessage, EVENT_SYSTEM_FOREGROUND, MSG, WINEVENT_OUTOFCONTEXT,
};

static FG_TX: OnceLock<Sender<u32>> = OnceLock::new();

/// 获取当前前台窗口的进程 PID;无前台窗口(锁屏等)返回 None
pub fn foreground_pid() -> Option<u32> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.0.is_null() {
            return None;
        }
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        (pid != 0).then_some(pid)
    }
}

/// 前台切换事件回调(WINEVENT_OUTOFCONTEXT,在注册线程的消息泵中执行)
unsafe extern "system" fn fg_event_proc(
    _hook: HWINEVENTHOOK,
    _event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _id_thread: u32,
    _time: u32,
) {
    let pid = foreground_pid();
    if let Some(tx) = FG_TX.get() {
        if let Some(p) = pid {
            let _ = tx.send(p);
        }
    }
}

/// 启动前台切换监听线程。返回后,前台变化会通过 `tx` 收到新 PID。
/// 线程内部运行消息泵,直到进程退出。
pub fn spawn_foreground_hook(tx: Sender<u32>) -> std::thread::JoinHandle<()> {
    let _ = FG_TX.set(tx);
    std::thread::Builder::new()
        .name("foreground-hook".into())
        .spawn(move || unsafe {
            // SetWinEventHook 直接返回 HWINEVENTHOOK;WINEVENTPROC 是 Option<fn>
            let _hook: HWINEVENTHOOK = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(fg_event_proc),
                0,
                0,
                WINEVENT_OUTOFCONTEXT,
            );
            let mut msg = MSG::default();
            // 消息泵:驱动 WinEvent 回调(必须在有消息循环的线程)
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            }
        })
        .expect("spawn foreground hook thread")
}
