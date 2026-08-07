# Audio Mute Manager(后台应用自动静音)

Windows 桌面工具:检测所有正在发声的应用程序,勾选要管理的应用后,
**应用在前台时自动恢复音量,切到后台自动静音**。

## 功能

- 枚举系统所有音频会话(应用名 / PID / 音量 / 静音状态 / 会话数)
- 勾选应用后点击"开始监控",策略自动生效
- 前台窗口切换实时响应(SetWinEventHook,非轮询)
- 新音频会话自动接管(应用重启、重新播放声音后策略依然有效)
- 关闭窗口最小化到系统托盘,托盘菜单:显示 / 刷新 / 退出
- 退出或停止时自动恢复所有音量

## 构建(Windows 环境)

需要 Rust MSVC 工具链(https://rustup.rs 安装,默认即可):

```bash
cargo build --release
```

产物:`target/release/audio-mute-manager.exe`,单文件,无需额外运行时。
复制到任何 Windows 10/11 机器即可运行(无需管理员权限)。

## 使用

1. 让目标应用播放声音(未发声的应用不会出现在列表)
2. 勾选要自动静音的应用(浏览器等可多选)
3. 点击 "▶ 开始监控"
4. 切走 → 自动静音;切回 → 自动恢复
5. 关闭窗口 → 驻留托盘继续工作;托盘"退出" → 恢复所有音量并退出

## 架构

```
src/
  main.rs          入口(cfg(windows) 分流)
  model.rs         纯数据模型 + UI<->监控线程通道消息
  policy.rs        策略纯函数 decide()(可单元测试,无 Windows 依赖)
  audio.rs         WASAPI:枚举会话 / 静音 / 新会话通知(COM)
  process_name.rs  PID -> 进程名
  foreground.rs    前台 PID 检测 + EVENT_SYSTEM_FOREGROUND 钩子
  monitor.rs       监控线程:命令/事件分发,执行静音策略
  ui.rs            egui 界面(列表 / 勾选 / 启停 / 状态)
  tray.rs          系统托盘(菜单事件转发)
  windows_main.rs  Windows 组装入口
```

线程模型:
- 主线程:egui UI + 托盘事件消费
- monitor 线程:COM(STA)+ 音频会话管理 + 策略执行
- foreground-hook 线程:WinEvent 钩子 + 消息泵(事件转发给 monitor)

## 已知限制

- 未发声的应用不会出现在列表(需要先播放声音)
- 浏览器多标签 = 同一进程多会话,勾选粒度是"进程"
- 系统音效(无 PID 会话)自动忽略
- 锁屏/断开远程会话时无前台窗口,被选应用按"后台"静音处理

## 许可

MIT License,详见 [LICENSE](LICENSE) 文件。
