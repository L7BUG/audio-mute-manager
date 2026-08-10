//! WASAPI 音频会话管理(Windows):枚举会话、静音控制、新会话通知。
//!
//! 所有函数必须在同一个 COM 线程内调用(monitor 线程),
//! COM 接口不可跨线程自由传递。

use std::collections::HashMap;
use std::sync::mpsc::Sender;

use windows::core::{implement, Interface, Result, GUID};
use windows::Win32::Media::Audio::{
    eConsole, eRender, IAudioSessionControl, IAudioSessionControl2, IAudioSessionManager2,
    IAudioSessionNotification, IAudioSessionNotification_Impl, IMMDevice, IMMDeviceEnumerator,
    ISimpleAudioVolume,
};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

use crate::model::SessionInfo;
use crate::process_name::process_name_of;

/// CLSID_MMDeviceEnumerator(windows 0.62 未内置该常量,按规范 GUID 定义)
pub const CLSID_MMDEVICE_ENUMERATOR: GUID =
    GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);

/// 音频 API 封装:持有会话管理器,供 monitor 线程使用
pub struct AudioApi {
    pub manager: IAudioSessionManager2,
}

impl AudioApi {
    /// 初始化:创建设备枚举器 -> 默认渲染设备 -> 会话管理器
    pub fn init() -> Result<Self> {
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&CLSID_MMDEVICE_ENUMERATOR, None, CLSCTX_ALL)? };
        let device: IMMDevice = unsafe { enumerator.GetDefaultAudioEndpoint(eRender, eConsole)? };
        let manager: IAudioSessionManager2 =
            unsafe { device.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None)? };
        Ok(Self { manager })
    }

    /// 单趟枚举所有会话,返回 (PID, 音量控制接口) 列表。
    /// 系统会话(无 PID)已过滤;失败的会话跳过,不中断整体。
    pub fn enumerate_controls(&self) -> Vec<(u32, ISimpleAudioVolume)> {
        let mut out = Vec::new();
        if let Ok(enumerator) = unsafe { self.manager.GetSessionEnumerator() } {
            let count = unsafe { enumerator.GetCount() }.unwrap_or(0);
            for i in 0..count {
                if let Ok(ctl) = unsafe { enumerator.GetSession(i) } {
                    if let Ok(pid) = session_pid(&ctl) {
                        if pid != 0 {
                            if let Ok(vol) = ctl.cast::<ISimpleAudioVolume>() {
                                out.push((pid, vol));
                            }
                        }
                    }
                }
            }
        }
        out
    }

    /// 枚举所有会话,按 PID 聚合为 SessionInfo 列表
    pub fn enumerate(&self) -> Vec<SessionInfo> {
        let mut map: HashMap<u32, (bool, f32, u32)> = HashMap::new();
        for (pid, vol) in self.enumerate_controls() {
            let muted = unsafe { vol.GetMute() }.map(|b| b.as_bool()).unwrap_or(false);
            let volume = unsafe { vol.GetMasterVolume() }.unwrap_or(0.0);
            let e = map.entry(pid).or_insert((muted, volume, 0));
            e.2 += 1;
            if e.2 == 1 {
                e.0 = muted;
                e.1 = volume;
            }
        }
        let mut list: Vec<SessionInfo> = map
            .into_iter()
            .map(|(pid, (muted, volume, session_count))| SessionInfo {
                pid,
                process_name: process_name_of(pid),
                muted,
                volume,
                session_count,
            })
            .collect();
        list.sort_by(|a, b| a.process_name.to_lowercase().cmp(&b.process_name.to_lowercase()));
        list
    }

    /// 恢复所有会话的音量(退出/停止时调用)
    pub fn unmute_all(&self) {
        for (_pid, vol) in self.enumerate_controls() {
            let _ = unsafe { vol.SetMute(false, &GUID::zeroed()) };
        }
    }

    /// 注册新会话通知,新会话出现时通过 tx 发送其 PID
    pub fn register_notifier(&self, tx: Sender<u32>) -> Result<()> {
        let notifier: IAudioSessionNotification = SessionNotifier { tx }.into();
        unsafe { self.manager.RegisterSessionNotification(&notifier) }
    }
}

/// 提取会话的 PID;系统会话返回 0
fn session_pid(ctl: &IAudioSessionControl) -> Result<u32> {
    let ctl2: IAudioSessionControl2 = ctl.cast()?;
    unsafe { ctl2.GetProcessId() }
}

/// COM 通知对象:新会话创建时把 PID 发给 monitor 线程
#[implement(IAudioSessionNotification)]
struct SessionNotifier {
    tx: Sender<u32>,
}

impl IAudioSessionNotification_Impl for SessionNotifier_Impl {
    fn OnSessionCreated(
        &self,
        new_session: windows::core::Ref<IAudioSessionControl>,
    ) -> windows::core::Result<()> {
        // Ref 的 Deref 目标是 Option<IAudioSessionControl>
        if let Some(ctl) = (&*new_session).as_ref() {
            if let Ok(pid) = session_pid(ctl) {
                if pid != 0 {
                    let _ = self.tx.send(pid);
                }
            }
        }
        Ok(())
    }
}
