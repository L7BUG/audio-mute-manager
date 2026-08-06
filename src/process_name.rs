//! PID -> 进程名(Windows)

use windows::core::PWSTR;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};

/// 根据 PID 获取进程可执行文件名(如 "chrome.exe")。
/// 进程不存在或无权限时返回空字符串。
pub fn process_name_of(pid: u32) -> String {
    if pid == 0 {
        return String::new();
    }
    unsafe {
        match OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) {
            Ok(handle) => {
                let mut buf = [0u16; 1024];
                let mut len = buf.len() as u32;
                let name = QueryFullProcessImageNameW(
                    handle,
                    windows::Win32::System::Threading::PROCESS_NAME_WIN32,
                    PWSTR(buf.as_mut_ptr()),
                    &mut len,
                )
                .map(|_| {
                    let path = String::from_utf16_lossy(&buf[..len as usize]);
                    std::path::Path::new(&path)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or(path)
                })
                .unwrap_or_default();
                let _ = windows::Win32::Foundation::CloseHandle(handle);
                name
            }
            Err(_) => String::new(),
        }
    }
}
