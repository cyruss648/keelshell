# Windows 目录发布句柄与 Kit 证据原件

共 237 份成员，其中 232 份原件、
5 份派生文件，合计 8904728 未压缩字节。
压缩包 1763368 字节，SHA-256
`41701965edfeb7c3fa6a630a9085d5e83cf889f00ca0bb2728f6411ea347a0e0`。`archive-manifest.json` 逐文件记录路径、长度、
SHA-256 和原件来源；`derived/` 明确区分派生重构说明。

最终选择为 `batch/inputs-freeze-r6.json`、`batch/source-r6/`、
`batch/full-gate-r4.json`、`batch/reviewer-tunnels-r6/summary.json`。完整工程输入
719 项，SHA-256 `f0f64a6629b2441d63eef1e801189eaef69dd848aea0039eef4d09ff143b5164`。
只有该最终输入自身成功的完整门禁及相同输入的最终评审参与最终绑定。

历史完整门禁失败保留原件，不能被后来的通过改写：

- `batch/full-gate-r1.json`：输入 `r3`，控制器 exit1，107.954s。
- `batch/full-gate-r2.json`：输入 `r4`，控制器 exit1，58.955s。
- `batch/full-gate-r3.json`：输入 `r5`，控制器 exit1，389.376s。

`batch/` 保留全部历史冻结、r3/r4/r5/r6 及后续所选快照、真实专项、
locale 两次 green 及 exit101 red、故障线程 crash frames 和分轮评审。
stack repro 控制器 exit250；lldb 控制器 exit124/timed_out=true，
进程全部回收。修订专项的真实重编译 `1/0/0` 和 ai_settings `79/0/2`
单独保存，不代替完整门禁。dependencies-live-r3 exit0 为新事实，旧 UNKNOWN 不变。
环境版本只引用 environment-r4 原始回执，不据此声明官方最新 Rust。

r2/r3 旧批准的相同输入门禁条件、r4/r5 的静态范围与组合 HOLD 保持原文。
最终评审自己的组合条件按最终冻结和门禁核对，不移签旧批准。
r1 测试是在修订后重构并按作者冻结 SHA 核实的字节，不称现场独立快照。
每轮完整输入从基线 `4d72e1e592bdd6194f6eadca3a6a760192b5fe82` 加该轮快照或 r1 重构文件恢复，
路径映射见 `derived/input-bindings.json`。r4 无父清单时仅在 `derived/`
计算 snapshot-r4 的长度和 SHA；已有父清单也保留并核对。

`previous-delivery/` 补存前批交付、精确 CI、index/committed 绑定及回读原件。
前批已有归档中的 70 份副本不重复。
验证依赖仓库内前批持久归档和基线 Git blob，单独复制本目录不足以验证全历史。
`kit-author/` 按 DELIVERY 清单保留作者原件及 DELIVERY 本身；
downloads/upstream/diffs 第三方包不重新分发。Microsoft HTML 仅保留已有来源
URL、抓取长度与 SHA 元数据，全文不重新分发。工具不自动清理目录。

从仓库根目录执行以下只读校验，无参数等同于 --current：

```sh
python3 -B docs/testing/records/2026-11-16-windows-directory-rename-evidence/verify_evidence.py --current
python3 -B docs/testing/records/2026-11-16-windows-directory-rename-evidence/verify_evidence.py --index
python3 -B docs/testing/records/2026-11-16-windows-directory-rename-evidence/verify_evidence.py --source <完整40位源码SHA> --evidence <完整40位证据SHA>
```

验证器默认读取 manifest 的最终选择；也可同时传入四个 `--final-*` 参数比对。
源码 SHA 与证据 SHA 独立，支持 `--index --source`；缩写 SHA 不接受。
全部成员在内存核验，不解包、不编译、不执行测试。
前批 Windows 实际为 63/4/0；本批 Windows 原生执行仍 0，期望库存 70 名。
这不代表新正常包、安装、跨平台桌面、IME、辅助技术或完整产品验收。
