# ADR 0126：远程编辑 Unicode 编码外壳

日期：2026-11-14。状态：实现与作者工程门禁通过；独立复核待完成，真实桌面验收仍需分别记录。

## 背景

远程文件编辑器原先只保留 UTF-8 BOM 与换行策略。实际运维文件中仍会出现带明确
字节序标记的 UTF-16 LE/BE 文件；把它们误判为二进制、按错误字节序写回，或在
保存审核中显示乱码，都会破坏审核式文件工作流。

编辑器正文应继续使用统一的 Rust `String`，使查找、替换、patch、三方合并和
GPUI 输入不依赖远端编码。远端原始字节、文件头、字节序和完整读回仍是保存授权
边界的一部分。

## 决定

`keelshell-core` 使用无依赖的 `TextDocument`、`TextFormat` 和 `TextEncoding`：

- UTF-8 可以带或不带一个 BOM；UTF-16 只接受明确的 `FF FE` 或 `FE FF` BOM，
  分别表示 LE/BE。没有 BOM 的 UTF-16 不猜测。
- UTF-32 LE/BE BOM 在 UTF-16 前缀判断之前拒绝，奇数字节、孤立代理项和非法代理
  对也拒绝。NUL、无效 UTF-8、原始字节、解码后的 UTF-8 正文和最终保存字节分别
  受现有大小边界保护。
- `TextFormat::prepare_save` 以观测到的编码、字节序、BOM 和换行策略生成唯一的
  出站字节。UTF-16 使用严格的 `encode_utf16`，不会把替代字符写回。
- `TextFormat::decode_body` 用已观测的外壳解码保存结果。这个 API 保留 BOM-less
  UTF-8 文件开头的真实 U+FEFF 正文，因为它在字节层面与 BOM 无法区分；通用
  `TextDocument::decode` 仍负责首次观察并按文件头解释格式。
- 格式兼容性包括编码、BOM 和换行策略；无换行与 LF 继续共享 LF 插入策略，
  编码状态不会通过 `retain_insertion_policy` 跨格式迁移。

app 的 worker 将阻塞读取、解码、合并和写回放在 UI 线程之外。保存和合并操作把
观测到的 `TextFormat` 随审核身份传给 worker，避免在 UTF-16 或开头 U+FEFF 的
情况下重新从原始字节猜测格式。成功发布后的完整读回只从结果观察换行策略，
再与审核时的编码和 BOM 外壳合并；因此 BOM-less UTF-8 开头的真实 U+FEFF 不会
在读回时变成格式 BOM。差异预览也先用同一外壳解码两侧正文，再计算逻辑文本
差异。审核正文显示解码后的完整 Unicode 文本，目标发布后仍逐字节完整读回；
旧基线、备份和未知结果隔离合同不变。

session 的编辑器写入入口重新验证受支持的 Unicode 文本和 1 MiB 边界，并要求
替换内容与被审核文件使用同一编码。UTF-8 的 BOM 状态不在 session 层单独强制，
因为 BOM-less 文件开头的 U+FEFF 与 BOM 在原始字节中不可区分；app 的已观测
`TextFormat` 负责生成精确字节。

## 后果

UTF-16 LE/BE 文件可以在同一远程编辑、合并、审核、备份和完整读回路径中处理；
普通 UTF-8 行为和既有权限、路径、SFTP、MCP 预算不变。MCP 仍保留自己的常规
UTF-8 读取契约，不因桌面编辑器扩展而扩大 AI 工具权限。

编码转换没有被做成自动格式菜单，也不会猜测无 BOM 文件。真实桌面上的输入法、
辅助技术、Windows/Linux 窗口和客户 SSH 仍需平台验收；GPUI/SFTP 回归只证明
受控本地联调路径。
