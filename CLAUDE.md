# audio-mute-manager

Windows 桌面工具：检测所有正在发声的应用程序，勾选要管理的应用后，**应用在前台时自动恢复音量，切到后台自动静音**。

## 技术栈

- Rust（`edition 2021`），仅 Windows 目标
- GUI：`eframe`（egui）
- 系统集成：`windows` crate（音频会话、前台窗口 Hook）、`tray-icon`、`image`

## 常用命令

```bash
cargo build --release   # 构建（产物：target/release/audio-mute-manager.exe）
cargo test              # 运行测试
cargo clippy -- -D warnings   # lint
cargo fmt               # 格式化
```

## 项目结构

- `src/main.rs` — 入口
- `src/audio.rs` — 音频会话枚举/音量控制
- `src/foreground.rs` — 前台窗口检测（SetWinEventHook）
- `src/monitor.rs` — 策略监控循环
- `src/policy.rs` — 静音策略
- `src/tray.rs` — 系统托盘
- `src/ui.rs` — egui 界面
- `src/windows_main.rs` — Windows 主流程

## 说明

- 需在 Windows 环境（Rust MSVC 工具链）构建；当前开发机为 Linux，无法本地构建验证
- 字体已子集化（GB2312 一级字 + UI 字符），避免打包过大
