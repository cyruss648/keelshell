# GPUI Kit 与真实终端技术调查

核实日期：2026-10-02（Asia/Shanghai）。本文件为只读调查记录，没有修改应用源码、安装依赖、运行应用或执行提交。版本来自 crates.io 实时 API，API 名称另与发布 tag 源码核对。

## 1. 名称与版本结论

用户指定的 **GPUI Kit 是真实项目与 crate**，可以直接采用 `gpui-kit = "0.7"`。官方仓库已从 GPUI Component 发展为 GPUI Kit；不能把用户需求悄悄换成旧的独立 `gpui-component` 组合。

- 官方仓库：[longbridge/gpui-kit](https://github.com/longbridge/gpui-kit)。
- 最新非撤回稳定发布：**0.7.0**，2026-09-28 03:14:47 UTC 发布；[crates.io API](https://crates.io/api/v1/crates/gpui-kit)、[0.7.0 manifest](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/kit/Cargo.toml)。
- `gpui-kit` 是应用入口，默认包含 `component`、`assets`；导出 `gpui_kit::*`（GPUI）、`gpui_kit::base`、`gpui_kit::component`、`gpui_kit::assets`。应用无需再声明一个不匹配的 `gpui` 版本。
- `gpui-component` 是 styled component 层；`gpui-base` 是行为与基础设施层。`gpui-shell` 是可选 JavaScript 扩展运行时，**不是 PTY/shell 终端控件**，本项目无需仅因名称含 shell 而依赖它。
- **软件与代码示例 Apache-2.0**；官方文档 prose/原始插图另提供 CC BY 4.0，复制文案与图示需相应署名。参考事实并自行设计不需要复制文档表达。[官方许可说明](https://github.com/longbridge/gpui-kit#license)
- 本机 cargo 缓存曾有 `gpui 0.2.2`、`gpui-component 0.5.1`，这些不能代表当前版本。

官方安装页面与快速开始页面的版本选择器虽已显示 v0.7.0，正文部分仍写 `0.6` / `0.6.5`。版本选择以 crates.io 已发布记录和 v0.7.0 tag 为准，API 以同一 tag 源码为准。

## 2. 依赖声明与可复现性

用户要求的 x.y 应落实于**项目自身 Cargo.toml 直接依赖**；提交 Cargo.lock 保留实际完整解析版本。`gpui-kit 0.7.0` 内部明确将 GPUI 快照全家族锁为 `=0.3.7`，原因是不同快照可能导致 API 不兼容。不能改写上游传递依赖或移除 lockfile 来追求表面上的 x.y。[tag 根 manifest](https://github.com/longbridge/gpui-kit/blob/v0.7.0/Cargo.toml)

已实时核对的候选（按使用需求添加，避免预先引入所有库）：

| 用途 | crate | 最新稳定版本 | 本项目声明 | 许可 |
| --- | --- | --- | --- | --- |
| UI | gpui-kit | 0.7.0 | `"0.7"` | Apache-2.0 |
| 本地 PTY/Windows ConPTY | portable-pty | 0.9.0 | `"0.9"` | MIT |
| ANSI/VT 终端状态机 | alacritty_terminal | 0.26.0 | `"0.26"` | Apache-2.0 |
| SSH | russh | 0.63.3 | `"0.63"` | Apache-2.0 |
| SFTP | russh-sftp | 3.0.1 | `"3.0"` | Apache-2.0 |
| HTTP / AI provider | reqwest | 0.13.5 | `"0.13"` | MIT OR Apache-2.0 |
| 异步运行时 | tokio | 1.53.1 | `"1.53"` | MIT |
| 序列化 | serde | 1.0.229 | `"1.0"` | MIT OR Apache-2.0 |
| 应用错误上下文 | anyhow | 1.0.104 | `"1.0"` | MIT OR Apache-2.0 |
| 类型化领域错误 | thiserror | 2.0.21 | `"2.0"` | MIT OR Apache-2.0 |
| 稳定实体 ID | uuid | 1.26.1 | `"1.26"` | Apache-2.0 OR MIT |
| 系统应用目录 | directories | 6.0.0 | `"6.0"` | MIT OR Apache-2.0 |
| 系统凭据存储 | keyring | 4.2.0 | `"4.2"` | MIT OR Apache-2.0 |
| 配置格式 | toml | 1.1.6+spec-1.1.0 | `"1.1"` | MIT OR Apache-2.0 |
| 结构化诊断 | tracing | 0.1.44 | `"0.1"` | MIT |
| 日志订阅层 | tracing-subscriber | 0.3.23 | `"0.3"` | MIT |
| 时间 | time | 0.3.55 | `"0.3"` | MIT OR Apache-2.0 |

来源是各 crate 的 `https://crates.io/api/v1/crates/<crate>`，例如 [portable-pty](https://crates.io/api/v1/crates/portable-pty)、[alacritty_terminal](https://crates.io/api/v1/crates/alacritty_terminal)、[russh](https://crates.io/api/v1/crates/russh)、[russh-sftp](https://crates.io/api/v1/crates/russh-sftp)、[reqwest](https://crates.io/api/v1/crates/reqwest)。`russh-sftp` 当前仓库自身 dev-dependency 为 `russh 0.63.2`，处于推荐的 0.63 系列；仍须应用集成编译与受控 SSH/SFTP 服务测试确认组合行为。

## 3. 最小窗口 API

以下为按 0.7.0 发布源码 API 组成的最小程序，可作为实现起点；**本调查没有编译或运行，不能算构建验收**。官方存在同类可执行 hello_world 与 recipe 测试。[官方 hello_world](https://github.com/longbridge/gpui-kit/blob/v0.7.0/examples/hello_world/src/main.rs)、[facade 源码](https://github.com/longbridge/gpui-kit/blob/v0.7.0/crates/kit/src/lib.rs)

```toml
[package]
name = "terminal-app"
version = "0.1.0"
edition = "2024"

[dependencies]
gpui-kit = "0.7"
```

```rust
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::*;

struct ShellApp;

impl Render for ShellApp {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .items_center()
            .justify_center()
            .gap_2()
            .child("Shell workspace")
            .child(Button::new("connect").primary().label("Connect"))
    }
}

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        open_window(WindowOptions::default(), cx, |_, cx| {
            cx.new(|_| ShellApp)
        })
        .expect("failed to create application window");
    });
}
```

API 注意事项：

- `init(cx)` 只在应用初始化时调用，先于组件与窗口创建。
- Kit `open_window` 已包装 `base::Root`，闭包返回应用内容 entity，不应重复包装 Root；返回 `(AnyWindowHandle, Entity<V>)`。
- Input、dock、terminal 等持续状态存放在所属 view/entity 中，不能每次 render 重建。
- `gpui_kit::actions!` 是 facade 定制宏，避免引用只存在于独立 `gpui` dependency 的 derive path。
- UI 测试用可选 feature `test-support` 与 `#[gpui_kit::test]`；测试模块显式导入类型，避免 glob import 导致普通 `#[test]` 名称混淆。

## 4. 跨平台与工程判断

官方安装要求：macOS 15+ 与 Xcode Command Line Tools；Windows 10+、MSVC Rust、Visual Studio C++ workload、Windows SDK、CMake；Linux 有 Ubuntu 24.04 原生库清单，运行需要图形 Wayland/X11 会话与可用 Vulkan 驱动。跨平台锁定依赖图的 Rust 基线至少 1.92（Linux oo7 依赖约束）。这些是上游要求，尚未证明本项目三平台可运行。[安装文档](https://gpui-kit.com/docs/installation/)

上游 v0.7.0 CI 文件配置了 macOS arm64、Linux x64、Windows x64 的测试 matrix，另在 macOS 配置真实 Metal rendering tests。这证明上游**设置了**三个平台检查路径，不能等同于此次应用已通过三个平台验证，也不能仅凭配置推断所有上游运行均成功。[CI 定义](https://github.com/longbridge/gpui-kit/blob/v0.7.0/.github/workflows/ci.yml)

建议项目：

1. 核心配置、连接模型、安全策略、ANSI 状态与 AI 上下文处理独立于 GPUI。Linux/Windows 原生 CI 单独验证，macOS 当前机做实际 UI 验收。
2. 无 GPU 核心测试、GPUI headless interaction 测试、真实 GPU/IME/clipboard/SSH 验收分层记录，不能把 cargo check 当产品可用性证明。
3. x64/arm64 是否作为初始发布目标由产品计划显式列出；未实际原生验证的平台标记 planned/unverified。
4. 首次 GPUI 构建较重。开发时可用 `debug = "limited"`，框架依赖适量 opt-level 提升交互表现；这会增加初次编译时间而非减少它。

## 5. 真实终端架构建议

推荐三个独立职责：**transport/session → terminal emulator → GPUI terminal renderer**。

### Transport 与 PTY

- 本地 session 用 `portable-pty::native_pty_system()`、`openpty(PtySize)`、`slave.spawn_command(CommandBuilder)`、master reader/writer；窗口 resize 同步 `MasterPty::resize`。Unix 后端为 UnixPtySystem，Windows 为 ConPTY。库提供 child 等待/终止接口。[portable-pty API](https://docs.rs/portable-pty/0.9.0/portable_pty/)、[平台选择源码](https://github.com/wezterm/wezterm/blob/main/pty/src/lib.rs)
- SSH session 用 `russh` 分配远端 PTY，request shell，收发原始 bytes，resize 发 window-change；SFTP 使用独立 subsystem/channel。必须实现 known_hosts 校验与首次指纹确认，不能示例式无条件接受服务器公钥。
- 构造 `SessionTransport` 的输入/输出/resize/close 生命周期接口；PTY blocking I/O 放后台 worker，通知 UI 时通过 bounded channel/coalescing，避免 1 byte 1 redraw。
- 普通 `Command::output()` 捕获不是交互终端，无法满足 shell line discipline、Ctrl-C、vim/top、终端 resize 语义。不要用文本框执行一条命令冒充终端。

### ANSI/VT 模拟

- 使用 `alacritty_terminal 0.26` 的 `Term<EventListener>` 与其 `vte` 再导出解析原始字节，保留状态；不要用 regex 剥离 ANSI 或逐行 append。
- `Term::new(config, &dimensions, event_proxy)`；`Term::renderable_content`、`grid`、`mode`、`resize`、`scroll_display`、`selection_to_string`、`damage/reset_damage` 为实现需要的已确认公开 API。[Term API](https://docs.rs/alacritty_terminal/0.26.0/alacritty_terminal/term/struct.Term.html)
- emulator 发出的 PTY response、title、bell、clipboard 等事件需要应用映射；尤其 device status 查询应回写真实 transport，否则 TUI 会挂起等待。[Event API](https://docs.rs/alacritty_terminal/0.26.0/alacritty_terminal/event/enum.Event.html)
- 渲染 cell 栅格：fg/bg、bold/italic/underline、光标、wide/combining chars、alternate screen、scrollback、鼠标选择、scroll offset。以 visible cells/变化区域工作，避免完整 scrollback 每帧转成巨大字符串。
- 输入 key 根据当前 terminal mode 编码，支持 Ctrl/Alt、方向键、function keys、application cursor/keypad；paste 根据 bracketed-paste mode 包装。终端模式与 GUI shortcut 的优先级需可配置。

### IME、剪贴板、GPUI

- 终端 view 实现 `EntityInputHandler`，由 GPUI `ElementInputHandler` 安装到绘制 element；文字输入用 committed-text 路径，不能只监听 `on_key_down` 拼字符。[GPUI 0.3.7 input 源码](https://docs.rs/gpui-pre/0.3.7/src/gpui/input.rs.html)
- 必须处理 `text_for_range`、`selected_text_range`、`marked_text_range`、`unmark_text`、`replace_text_in_range`、`replace_and_mark_text_in_range`、`bounds_for_range`、`character_index_for_point`。候选框定位跟随 terminal cursor；UTF-16 range 与 UTF-8/Unicode cell 区别需要测试。
- composing/marked text 只在本地绘制，commit 才写 transport。否则中文输入过程会把拼音逐键发进远端 shell。
- 复制使用终端选择范围；paste 使用系统剪贴板和当前 terminal mode。OSC52 clipboard read/write 默认受连接权限控制，避免远端输出直接读取本地剪贴板；多行 paste 给清晰预览。

### 避免误引许可证

GPUI 本体是 Apache-2.0，但 **Zed `crates/terminal` 与 `crates/terminal_view` 当前各自 manifest 是 GPL-3.0-or-later**。可以研究架构，不应无意识复制实现进准备按其他许可发布的项目。应以 alacritty_terminal + portable-pty + 自有 GPUI renderer 实现。已只读核实：[terminal manifest](https://github.com/zed-industries/zed/blob/main/crates/terminal/Cargo.toml)、[terminal_view manifest](https://github.com/zed-industries/zed/blob/main/crates/terminal_view/Cargo.toml)。

## 6. 最低终端验收集

以下为建议测试，不是已执行结果：

- 单元：CR 覆盖、ANSI truecolor、宽字符/combining、跨 chunk UTF-8、alt-screen 往返、scrollback 边界、DSR 回应、按键模式、bracketed paste、resize 后 cursor/grid 一致性。
- 本地集成：真实 PTY 下执行带唯一 sentinel 的 shell 命令，检查 `isatty`、Ctrl-C、退出状态、连续 resize、关闭 tab 后子进程与 worker 退出；设置超时确保失败可控。
- SSH 集成：受控 sshd，正确/未知/变化 host key，password/key/agent，连接失败/超时/掉线，远端 resize 与 interactive shell，SFTP 上传下载哈希和取消。
- 原生 UI：macOS 中文 IME、英文键盘、CJK/emoji、选中复制、paste、vim/top、多窗口 focus；Windows ConPTY 和 Linux Wayland/X11 分别原生跑同类记录。
- 长时：大量持续 output 时 UI 可响应，bounded queue 不无限增内存，scrollback 上限可靠，关闭大量 sessions 后无后台泄漏。

## 7. 证据范围

确认：存在与版本、发布 manifest、关键 API、上游平台/CI 声明、候选许可证、终端能力所需实现接口。

未确认：本项目依赖解析、实际构建、最小窗口运行、GPUI 0.7.0 在当前机器表现、跨平台原生行为、SSH/SFTP 互操作、IME 正确性与性能。后续实现必须将这些分别填入测试记录，不得用本调查替代验收。
