# 95813a7 精确 CI 终态

Quality `43873806902` 绑定源码 `95813a76f0ec9b59fd043cd982e33cdc01dd69f8`，
completed/failure，watch `1/reaped/1576.244s`；未取消、未重跑。
Windows `133321520130`：filesystem `70/0/0`、core archive `41/0/0`、
session `329/0/3 ignored`，332 名独立编译库存与执行逐项相等；
45 名 Zmodem 为 `43/0/2 ignored`。完整 gate 停在 Python
290 run／1 failure／34 skipped，全量 Rust 未运行。

macOS `133321519808`、Ubuntu `133321520197` 的完整 gate 与真实 OpenSSH 均 success。
普通 Rust 分别 `3225/0/48 ignored`、`3218/0/49 ignored`，各有 14 项 rustdoc 通过；
app 分别 `1341/0/2 ignored`、`1342/0/2 ignored`。
CI app 证据是 `running N` 声明数与唯一逐名结果，不含独立 `--list`；
宿主显式 1343 名库存的签署另见原 93 成员封存。

归档 21 成员、3,747,526 字节原文，压缩 649,722 字节，
SHA-256 `b3aa1c027aef7d7cca6eefa38731e41d43275266c41103349d97e88a157e2f75`。
三平台原日志、最终 API 元数据、watch、库存／结果、r2／r3 解析器及诊断均保留。
macOS 请求沿用 `active-log` 名字，但响应实际含完整 OpenSSH 与 post-job cleanup，
终态 API 确认 success。r2 因过早以 stderr 的 Doc-tests 头切分普通 stdout 而拒绝输出；
r3 保持当前运行直到摘要，对 rustdoc 只记子运行、不猜所属 crate。
两平台各 92 个普通套件与全部文档子运行的声明／唯一结果／摘要均匹配。
没有重跑测试、改源码或清理 Cargo。

执行本目录 `verify.py` 可核对成员、重新派生 r3、精确 Git 958 的 719 输入及上述边界。
实际验证 `0/reaped/8.131s`、零 mismatch，测试重放 0。
本补充不改变原封存时新候选 Windows 执行 0，也不签署后续 `ee3ad73` 工装修订。
