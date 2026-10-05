# 隔离代理夹具三平台 CI 与 Windows 实际对照

日期：2026-10-06。精确提交 `e821d5ed350379fbd08426d99b2658611a8c0d6f`，parent为3f26诊断提交；[Quality37346734818](https://github.com/cyruss648/keelshell/actions/runs/37346734818) 的三个作业全部success。独立复核及根读回确认run/job API与三个实际checkout精确相等，测试源仍与已复核的SystemRoot候选相等。

| 平台 | 普通 Rust | 文档测试 | 控制器 | 打包通过/跳过 | 脚本通过/跳过 | OpenSSH |
| --- | --- | --- | --- | --- | --- | --- |
| macOS 26 | 1155通过、0失败、11 ignored | 8通过 | 默认及显式2MiB均完成，future4624字节 | 57/0 | 6/0 | 9具名通过 |
| Ubuntu 24.04 | 1155通过、0失败、12 ignored | 8通过 | 默认及显式2MiB均完成，future4624字节 | 57/0 | 6/0 | 9具名通过 |
| Windows 2025 | 1136通过、0失败、11 ignored | 8通过 | 默认及显式2MiB均完成，future4968字节 | 53/4 | 5/1 | 步骤跳过 |

三平台格式、严格workspace/all-targets Clippy和x.y策略均实际通过。原73项跨平台行为在两个Unix作业全部具名ok，Windows加专属对照为74项；新九项测试夹具行为分别实际通过，Windows额外一项对照通过。这些均已包含于上述普通测试数量，控制器标记与单独OpenSSH步骤不重复加入普通数量。供应商CLI opt-in保持ignored，不计供应商验收。

## Windows 实际对照

Windows原日志第585行实际写出固定短回执，下一行paired test实际ok：

```text
Windows proxy environment control: cleared=provider-init-10106; explicit-SystemRoot=real-http-completed; both-reaped-and-EOF
```

精确测试源码在写出回执前要求同一个真实child：清空路径必须实际非零退出并捕获10106；显式传递路径必须成功并出现仅在三次真实代理HTTP请求、认证/路由/头/脱敏与origin零访问断言全部通过后产生的标记。两次均要求reap和stdout/stderr EOF；原单次八秒总期限、容量与收尾机制保持。回执不打印SystemRoot值、系统路径、成功child PID或完整输出。

根独立取得的Windows原job日志206722字节、SHA-256 `9e07c2f3e3161d744ee34bd85ae74127e6a8eed63c5f1613556c972f8378855e`，与独立复核的原始/最终/全ZIP对应aggregate逐字节相同。当前runner的配对结果证实了本次最小环境修正；不能追溯确认旧c7被抑制的child错误原因。此前[c7失败](2026-10-05-ai-request-options-ci.md)及[3f26实际10106失败](2026-10-06-ai-proxy-fixture-readiness-ci.md)均保留。

## 原材料与验收边界

完整run ZIP328967字节，SHA-256 `a22c3b927fcfbccf8dd013980c182faba15b44a8748e20a85161c29ac9367925`，35成员CRC/字节读回通过。Unix两份OpenSSH产物的API run/head、ZIP长度、digest、CRC和全部成员均核验，九个实际测试及五个步骤均退出0。原清理回执分别记录61/65个累计kernel-birth身份、known owned/observed stopped及临时目录删除；这是原回执核验，不是审查者观察runner进程，完全未观察的脱离后代超出证明。

独立自包含证据173 payload /174归档成员已冻结并经根全量逐bytes/hash读回。manifest SHA-256 `57948ee98ca07f2916d618fe43958e77266e25cd5f19e931799f8fd3cdfa39f6`；归档1200110字节，SHA-256 `9feeb73263192c03489d8255afaf1eaed32a72ba8abe71d982515640ef397c5b`。私有辅助核验器首次文件名映射失败及修正后两次成功保留，不改变CI实际状态。

主树已从c7精确快进到e821并保存恢复最新八份文档，258工程输入与候选完整门禁相等；根完整整合门禁370.567841秒通过1155普通+8doc+6脚本与严格检查/两控制器，258输入前后相等，尚未推送main，见[整合记录](2026-10-06-ai-proxy-fixture-main-integration.md)。此源码CI及测试夹具修正不构成新桌面、供应商、生产服务器、Release或安装更新验收；本地Ask过程显示生产候选仍独立复审，尚未整合。
