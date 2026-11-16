# Windows 归档诊断 CI 原件

本目录绑定源码 `76b8253acd9bfa62180563f482e851064bc1ea34` 的 Quality
`43806549649 / attempt 1`。该运行终态为 `completed/failure`，macOS 和 Ubuntu
成功，Windows 失败。本目录保留失败事实，不将后续诊断或本机交叉检查移签为修复。

`originals.tar.gz` 保存调查目录的全部 640 份原件，原始字节共 6,613,397，
压缩后 990,281 字节，SHA-256 为
`0a49de54fb50794af3fadd3501a2346f0a1ebcf46788357088af8c7d7cf6da65`。
完整逐文件路径、长度和摘要见 `archive-manifest.json`；`diagnosis.md`
为非作者调查报告的逐字节副本。包中包含三平台原始日志、45 次观察快照、
命令回执和来源快照，不需要保留重复的临时目录。

Windows app 实际结果为 `1320 passed / 8 failed / 2 ignored`；
macOS 为 `1339/0/2`，Ubuntu 为 `1340/0/2`。八项 Windows 失败均停在
`review-error`，类别 `io/other-redacted`，受限显示后缀为 `Some(123)`，
原子写入开始次数为 0。该显示后缀不能证明 typed OS error、NTSTATUS 或失败调用；
根因保持 `UNKNOWN`。八项安全断言的正文未修改，本次失败未到达这些正文断言。

在仓库根目录运行：

```sh
python3 -B docs/testing/records/2026-11-16-windows-archive-ci-evidence/verify_evidence.py
python3 -B docs/testing/records/2026-11-16-windows-archive-ci-evidence/verify_evidence.py --index
```

提交后可用 `--commit` 加保存本目录的完整提交 SHA 验证精确 Git 树。
验证器不向磁盘解包，检查全部 640 个成员的类型、路径、长度和摘要，
三平台日志的源码身份、原作者 manifest、报告副本，以及 18 份来源文件与
上述源码提交的 Git blob 一致性。预检及完整工程门禁的后续结果应使用各自的新记录。
