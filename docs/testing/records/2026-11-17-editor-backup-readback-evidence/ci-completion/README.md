# 编辑器备份修复的精确 CI 终态

Quality `43934041540`，源码 `a1a97b3b2ee0f0ccf91ea53ca96623aa8867fe8d`，
Windows／macOS／Ubuntu 三平台均为 `completed/success`。
watch 为 `0/reaped/428.496959s`，没有活动 watcher。
Windows session 预检与完整 gate 均成功；原 `43929740313` 的编译失败及取消
状态保持在原记录，不重跑或追溯改写。

| 平台 | 普通 Rust passed / failed / ignored | app passed / failed / ignored | rustdoc | Python run / skipped |
| --- | --- | --- | --- | --- |
| Windows | 3294 / 0 / 50 | 1361 / 0 / 2 | 14 / 0 / 0 | 293 / 34 |
| macOS | 3307 / 0 / 52 | 1372 / 0 / 2 | 14 / 0 / 0 | 293 / 2 |
| Ubuntu | 3300 / 0 / 53 | 1373 / 0 / 2 | 14 / 0 / 0 | 293 / 2 |

各平台 92 个普通 suite 及 rustdoc 的声明数、唯一逐名结果与摘要相等。
CI 没有独立 app `--list` 库存，不补写该证据。macOS／Ubuntu 的标准 OpenSSH
步骤成功，Windows 不运行该步骤。本 CI 不包含后续 Darwin UDP 工作差分，
也不代签原生桌面、正常应用包、安装或发行。

`originals.tar.gz` 的 27 成员保存状态、watch、完整原 job 日志、解析工具与
派生计数、原 727 输入冻结及精确 Git 绑定回执。`manifest.json` 包含逐项摘要。
验证只读原件与指定 Git blob，不重放测试：

```sh
python3 docs/testing/records/2026-11-17-editor-backup-readback-evidence/ci-completion/verify.py
```
