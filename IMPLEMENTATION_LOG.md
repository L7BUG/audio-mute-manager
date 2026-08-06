# 实现日志 — Windows 后台应用自动静音工具

> 项目:`/home/l/rust/audio-mute-manager`(Linux 服务器上编写,Windows 目标交叉验证)
> 起始:2026-08-06。所有 Windows API 用法已通过
> `cargo check --target x86_64-pc-windows-msvc` 类型检查验证(0 error / 0 warning)。

## 任务完成情况

| 任务 | 内容 | 状态 |
|---|---|---|
| T1 | 项目初始化:Cargo.toml + 依赖 | ✅ |
| T2 | COM 初始化 + 默认渲染设备 | ✅ |
| T3 | 枚举音频会话(PID+进程名) | ✅ |
| T4 | 前台窗口 PID 检测 | ✅ |
| T5 | 静音控制 SetMute | ✅ |
| T6 | 前台切换事件监听 SetWinEventHook | ✅ |
| T7 | 控制器策略逻辑 + 单元测试(5 个用例全过) | ✅ |
| T8 | 新会话自动接管 IAudioSessionNotification | ✅ |
| T9 | egui 主界面 | ✅ |
| T10 | 系统托盘常驻 | ✅ |
| T11 | 打包与发布配置(release profile) | ✅ |

## 关键技术决策

1. **依赖全部 target-specific**:`[target.'cfg(windows)'.dependencies]`
   放 windows/eframe/tray-icon/image,Linux 上 `cargo test` 只编译纯逻辑,
   可以本地跑策略单测;Windows 上全量编译。

2. **线程模型**(COM 不跨线程):
   - 主线程:egui UI + 消费托盘命令
   - monitor 线程:CoInitializeEx(STA)+ AudioApi + 策略执行
   - foreground-hook 线程:SetWinEventHook + GetMessageW 消息泵,
     前台变化通过 mpsc 转发给 monitor
   - 新会话通知(IAudioSessionNotification 回调在 COM 线程)→ channel → monitor

3. **WINEVENT_OUTOFCONTEXT 回调必须在消息泵线程**:钩子回调由消息循环
   驱动,独立线程 + GetMessageW 循环解决(回调里直接 GetForegroundWindow
   取 PID 再 send,开销极小)。

4. **退出恢复音量**:monitor 线程收到 UiCmd::Quit 时先 unmute_all()
   再退出循环;托盘"退出"走同一路径。

## 踩坑记录(API 版本差异,全部已解决)

1. **windows 0.62 无 `CLSID_MMDeviceEnumerator` 常量**
   → 按规范 GUID 手动定义:`GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e)`

2. **`IMMDevice::Activate` 需要额外 feature**
   `Win32_System_Com_StructuredStorage` + `Win32_System_Variant`,否则
   E0599 method not found。

3. **SetWinEventHook 系列在 `Win32_UI_Accessibility`**,但
   `EVENT_SYSTEM_FOREGROUND` / `WINEVENT_OUTOFCONTEXT` 常量在
   `Win32_UI_WindowsAndMessaging`(两者都要)。

4. **SetWinEventHook 签名**:返回 `HWINEVENTHOOK`(非 Result);
   `WINEVENTPROC = Option<unsafe extern "system" fn(...)>`,直接传
   `Some(fg_event_proc)`。

5. **eframe 0.36 App trait 重构**:`fn update(ctx)` 改为
   `fn ui(&mut self, ui: &mut egui::Ui, frame)`;`TopBottomPanel/SidePanel`
   合并为 `egui::containers::Panel::top(id)` 等;`Panel::show` 与
   `CentralPanel::show` 都接受 `&mut Ui` 而非 `Context`。

6. **windows 0.62 COM 实现宏**:`#[implement(IFoo)]` 生成 `Foo_Impl` 包装类型,
   用户实现 trait 要写 `impl IFoo_Impl for Foo_Impl`(不是 Foo);
   字段通过 Deref 访问;`IAudioSessionNotification_Impl::OnSessionCreated`
   参数是 `Ref<IAudioSessionControl>`,Deref 目标为 `Option<...>`,需
   `(&*param).as_ref()` 取会话。

