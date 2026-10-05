# MCP 重授权测试的真实操作观察

基线为 `a909609c2bf64455ad48ec6c1704bdd68ec3df9c`。首次文件提案 [Quality37306263578](https://github.com/cyruss648/keelshell/actions/runs/37306263578) 在 macOS 的 `mcp_file_failed_regrant_releases_finished_preparation_and_keeps_old_scope` 点击授权后立即检查 `mcp.busy` 时失败；Linux/Windows 同名测试通过。原三平台日志和失败完整保留。本机精确基线单项运行通过，因此不能据此确认原 CI 的具体事件或完成时序。

候选只改两份测试源码，生产、依赖、Cargo.lock、工具链及产品限时不变。真实 SFTP fixture 增加按精确路径持有 REALPATH 响应的 RAII 屏障；拥有者 Drop/显式释放唤醒原处理，clone 不会释放别人的屏障，旧处理不会占用新一代屏障。兜底仍有界，不持有文件系统 mutex 等待。

失败重授权测试保持真实 GPUI 按钮入口及原有提案流程：先捕获尚未交付的真实文件准备，再点击对不存在目录的授权；等待服务器确实收到该路径 REALPATH 后，检查生产 busy 和旧准备名额；释放响应，等待生产失败处理结束，再交付旧完成帧。原界面 7 秒、产品根校验 5 秒保持；新 fixture 兜底 10 秒不扩大产品等待。旧授权、计数归零、旧请求错误、无写入和后续仍可提案的断言保留，不能以按钮点击或固定睡眠代替操作开始。

作者限定检查通过：三个屏障测试覆盖精确路径、clone/Drop、释放先于 poll、代际隔离及到期所有权；19 项 MCP GPUI/真实 TCP SSH/SFTP 回归通过。首次候选访问私有 revision 字段导致编译失败，原日志保留；去除新增的私有字段访问后，通过已有真实路径观察和原 stale 完成断言验证，不改生产可见性。

作者完整 `scripts/check.py` exit0，354.179 秒，1083 普通 Rust、8 rustdoc、6 Python、43 个标准 Rust harness，11 ignored 不计执行；格式、严格 workspace/all-targets Clippy、x.y、默认/显式 2 MiB 控制器通过，控制器 future 仍为 4624 字节。全部工程输入前后一致。记录文档在门禁结束后添加，最终 Rust 文件仍与该次门禁逐 SHA 一致。

新的非作者限定复核 PASS，无已证实限定 P1/P2：3 app 屏障、3 session 屏障、19 MCP GPUI、105 TCP SSH/SFTP、6脚本、格式/严格 workspace all-targets Clippy/x.y/diff 通过。Session 屏障3包含在105内，不重复计覆盖。私有 disabled 点击反例实际验证 entered0、busy=false、旧授权和准备保持、零write/原字节不变；它不证明原CI的具体触发原因。首版私有字段编译失败保持，私有探针移除后480工程输入逐SHA恢复，再次19MCP/格式/严格Clippy通过。

非作者99proof、100归档成员由根逐字节/摘要核验保存，manifest SHA256 `b17457c13991d8099c7015f54c21e3092725519c4904749c5f8e913003001330`，归档 SHA256 `10c4050d02a178193eba63a72be9d73a4420836f274ab1a7aecebae4f8ce9844`。15项记录身份消失、TMP删除，只声明已观察身份。根已导入精确两份Rust及本记录。主树完整 `scripts/check.py` exit0，439.533秒，1083普通、8rustdoc、6Python、43标准Rust harness、11ignored；格式、严格workspace/all-targets Clippy、x.y和默认/显式2MiB控制器通过，future4624字节。全部输入前后一致；最终状态文档在门禁后更新，生产/两份测试Rust/依赖/锁/工具链仍与门禁逐SHA相等。原log112778字节，SHA256 `7ce6173cefa54374f433b80bc6f5bfa921b1664238251a09a0990af3c482a5e9`。

作者与独立复核工作区已可恢复归档；根保存副本核验不变，实际工作目录和cache已消失。主树门禁记录的自有进程出生身份已消失，无超时/信号；私有TMP确认空后删除，仅声明已观察身份的清理。

原 CI 不追溯标为通过；新提交后的三平台结果另行核验。该候选没有新 GUI 构建或原生窗口验收，也不扩大既有八工具开发包、供应商、正式 Release 或安装更新的证据范围。

新 df8 提交三平台源码 CI 已独立核验且根完整读回通过，范围与跳过项见[CI记录](2026-10-05-mcp-grant-readiness-ci.md)。原 a909 失败保持，不将新通过追溯为原通过。
