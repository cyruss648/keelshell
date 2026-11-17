# MCP 审核滚动的精确证据

结论和范围以[测试记录](../2026-11-17-mcp-review-scroll.md)为准。
最终工程为 777 输入 r2，aggregate
`5b5c1d2887d21ecf7e721d212e81f9f64acfe574b6ba32689dd9c6ad0cbb1e12`；
正常原生包绑定 r1，774 项相同、三个测试文件不同，生产输入保持相同。
原生滚轮 P2／根因 UNKNOWN、紧凑窗口、辅助技术和其他平台 GUI 继续开放。

## 原件与维护入口

`originals.tar.gz` 保存 555 件、19,111,591 逻辑字节，压缩后 9,478,543 字节，
SHA-256 为
`07cc1659cf3164e8b24c0a0fed67f70dcafe3a8bd705af8fedb08f7174cb19ee`。
`manifest.json` 对每件记录原 source、archive member、长度及 SHA；
不解包或执行包内脚本也可逐项复核。原件中的旧绝对路径只是现场定位，
展开副本回收后按 source → member 映射读取，原日志行号不变。

| Archive 前缀 | 件数 | 内容 |
| --- | ---: | --- |
| connected-native | 107 | 准确基线正常包、原生按钮裁切失败、两种模板复制解析、退出及资源回收 |
| layout-controls | 9 | 两版 view、旧实现与仅分离正文对照、精确源码恢复 |
| viewport-native | 76 | 首版原生滚轮失败、错误实例排除、专项编译／断言失败及滚动条对照 |
| test-author | 5 | 回归作者原稿及静态检查 |
| diagnosis-review | 3 | 首版独立 HOLD、未隔离的根因及短 diff 缺口 |
| scrollbar-native | 83 | 15 组未编辑 JPEG／AX、正常包绑定、真实文件读回／命令拒绝和回收 |
| source-review | 66 | 原 P3、错误预编译路径失败、准确 r1 MCP40、最终专项4及关闭 P3 的补审 |
| native-review | 11 | 逐图审查、原输入归属 P2、三测试差异补审与唯一开放滚轮 P2 |
| fixture-author | 28 | 首轮门禁原件副本、测试前置修订、作者失败、最终模块与生命周期结果 |
| fixture-review | 5 | 新非作者两个 Python 模块独立34通过、生产不变和真实回收 |
| monitor-ci-seal | 30 | 旧监控 CI 封存辅助原件、工装失败与精确来源摘要 |
| integration | 132 | r1／r2冻结、两轮完整门禁、最终96编译库存、正常构建及旧CI推送／回收 |

43 件、447,531,402 字节明确排除：三个正常包、一次性运行库及 reviewer 复制的
测试二进制。排除项仍保留逐文件摘要与理由，未将它们作为 Git 原件上传。
当前修订正常包 `work/mcp-scrollbar-native-20261117/package-r1/KeelShell.app`
继续保留维护；旧失败包和复制测试二进制仅在核对封存及摘要后回收。
公钥认证的临时私钥已按原回执删除，没有把私钥、MCP临时能力或主密码纳入封存。

## 限定结果

完整工程门禁自然 `0/reaped/782.087748s`，Rust 3469／0／68 ignored，
15 rustdoc，Python 主526 run／3 skipped；fmt、Clippy、依赖政策及三个 controller通过。
96 普通编译库存逐名／状态／计数相等，前后摘要相等，777 输入前后相等。
首轮完整门禁失败及全部先前原生失败保留，不重写为最终成功。

原生只签正常包的显式拖动路径：完整 diff 首尾、一次文件批准后独立
6628 字节全等读回、另一命令拒绝、三主题双语宽窗、撤权及自然退出。
九次工具调用无错误；没有供应商／模型请求、安装、其他平台桌面或完整发行。
原生滚轮仍未观察到位移，显式滑块成功不关闭这个 P2。

源审及夹具非作者最终 P0–P3 全0；原图非作者限定 Hallmark ship，
开放 P0=0／P1=0／P2=1／P3=0。所有本批作者和 reviewer 已由父级关闭。
预编译、离线 Python、正常包与最终 workspace gate 的版本各自独立记录。

## 只读核对

仓库根目录执行：

```sh
python3 -B docs/testing/records/2026-11-17-mcp-review-scroll-evidence/verify.py
python3 -B docs/testing/records/2026-11-17-mcp-review-scroll-evidence/verify.py --commit FULL_40_HEX_COMMIT
```

默认逐字节复核 archive、成员、工程冻结、完整门禁、库存、正常包输入差异和
原生客户端／应用回收回执。`--commit` 进一步核对全部 777 工程 Git blobs
及本目录顶层交付文件；不会以旧 HEAD 或仅源码摘要代签编译产物。
无产品重跑、网络、Git 写操作、解包执行或 target 读取／修改。
准确提交、普通推送、三方 SHA 与后续展开副本回收另保存为交付回执。
