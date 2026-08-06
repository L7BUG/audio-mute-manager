//! 策略逻辑:纯函数,无 Windows 依赖,可单元测试。
//!
//! 规则:被选中的 PID,当前台 == 该 PID 时恢复音量(Unmute),
//! 否则一律静音(Mute)。未选中的 PID 不做任何事(None)。

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Mute,
    Unmute,
    None,
}

/// 决定对指定 PID 应采取的动作。
///
/// - `selected`: 被用户勾选管理的 PID 集合
/// - `foreground`: 当前前台窗口的 PID(None 表示无前台窗口,如锁屏/断开会话)
/// - `pid`: 要决策的目标 PID
pub fn decide(selected: &[u32], foreground: Option<u32>, pid: u32) -> Action {
    if !selected.contains(&pid) {
        return Action::None;
    }
    match foreground {
        Some(fg) if fg == pid => Action::Unmute,
        _ => Action::Mute,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_and_foreground_should_unmute() {
        assert_eq!(decide(&[100, 200], Some(100), 100), Action::Unmute);
    }

    #[test]
    fn selected_but_background_should_mute() {
        assert_eq!(decide(&[100, 200], Some(200), 100), Action::Mute);
    }

    #[test]
    fn unselected_should_do_nothing() {
        assert_eq!(decide(&[100], Some(100), 300), Action::None);
        assert_eq!(decide(&[100], Some(200), 300), Action::None);
    }

    #[test]
    fn no_foreground_should_mute() {
        // 锁屏/无前台窗口时,被选应用视为后台 → 静音
        assert_eq!(decide(&[100], None, 100), Action::Mute);
    }

    #[test]
    fn empty_selection_should_do_nothing() {
        assert_eq!(decide(&[], Some(100), 100), Action::None);
    }
}