7. **tray-icon 0.24 事件闭包要求 Send**:菜单事件 handler 不能捕获
   `MenuItem`(内部 `Rc`),改为提前提取 `MenuId.0`(String)副本比较。
   `MenuEvent` 在 `tray_icon::menu` 命名空间下。

8. **GetMute 返回 `BOOL` 非 `bool`**:`.as_bool()` 转换。
   `HWND` 判空用 `.0.is_null()`(不是 `== 0`)。

9. **egui 借用**:`for s in &self.sessions` 内层闭包里同时改
   `self.selected` 会触发借用冲突,把勾选变更移到闭包外处理。

10. **IMMDevice::Activate 第二参数是 `Option<*const PROPVARIANT>`**,
    传 `None` 即可(依赖上面第 2 条的 feature)。

## 验证情况

- ✅ `cargo check --target x86_64-pc-windows-msvc`:**0 error / 0 warning**
- ✅ `cargo test`(Linux 本地):5 个策略单测全过
- ✅ **Windows 实机验证(用户亲测通过)**:应用列表枚举正常、中文显示
  正常、勾选后切后台自动静音、切回前台自动恢复——核心功能全部按预期工作

## 待办(如需继续)

- [ ] 开机自启(注册表 Run 键或 Startup 快捷方式,可选)
- [ ] 应用图标 .ico 嵌入(当前为运行时绘制的托盘图标)
- [ ] 同进程多会话的去重显示优化(当前已按 PID 聚合)

## 更新记录

### 2026-08-06 字体修复(方块字问题)

- 问题:egui 默认字体不含 CJK,中文界面显示为方块(豆腐块)
- 修复:`src/ui.rs` 新增 `install_cjk_font()`,编译期通过
  `include_bytes!` 嵌入思源黑体 Noto Sans SC(assets/NotoSansSC-Regular.otf,
  15MB,下载自 notofonts/noto-cjk),注册到 Proportional/Monospace 字体族
- 注意:egui 0.36 的 `font_data` 值为 `Arc<FontData>`(需 Arc::new 包裹);
  字体引擎(ab_glyph)不支持 .ttc 集合,服务器上的文泉驿正黑(wqy-zenhei.ttc)
  无法直接使用,故下载单文件 .otf
- exe 体积:13MB → 约 30MB(含字体)

### 2026-08-06 交叉编译成功(MinGW)

- 服务器自带 `x86_64-w64-mingw32-gcc`(GCC 14)+ `x86_64-pc-windows-gnu` target,
  可直接交叉编译出 Windows exe(此前仅做 MSVC target 类型检查)
- `cargo build --target x86_64-pc-windows-gnu --release`:全量 LTO 约 13 分钟
- 产物:`audio-mute-manager.exe`(PE32+ x86-64,依赖全为系统 DLL)
- 增加 `#![cfg_attr(windows, windows_subsystem = "windows")]`:隐藏控制台窗口,
  PE 子系统验证为 Windows GUI(00000002)

### 2026-08-06 Git 远程仓库

- 本地仓库初始化并提交(3 个提交),推送到
  `git@github.com:L7BUG/audio-mute-manager.git`(main 分支)
- 服务器已配置 GitHub SSH key 认证(用户 L7BUG)

### 2026-08-06 Windows 实机验证通过

- 用户在 Windows 上运行交叉编译的 exe,**亲测核心功能全部正常**:
  应用列表枚举、中文界面显示、后台自动静音、前台自动恢复
- 项目目录整理至 `~/rust/audio-mute-manager/`(Rust 项目集中管理)

### 2026-08-07 字体子集化(体积优化 29MB → 15MB)

- 问题:全量思源黑体 16MB 嵌入导致 exe 高达 29MB
- 优化:fonttools pyftsubset 提取 **GB2312 一级字(3755 常用汉字)
  + UI 源码字符(342 个)+ 常用标点**,子集字体 1.7MB(原 1/10)
- 验证:3791 字符全部覆盖 UI 文本与常见进程名(网易云音乐/微信等)
- exe 预计 29MB → 15MB
- 工具链备注:fonttools 装于 ~/.venvs/ft/pylib(--target 方式,规避
  PEP 668);服务器无 pip/uv,用 ~/.local/bin/python3.11(uv 管理的
  cpython)执行;注意 `pip --target=~/x` 波浪号不展开,须用 $HOME
