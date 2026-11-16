# ee3ad73 三平台精确 CI 终态

Quality `43880245383` 绑定 `ee3ad7320e90f40c9d777c500543cc6e909d0484`，
completed/success；watch `0/reaped/1893.751s`，没有超时、中断、取消或重放。
Windows `133343901451`、macOS `133343901229`、Ubuntu `133343901650`
完整 gate 均 success，macOS／Ubuntu 的真实 OpenSSH 均 success。

| 平台 | 普通 Rust passed / failed / ignored | app passed / failed / ignored | Python run / skipped | rustdoc passed |
| --- | --- | --- | --- | --- |
| Windows | `3212 / 0 / 46` | `1330 / 0 / 2` | `293 / 34` | 14 |
| macOS | `3225 / 0 / 48` | `1341 / 0 / 2` | `293 / 2` | 14 |
| Ubuntu | `3218 / 0 / 49` | `1342 / 0 / 2` | `293 / 2` | 14 |

各平台 92 个普通 suite 和全部 rustdoc 子运行的声明／唯一逐名结果／摘要匹配。
CI app 没有独立编译 `--list`；Python 只有完整 discovery 摘要，没有单例逐名 `-v`。
macOS／Ubuntu 后续 Rust 内的 30 项 Python archive helper 单列，Windows 无该段摘要；
这些 helper、预检与三个 controller 不重复累加。
Windows codec／session 分别有独立 41／332 名库存，与执行逐项相等；
45 名 Zmodem local 为 `43/0/2 ignored`。

七类实际 Windows tree 控制均保留原对象、身份先于 ACK、父 exit7、拒绝空／部分记录，
终态 direct reaped、active 0、pipes drained。native nested Job barrier 的两个原对象
关闭 admission 前后均 LIVE，late CreateProcess 被拒绝（Win32 1816）、marker 未执行，
两个原对象最终 TERMINAL，controller exit 0、无错误、完整回收。
新的实际 exclusive 文件反例由完整 discovery 覆盖，不填不存在的逐名回执。
本记录仍不等于真实 GPUI 桌面、完整产品、独立 peer 或发行验收。

归档共 36 成员、5,197,861 字节原文，压缩 858,818 字节，
SHA-256 `9d723bcfc899038daa62f0a592737cf69a080d3bca0548cf92b645ad0e991db2`。
含三个完整原日志、七次 API 读取（r7 是终态）、已完成 watch、派生 JSON、
原解析器和获取／解析诊断。运行中 Windows 日志 404 保持；首次 macOS 日志获取
被 gh 的终端转义保护拒绝，加入 `--allow-escape-sequences` 后原文保存到文件，
没有将转义输出直接显示为终端内容。这两次获取退出 1 不是 CI 失败或测试运行。

新汇总辅助器首轮错误地把 Rust 内的 30 项 Python helper 与主 discovery 合并选择，
因两份摘要而拒绝输出 macOS／Ubuntu 派生文件；诊断保持。
修正只把主 discovery 边界限定在随后 cargo fmt 命令前，旧 958 三平台重派生相等，
当前三个日志亦精确匹配。没有改工程源码、清理 target 或重跑测试。

运行 `verify.py` 核对全部成员、回执、最终 API、三平台计数和 Windows 控制，
并从原 25 成员／719 Git 输入绑定精确源码。实际 `0/reaped/6.699s`、零 mismatch。
它只在隔离临时目录运行已封存解析器并读取日志，不运行测试／Cargo 或正常程序。
新非作者 Parfit 已独立核对终态，限定 APPROVE，P0–P3 全 0；
没有 Cargo、测试、CI 查询或 target 写入，已关闭。
[复核与清理原件](independent-review/README.md)保留完整报告和实际命令，
后续回收 98 文件／7,481,351 字节，正常包六文件摘要与共享 target 目录身份不变。
不修改原 25 成员或 26 成员的运行中／执行 0 签署时点。

活动原件副本回收后，再从仓库内封存独立验证一次：
`0/reaped/6.802s`，全部重派生和精确源码绑定仍相等，测试重放 0。
两次实际验证的回执／原文分别保存在 `verification-r1.json`／`.txt` 和
`verification-after-cleanup-r1.json`／`.txt`；没有依赖已删除的活动原件。
