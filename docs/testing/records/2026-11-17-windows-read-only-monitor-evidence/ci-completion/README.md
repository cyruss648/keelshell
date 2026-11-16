# Windows 只读监控的精确 CI 终态

固定工程 `5d2df2fc5ac8566fe11182b7f4337076296dfa54`：
Quality **43972364934** 与 Windows monitor integration **43972365007**
均 `completed/success`。最后终态观察为 `2026-11-16T23:23:37.483469Z`，
即北京时间 **2026-11-17 07:23:37**；不是 CI 开始或封存时间。
观察器已自然结束、exit0、实际 reap，原进程 active0、pipes drained、无清理错误。

| 平台／job | 普通 Rust passed / failed / ignored | app passed / failed / ignored | session lib passed / failed / ignored | Python 主 discovery run / passed / skipped |
| --- | --- | --- | --- | --- |
| Windows 2025／133667528035 | 3394 / 0 / 57 | 1387 / 0 / 2 | 445 / 0 / 3 | 405 / 368 / 37 |
| macOS 26／133667527720 | 3407 / 0 / 59 | 1398 / 0 / 2 | 404 / 0 / 4 | 405 / 403 / 2 |
| Ubuntu 24.04／133667528119 | 3400 / 0 / 60 | 1399 / 0 / 2 | 401 / 0 / 5 | 405 / 403 / 2 |

各平台 **93** 个普通 Rust suite、**6** 个 rustdoc subrun，逐套声明数、
唯一逐名结果及摘要相等；rustdoc 各 **14/0/0**。
Windows 路由各 **9/0/1**，23 个 Windows app 回归在三平台均逐名 `ok`。
普通 Rust 包含 app／session／routing；过滤专项、预检、rustdoc 和 Python 不重复相加。
CI 没有另跑独立 `--list` 总库存，主 Python discovery 也没有逐名 `-v` 库存；
本机最终独立编译库存另在根封存中。

包装 Python 另为 **57 run**：Windows53通过／4跳过，macOS／Ubuntu各57通过。
macOS／Ubuntu 的内嵌 Python helper 各30通过，Windows未报告该 subrun。
三个完整 Rust quality gate 成功；Windows filesystem、archive codec、session
三个预检成功。macOS／Ubuntu 的真实标准 OpenSSH 步骤及回执保存成功，
Windows 对应步骤跳过。42个 Quality 步骤原样保存：
Windows11成功／3跳过、macOS10／4、Ubuntu11／3。

## 实际 Windows API 与认证路由 replay

direct job **133667526984** 的10个步骤成功；isolated SSH job **133667529467**
跳过。平台限于 Server2025／build26100／AMD64、Python3.15.2、
Windows PowerShell5.1.26100.33648。

artifact **14279316541** 的 ZIP 为 **46,825字节**，SHA-256
`b2afa96f9ada2ae38d8e1d21b8b05587db512da3b43f0ba4ae071aa0a2914af9`，
等于保存的官方 digest。34个唯一 ZIP 成员通过 CRC、与解压原件逐字节相等。
五个实际 cmd→PowerShell 命令与精确 Git 的生产装配相等，
原进程对象均 live 时保留、自然终止exit0、完整对象 census、
pending／active0、管道排空，无清理错误；fixture也已终止并 reap。
独立枚举覆盖3稳定接口、2固定卷，匹配4个自有双栈监听／绑定端点，
排除2个 connected-only TCP；工作集差值0。
CPU CIM和独立累计计数采样窗口不同，差值只作信息，不签精度一致。

该 job 的 Python **111 run＝108通过＋3跳过、0失败**，111个唯一结果名与摘要相等。
三个跳过项是负向 non-Windows 边界。早期
`completed-jobs-summary-r1.json` 仅记run111／failed0，未记跳过，原件不改写；
最终 `terminal-summary-r1.json` 补齐逐名结果和准确拆分。
作者及 Gauss 在 macOS 的111/0/0保持原执行范围。

指定测试 `captured_windows_api_frames_are_accepted_through_authenticated_monitor_routing`
实际 **1通过／0失败／0忽略，9 filtered out**，使用受控认证peer回放真实采集帧。
真实 Windows OpenSSH、默认远端 shell 和远端进程生命周期仍 **UNRUN/HOLD**；
API、capture replay或CI runner均不代签原生桌面、其他Windows版本／架构和发行。
Hypatia的102项离线独立复算、原失败及限定APPROVE保存在[交付封存](../delivery/README.md)。

## 原件与复算

`originals.tar.gz` **91成员／884,984字节**，SHA-256
`198e1d8c5f4fdaf8268f57fabeea52bc3a53c298d68b01317dabc484b8beadef`。
`manifest.json` 逐项绑定大小与摘要。包括221次有界只读GitHub请求回执、
去重保存的原响应、17次不同状态观察、最终run快照、四份job原日志及下载回执、
Rust逐名派生结果、34个Windows原件及原ZIP、观察／派生代码和外层终态回执。
请求、下载均exit0；没有手动CI重跑、取消或远端变更。

758个工程输入逐项与固定提交的Git blob相等，aggregate
`6759f672ae1efb83f6a400145109ee9bdc75ed192e2b7647813a252a1eded210`。
本次只重算原件，不重放测试或运行Cargo：

```sh
python3 -B docs/testing/records/2026-11-17-windows-read-only-monitor-evidence/ci-completion/verify.py
```

封存后实际 **exit0／reaped／2.423535s**，91成员、758输入，mismatch0。
成功回执 `ci-evidence-verify-r1` 同时保存在交付封存。
验证器核对最后GET与run快照、五个job、逐名Rust、Python计数、
原ZIP／digest、实际生产装配和精确Git输入，不将后续文档提交当作新工程CI。
完整产品、原生桌面、签名／公证及六目标发行保持开放。
