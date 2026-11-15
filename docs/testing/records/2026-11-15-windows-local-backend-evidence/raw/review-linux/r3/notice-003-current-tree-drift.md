证据边界 notice：本次 Linux 结果只绑定不可变快照，不覆盖继续修改的工作树

本 notice 不报告 Linux 运行失败，也不重新打开 r2 的两项 P2。
`linux-final-r1/receipt.json` 为 PASS，252 passed / 0 failed / 0 ignored。
首轮只读核验已经确认 `source.tar` 的全部 586 文件与
`8a7523ec8a21d5a9439592b99b6c30390b1be9a2` 的 Git blob、
`source-manifest.json` 及 `combined-inputs-r1.json` 完全一致。

首轮核验附加检查当前工作树时，观察到父线程继续集成后的输入漂移：

- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/archive_transfer/local_windows.rs:28`
  冻结 SHA-256 `3d2a16199f53c0e96f26139716fb939a9914f9bb08d9838558bcacb3ff2c21a9`，
  当次读回 `b2598247a7ae0ef2c4e987c2c96ad913dc0114e622555daf5c1cc8f57b1a0414`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/archive_transfer/local_windows/tests.rs:7`
  冻结 `9bc3d82a11dd5b66e6b2e31116d48aa46f5b017f521556c37e0e3dd79b5504ba`，
  当次读回 `806877aaa91ea6aae15ee3537380c9c938d2337772f73a37a1804b33bf0c6c14`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-session/src/sftp/archive_transfer.rs:12`
  冻结 `c64257dcc46f927b02e169bbdab95c62341d874cd9408332aa339b00f65ae3f2`，
  当次读回 `fbe10992675decfa201f8a213d4b845a8b26b94eaa9bb42781d75dd1d4b8da0e`。
- `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/crates/keelshell-app/src/files/archive_tests.rs:8`
  冻结 `48c20ba23b1ee92be0aa8e8d1e7d7e900e5255f1c6757a7828278d82b64550ab`，
  当次读回 `2c8329d0e58d05b2179c2dd3b7053507aa090cea1b27f8645025741255667bce`。

影响：252 项结果可以签署精确的 8a7523ec 验证快照，不能改签正在变动的
当前源码或后续组合冻结。以上仅是当次读回，后续漂移以父的新冻结为准。
其中 archive_transfer.rs 新增 `cfg(any(windows, test))` 模块，不能只因文件名
含 Windows 就概括为 Linux 输入未变。本 reviewer 不审查或运行这些后续改动。

首轮核验自身因附加的当前树一致性断言中止，exit 1 / waited_and_reaped=true，
不是 Linux 执行失败。原件保留于本目录的 `audit-r3.command.json`、
`audit-r3.stderr.log` 和 `audit-incomplete-r3.json`。
后续 r3 附录继续完成对指定不可变快照的限定核验，并单独保留此边界；
不会将首轮中止、旧 r1 HOLD 或 r2 APPROVE 改写成另一种历史。
