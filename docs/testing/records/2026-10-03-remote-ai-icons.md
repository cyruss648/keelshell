# 远程工作区、命名 AI 配置和白底图标验收

执行日期：2026-10-03。环境：本机 macOS Apple Silicon，仓库迁移后的工作区。当前检查点为持续开发版本，不代表全部远程 SSH 能力已完成。

## 自动门禁

`python3 scripts/check.py` 最终通过：依赖 x.y 政策、cargo fmt、全工作区/全 target Clippy（-D warnings）、181 项测试，0 failed / 0 ignored。包含 core 存储迁移、AI HTTP、SSH/SFTP/TCP、终端输入、50项应用测试及文档示例。

应用测试验证真实 GPUI 事件，不在测试内代替生产 callback 改 active：分屏点击右侧后捕获右侧内容/host/session；两侧 bounds 不交换；命令只进入右侧传输队列；A 的命令草稿切到 B 后无法发送；清空后新命令才能绑定 B。

新 AI 测试验证命名配置 CRUD、默认项、无明文密钥持久化、旧配置迁移、模型发现、错误分类、取消、响应大小、请求期限、旧结果隔离、保存期间新输入保留、设置页接管焦点，以及准确预览撤销。没有调用商业 AI 服务。

`cargo build -p keelshell-app --locked` 通过。已知 transitive `block 0.1.6` 仍有 Rust future-incompatibility 提示，它不属于本次 -D warnings 错误；后续 GPUI 依赖升级需继续跟踪。

## 原生 macOS 流程

1. 新目录打包 .app，使用隔离数据目录启动；中文默认且没有本地 shell 会话。
2. 新建中文 SSH 配置、核对显示的 SHA256 指纹与夹具启动输出、信任后登录。重启后持久化连接/语言/AI默认项/主机信任读取正确。
3. 通过 SFTP 读取 welcome.txt，添加中文内容，审核目标及117字节长度，确认原子保存。夹具目录字节读取和重启后再打开文件都确认新内容。
4. SSH split 打开第二个真实 channel；同时开启 AI 侧栏，界面显示两个终端。
5. AI 设置中手动填写本机 HTTP 地址，无认证；发现两个模型，手动选 fixture-small，固定提示连接测试显示成功及服务端模型；设为默认并应用。
6. 捕获明确选定的终端屏幕，输入中文问题，预览准确 JSON 后显式发送。本机 HTTP 服务收到一条模型发现 GET、一条固定提示 probe POST 和一条用户确认的 chat POST。
7. 返回建议只进入命令审阅栏，用户动作后才送入 SSH 夹具并回显。夹具不执行系统命令。
8. 中英切换保留 SSH 会话、远程文件文本和 AI 问题/回答。实际发现并修复远程 Textarea 默认高度不足；最新构建中多行内容完整可见。
9. 访达应用简介正确识别为 Apple Silicon 应用，显示白底蓝青 K 图标；图标小尺寸及预览均显示。没有安装到 Applications。

## 图标与打包

35个产物通过源图身份、尺寸、alpha、像素、SHA256和容器目录核对；ICO包含16/20/24/32/40/48/64/128/256。Pillow独立解码、Apple iconutil解码和重复转换一致检查通过。macOS plist lint通过；Linux staging正确拒绝Mac二进制。详情见 packaging/VALIDATION.md。

## 修复和失败证据

工作目录保留此前失败日志：新测试 tuple index 类型和 Focusable import、测试借用、Clippy折叠/无效转换、未注册test observation、test window未激活造成focus callback未触发，以及迁移缓存中的编译期旧路径。修复测试启动激活后，生产focus callback的路径测试通过；未用跳过测试或忽略断言掩盖失败。

GPUI Kit当前Button snapshot未暴露disabled属性，因此测试不把None当作enabled/disabled证据；通过真实点击和生产方法验证无错误队列输入。架构扫描从Cargo运行时manifest目录读取，迁移后仍检查当前源码。

## 未证明的范围

- SSH夹具只回显并操作临时目录。它拒绝exec，所以监控显示失败原因及缺失值，不能代表真实Linux监控验收。
- 没有商业模型账号、真实服务器或客户数据；高级AI协议、凭据库和Agent仍未实现。
- macOS原生成功与图标容器校验不证明Windows/Linux构建、桌面显示、DPI、安装或签名；这些均待原生验收。
- 发布、签名、公证、安装器和GitHub远程操作均未执行。

## 本地证据索引

以下日志和截图留在ignored work，包含临时运行路径，不进入公开提交；哈希固定本次证据。源码/资源源图保留在仓库。

| Evidence | SHA256 |
|---|---|
| `work/integration-gate-12.log` | `6813781a91486fddeac460a460cf2df69131cd3c480bcb0bca661fd63b560c9c` |
| `work/native-build-3.log` | `6f1dfafbadffa04d29328ac85e6adcff7e4a1098b99cfe3631f4d6d95035d76b` |
| `work/native-evidence/03-ai-settings.jpg` | `5b5b139f91653caa77b4ef23a1958c61a9e8f731084469acf5b1d3344b9c49c7` |
| `work/native-evidence/04-host-fingerprint.jpg` | `21f9d42a9c2b673acb0a7b306ce2620aa27fff33102f48f29e299b119890defb` |
| `work/native-evidence/05-sftp-review.jpg` | `7cddb7dfd38f54a9682a25d8134b84343ffc8698123c1012cf53c151209934e9` |
| `work/native-evidence/07-ai-reply.jpg` | `7c7e14b4c3537260ad115b65bba7327153ab7e42b99e1bedb03c53e6c66c2108` |
| `work/native-evidence/08-editor-height-fixed.jpg` | `459ac20199d054f167e9923a3795adb9fe736b6d2ad479a0ba285a05d78b8fbe` |
| `work/native-evidence/09-finder-icon.jpg` | `4c80ce38b05d0a39512b298d402a26122b61f04d736e96dd464d90236b9da84f` |
| `work/packages/macos-native-20261003-3/package-manifest.json` | `378e55d1a6141cf47cb920f5babc2c755ad82515aaeb0b6d0c365f3d27daefd8` |

最新验证二进制 SHA256：`6d0af102fe3258e18d3891088422bf0ce3798a728611bfd36fcbd59ebf1e9cfc`。
