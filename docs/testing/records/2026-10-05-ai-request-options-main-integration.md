# API 请求选项 F 主树整合

主树基线 `df8cf8be4ea66f1186d511ef84c202709bd4fd4e`，候选作者基线 `23310ee13286adb488a451277addf49b05cd476b`。导入前主树干净，48 候选路径逐bytes/hash等于作者基线；非作者 F 最终封存完整读回后，应用其冻结 patch。导入后全部48文件等于冻结 F，中英文 README 同步更新。后续主树文档保留较新 MCP 进展。没有用 E 旧二进制作为新主树原生包。

[F 独立复核](2026-10-05-ai-auth-header-case.md)限定通过；E 大小写 P2、D 名称秘密 P1、C 草稿秘密 P1 与此前失败均保留。API 请求头、显式代理、临时/环境/加密引用、全目录已知秘密拒绝及元数据保存边界是本增量范围；对外 MCP 仍仅让外部智能体调用 KeelShell。

状态：主树完整 `scripts/check.py`、57项打包、新MAC15构建/标准双程序打包与产物输入核验通过。新原生 API 临时/环境引用及手动 Ask 已验证限定事实，中英文三主题的长文件人工拒绝已通过实际SFTP读回；原预期未满足、记录器失败及最小窗口未验证边界分别保留。见[API原生记录](2026-10-05-ai-request-options-native.md)与[文件审阅六组合](2026-10-05-mcp-file-review-native-matrix.md)。前一 df8 三平台源码 CI 通过范围见[同步CI](2026-10-05-mcp-grant-readiness-ci.md)，不验收本 F 增量；新提交 CI 另核验。

Windows/Linux 原生桌面、供应商完整 Agent/MCP、正式 Release/签名/公证和已安装更新仍开放。无遥测或覆盖既有安装。

## 根完整门禁与产物输入

`main-F-fullcheck-1` exit0，422.111秒；1146普通Rust、8rustdoc、6Python、11ignored，42普通/4doc harness。格式、严格workspace/all-targets Clippy、x.y和默认/2MiB控制器通过（future4624字节）。全部输入前后一致，118,596字节log SHA256 `9b2f1a14ff3a2bb6c61942644b6e659af8d07f7769a7c8dbab0b92fb09256714`。另57项打包测试通过。既有block0.1.6 future-incompatibility提示保留。

显式MAC15两步构建46.226秒通过，新app/MCP与loopback_fixture均actual arm64，实际minos分别15.0/11.0/15.0，动态依赖只读检查通过；258份Rust/Cargo输入与门禁、构建前后及当前一致。状态文档之后更新，不改变已检工程字节。全部记录的自身出生身份消失，零超时/信号，仅声明已记录身份，不扩大未观察后代清理。标准双程序打包和原生执行另续。

原版helper的52proof/53归档已根完整读回，NOT_READY与首Python3.9 ERROR保持；修订版v2的51proof/52归档作者26offline通过并根逐字节读回，全新非作者26offline与追加生命周期检查通过，85proof/86归档经根核验。修订包括完整行期限、请求总期限、startup读行容量/取消与已观察脱离PGID后代的private保护；bind纳入未提交的新Rust。其预审通过只提供新GUI启动前置，实际原生结果及记录器PTY修复单独记录。

新API与文件矩阵均完成非作者冻结证据限定复核，分别59proof/60归档与143proof/144归档，经根逐bytes/hash及全部tar成员读回核验。文件矩阵复核查看33张原JPEG及原回执，未发现新的已证P1/P2；没有另跑GUI或供应商。两份原生记录保留全部首次失败、未满足预期及尺寸边界，不用冻结证据复核扩大原生范围。两个增量工作区的author-F 34/35与review-F 656/657证明已根再次读回，待阶段提交后可恢复归档。
