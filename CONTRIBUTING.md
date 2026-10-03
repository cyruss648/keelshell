# 参与 KeelShell

欢迎报告问题、改进文档、补充平台验证或提交实现。较大的功能改动请先在 Issue 中描述使用场景与预期行为，以便确认范围和交互方式。

## 开发环境

安装 Rustup 和 Python 3.11+，按 [构建环境配置](.github/actions/setup-build/action.yml) 准备系统依赖。仓库固定 Rust 工具链，直接 registry 依赖采用 `x.y` 要求，Cargo.lock 保存解析后的完整版本。

```sh
cargo run -p keelshell-app --locked
python3 scripts/check.py
python3 -m unittest discover -s packaging -p 'test_*.py' -v
```

完整的维护约定见 [AGENTS.md](AGENTS.md)，当前开发上下文见 [交接记录](docs/HANDOFF.md)。

## 工程布局

| 路径 | 职责 |
| --- | --- |
| `crates/keelshell-app` | GPUI Kit 界面、交互、会话生命周期 |
| `crates/keelshell-core` | 配置模型、校验、持久化 |
| `crates/keelshell-session` | SSH、SFTP、转发、监控 |
| `crates/keelshell-ai` | 服务请求、上下文、脱敏、建议审阅 |
| `packaging` | 平台打包、归档验证与发布 |
| `docs` | 产品、设计决策、测试证据和路线图 |

## 提交改动

保持一次改动围绕一个明确问题，说明变化前后的行为。对涉及行为的改动增加相应测试；不要通过忽略失败或扩大超时来掩盖问题。阻塞网络和文件 I/O 应放在界面线程之外。

提交前运行适用的检查，更新相关产品文档与测试记录。请分别写清编译结果、自动测试、原生界面验收和真实服务互操作情况；某个平台通过不代表所有平台通过。

## 报告问题

请提供系统与架构、应用版本或提交 ID、复现步骤、预期结果和实际结果。截图与日志应移除主机凭据、API Key、私钥、内部服务地址和业务数据。可复现的匿名配置或隔离测试服务通常更有帮助。
