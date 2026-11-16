# 编辑器备份完整读回：原件与复算

本封存绑定基线 `7058925330b96161a25142fb5bee94fa24aa5117` 加当前功能差分：
727 工程输入，aggregate SHA-256
`78d8a0c0000ce0afb661d8af0c6712a266cd44fa168b801d37c1672c39ddc4c2`。
`originals.tar.gz` 含 206 个成员，1,367,770 字节，SHA-256
`3128e0fef41dab3293f674ccb39ea3d1746eba37ed171b8ffc3cdb287c2ac5ef`。
`manifest.json` 逐项保存成员名称、字节数与 SHA；不含测试二进制、正常应用、
密钥或 target。上一功能批的三平台 CI 原件单独保存在
[监听列表 CI 终态](../2026-11-17-darwin-tcp-listeners-evidence/ci-completion/README.md)，
本封存不重复收录，也不把旧 CI 移签为当前功能。

从仓库根目录执行：

```sh
python3 -B docs/testing/records/2026-11-17-editor-backup-readback-evidence/verify.py
```

提交后可加 `--source` 和完整 40 位提交 SHA。验证器直接读取该提交的 Git blob，
核对全部成员及 727 项工程输入，并从原日志重新派生完整门禁；不会重编译或重放测试。
提交前工作区验证实际 `0/reaped/0.191206s`、零 mismatch。
原始运行事实、边界及交付补充见[测试记录](../2026-11-17-editor-backup-readback.md)。

## 保存的主要原件

- `inputs-freeze-r*.json`、门禁前后和正常构建后快照；准确 Cargo 产物、
  各轮独立库存、实际门禁 Running 路径和二进制摘要。
- `full-gate-r1.*`：实际 `0/reaped/701.468731s`。92 个普通套件为
  `3307/0/52 ignored`，其中 app `1372/0/2`；14 rustdoc；
  Python discovery 293/2 skipped，archive helper 另列 30。
  自定义 controller、readiness、observer I/O、small-stack 均通过。
  `gate-summary-r1.json`、`gate-suites-r1.json` 及解析器支持独立复算。
- r4 新 session30/app5 的逐名通过；`openssh-r1/` 和外层回执记录真实标准
  OpenSSH `18/0/0`、八份独立原字节备份、私有 mode、继续 SSH 和完整资源清理。
- `source-review/` 与 `function-review/` 保留初审 REQUEST_CHANGES 及 r4 追加
  APPROVE，P0–P3 全 0；功能追加独立 app/editor `88/0/0`，准确库存1374。
  源码审独立测试执行 0；功能审使用父级 prebuilt、独立编译 0。
- 初始编译、session fixture、app staging/取消、长触发行、tooltip 移入失败，
  以及错取旧二进制导致 running0 的原日志／回执全部保留。
  决定性 tooltip-negative-r5 是实际 running1/fail1/exit101，移入后详情消失。
  reviewer 外层文档变化诊断与真实测试退出 0 分别保留。
- 官方依赖查询的超时、失败与 r3 成功，37 包／113 声明；
  两次正常 build 的各自输入、r4 precommit 包结构和六文件摘要。
  build.commit=null 的旧包不能声称来自尚未创建的提交。
- `native-availability-r1.json` 保留当时锁屏、自动解锁失败的实际观察，
  GUI 操作／截图／启动／安装均为 0；`agents-closed-r1.json` 记录三位自有
  agent 已关闭、没有新建用户 chat／worktree／分支／容器。

备份读回是有界观察，不承诺远端 CAS、持续身份锁、owner/ACL 保留或崩溃耐久性。
完整产品、原生三平台、精确提交 CI 和发行安装仍按各自证据验收。
