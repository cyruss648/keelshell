# 精确交付与 Windows 测试编译补全

本补充保存已推送工程 `f810f674c57b2c25dac5b987d77bf38656e3af56` 的实际
提交／push／三方读回、206成员／727输入精确 Git 复算、正常包来源及清理结果；
并保存该提交的 Windows 编译失败和一行测试 helper 修复。
成员、字节数与 SHA-256 以 `manifest.json` 为准，不含应用或测试二进制。
主功能原件仍在[上一级封存](../README.md)，未覆盖初审、旧失败或运行事实。
本补充含60成员，202,887字节，SHA-256
`acdab5bcd75bbb8eabc42117859178cf6e954c12591b3e958f64dfeba9b4c58c`。

修复冻结 r5 仍有727工程输入，aggregate
`5c93cae03daaf370227820a815ad969623a0fb6bef1afb0d6e5362f09816528b`。
只有 `crates/keelshell-session/src/sftp/archive_transfer/local_windows/tests.rs`
增加一个明确的 `EditorBackupUnverified` 固定标签分支；其余726输入逐项相等。
生产错误、传输、app、AI、主题和依赖均没有随此修复变化。

```sh
python3 -B docs/testing/records/2026-11-17-editor-backup-readback-evidence/delivery/verify.py
```

可加 `--source` 和本次修复所在提交的完整40位SHA，直接核对 Git blob。
验证器检查原件、727输入、准确一行差分和session逐名库存／结果，重新派生
限定门禁，不编译、不重放测试。上一级验证器应使用 `--source f810f674c57b2c25dac5b987d77bf38656e3af56`
验证其原727输入；不能要求它把后续 Windows 修复视为原冻结。

## 实际结果与边界

- `feature-readback-r1.*` 在 `2026-11-16T18:32:32Z` 读回本地、tracking、实际远端
  均为 f810f67，工作区干净；普通 push，无强推。
- `source-binding-r1.*` 是精确 f810f67 Git 复算，`0/reaped/6.981018s`、零 mismatch，
  没有测试重放。
- `package-committed-manifest-r1.json`、`normal-package-committed-r1.json` 绑定 f810f67。
  六文件摘要完整，全部应用文件与主封存 precommit 包逐字节相同，只改变 manifest
  的来源提交；打包／arm64／minos15.0检查通过，没有启动、安装或签名。
- `current-ci-*.json/log` 是 Quality43929740313 的实际快照及 Windows133518648598
  原日志。Windows filesystem70/0/0、archive41/0/0通过，session预检编译
  E0004 后未运行库存、session或完整gate。快照时另两平台仍运行中；
  不补签它们终态，也不把后续修复写成旧run成功。
- `windows-fix-gate-r1.*` 实际 `0/reaped/26.865652s`，依赖策略、fmt、严格Clippy、
  diff通过；准确 session 库存343与r4及339/0/4实际执行相等，新backup30全ok。
  此处实际 binary 路径和SHA单独保存，未复用不同features的旧路径。
  Windows条件编译仍须在修复后精确runner确认。
- `windows-review-r1/` 的新非作者Banach为APPROVE、P0–P3全0，仅签署这次
  固定输入的静态窄复核；独立编译、测试和Windows执行均为0，不签并行及
  后续docs。已实际关闭，至此四位自有agent均关闭，关闭状态单独保存。
- 最初派生因helper stderr同行前缀少读3项而退出1；初版脚本、失败、修订后
  343名逐项派生结果全部保留，不重放成功测试。
- 补充验证器初次比较因内存tuple与JSON数组表示差异退出1；改为相同JSON表示
  后复算，逐名／计数断言保持。`verification-diagnostics/` 逐字节保留初版及
  失败回执／输出；原60成员封存未改，不为验证器问题重放测试。
- `cleanup-command-r1.*` 保持exit1：已按Git验证删除242文件后，未封存helper
  挡住目录删除。`cleanup-command-r2.*` 恢复为0，242文件／70,086,612字节
  缺席事实与逐项来源相符；当前正常包6文件与先前来源回执相等。
  首轮没有完成target前后比较，恢复轮才有目录身份前后相等，二者不混同。
  遗留helper已纳入本补充原件，待精确Git核对后回收。

完整产品、FILE-05/M4、原生macOS/Windows/Linux、供应商及发行仍开放；
正常包结构、loopback、headless和编译均不代签桌面验收。
