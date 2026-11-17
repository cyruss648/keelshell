# 本地转发生命周期：限定工程证据

本批769工程输入r2 aggregate：
`83e2e1b00ba4d373a407030c79538f8a5039347931236344315ad8b70844a5b7`。
设计、实际命令和最终交付状态见[测试记录](../2026-11-17-tunnel-transport-lifetime.md)。
完整产品、原生桌面、Windows真实OpenSSH forwarding与发行仍开放。

`originals.tar.gz`保存271成员／5,690,677逻辑字节，压缩后1,241,738字节，
SHA256 `ea318ba1a4ad23a7a8708215a8fb2b373d06f6fbf748750a3a775e49110fbb47`。
[manifest.json](manifest.json)逐件保存长度、摘要及原work路径映射。

- `parent/`：r1/r2冻结、原Clippy失败、完整门禁r2、95普通二进制库存、
  新16项的精确最终库存、两次实际OpenSSH6项与清理、完整依赖新快照、
  正常旧包／target保护信息及本批5位agent关闭观察。
- `first-review/`：Pauli的REQUEST_CHANGES、唯一P2、45项Python及原源码快照。
- `final-review/`：Curie对r2限定APPROVE、P0–P3全0、45Python与16项预编译复跑、
  原件离线复核和输入无漂移检查。不是独立重编译或SSH重跑。
- `previous-delivery/`：前批最后12份提交／推送／读回及回收回执，方便回收原剩余目录。

最终完整门禁为 `0/reaped/845.590925s`：普通Rust3435/0/68 ignored、
14rustdoc、Python主526 run／3 skipped；三个controller gate通过。
95普通套件的逐名结果、精确`--list`及二进制前后摘要相等，
库存检查 `0/reaped/1.746323s`。两个自定义controller不支持普通库存，
未向其发送`--list`；以实际门禁执行为准。

封存初稿误选两份reviewer复制的预编译二进制。它们逐字节等于持续保留的
target原件，已核对后回收2文件／63,816,528逻辑字节；
最终归档仅保留执行日志、来源与摘要，未把测试二进制提交到Git。
`parent/initial-archive-selection.json`保留初稿选择和摘要，
`prebuilt-copy-cleanup.json`保留源／副本核对。工程输入及产品结果未因此改变。

Git绑定通过后，原work副本才按完整文件集和摘要回收。原work路径只是出处，
不作为未来维护入口；持续保留的唯一target和正常旧包也不能替代新原生验收。
前批精确CI的7原始ZIP和非作者复算另存于
[CI终态](../2026-11-17-independent-zmodem-peers-evidence/ci-completion/README.md)。
