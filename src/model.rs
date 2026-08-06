//! 纯数据模型:音频会话信息(跨线程传递,不含 COM 对象)

/// 一个音频会话的只读快照,用于 UI 展示
#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub pid: u32,
    pub process_name: String,
    pub muted: bool,
    pub volume: f32,
    /// 会话数(同一进程可能多个会话,如浏览器多标签)
    pub session_count: u32,
}

impl SessionInfo {
    pub fn display_name(&self) -> String {
        if self.process_name.is_empty() {
            format!("PID {}", self.pid)
        } else {
            self.process_name.clone()
        }
    }
}

/// UI -> 监控线程 的命令
#[derive(Debug, Clone)]
pub enum UiCmd {
    /// 更新被管理(勾选)的 PID 集合
    SetSelection(Vec<u32>),
    /// 开始监听(启动前台钩子并应用策略)
    Start,
    /// 停止监听(恢复所有音量)
    Stop,
    /// 请求刷新会话列表
    Refresh,
    /// 退出程序
    Quit,
}

/// 监控线程 -> UI 的事件
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// 会话列表已刷新
    Sessions(Vec<SessionInfo>),
    /// 当前前台 PID
    Foreground(Option<u32>),
    /// 监控状态变化
    Monitoring(bool),
    /// 错误信息
    Error(String),
}
