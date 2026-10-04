# 界面与本地智能体/MCP计划核对

日期：2026-10-04。类型：需求/资料核对，不是功能验收。

用户新增明暗主题（默认系统）、更丰富但克制的元素与专业视觉，以及对外MCP和本地Claude Code/Codex调用。随后明确纠正：不需要KeelShell访问其他MCP服务。该纠正已成为[正式计划](../../product/DESIGN_AND_AGENT_PLAN.md)的范围边界。

本轮读取现有主题装配与AI配置/参考文档，确认应用目前固定Light且有浅色硬编码；只读运行两项本地CLI的版本/帮助，未请求模型、读取认证内容或改变CLI设置。实际版本为Codex CLI0.160.0、Claude Code2.1.285；它们仅是本机检测值，不是未来跨平台支持基线。

已直接打开DBX AI/MCP、Codex非交互/app-server、Claude Code程序化运行、MCP2026-07-28传输、Zed主题、Fluent2颜色和WCAG对比说明。Apple HIG页面只返回JavaScript提示，部分Carbon/Radix/Fluent候选地址无法读取，未将其作为已核实的选型依据；Fluent2实际颜色文档可访问，已采用。官方链接保存在计划和[设计资料库](../../design/README.md)。

当前DBX MCP工具组可见，仅检查工具元数据；版本状态为“版本未知，已进行能力探测”。没有读取DBX连接/数据库，也没有执行SQL、消息或配置变更。本计划参照交互原则，不声称DBX真实业务流程或本地CLI模型请求已验收。

验证仅包含需求映射、范围排除、来源链接与Markdown本地引用检查；没有新增运行时代码，因此不为文档修改新增单元测试。原生截图、三平台主题切换、真实CLI和MCP调用必须在后续功能提交单独验收。

## 主题基础只读复核

独立代理及其子代理只读检查当前应用、core配置和本机锁定的GPUI Kit
0.7.0 / gpui-pre 0.3.7源码；没有修改运行时代码、启动GUI或请求模型。

- `keelshell-core/src/model.rs`已定义`Theme::{System, Dark, Light}`，现有默认
  为Light，`WireSettings.theme`缺少默认。应复用现有字段：新配置和缺字段
  默认System，保留明确保存的旧值；未知枚举/schema和损坏配置继续拒绝。
  旧文件无法区分Light来自历史默认还是用户选择，不应批量强改。
- `main.rs`固定Light后调用`design::install`；`design.rs`的七个固定浅色
  token及AI/文件/批量/危险审核中的直接色值需要共同迁移。终端ANSI/OSC
  的独立色板与背景契约不是应用外壳token。
- Toolkit的`ThemeMode`只有Light/Dark；core持久偏好需解析有效mode。
  `Window::appearance`和`App::window_appearance`可读外观，优先窗口以符合
  Linux实现；`observe_window_appearance`订阅保存在现有Workspace中，回调
  仅在System偏好时应用变化，不写配置。
- `Theme::change`后按有效mode安装语义色，使用`Theme::update`同步控件
  tokens/base投影。macOS `set_window_appearance`可强制显式外观，None恢复
  跟随；恢复时须处理窗口缓存/异步通知，其他平台此调用目前无实际作用。
- 显式偏好走候选状态和现有后台保存，成功后应用，失败保留原状态。
  Workspace普通保存会更新snippet sources并推进`command_sources_revision`；
  外观保存须避免无意义的推进，或证明审核/参数/补全/批量草稿不会失效。

最小回归应覆盖新/缺字段配置、三值往返与旧明示值、非法配置原字节保留；
同一Workspace的模式/控件/自绘色一致，显式模式不随系统变化；保存失败、
在途编辑、EntityId/文本/revision/审核/任务句柄保留。不能重建Workspace、
终端或AI/文件面板来换主题。三平台系统实时切换另做原生记录，Linux portal
缺失时的fallback不能作为其系统跟随验收。本节是入口事实及实施建议，主题
继续保持待实现状态。
