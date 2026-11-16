# 命令输入框双向焦点与精确 CI 证据

本目录保存 205 份无损原件，原字节 `15,646,519`；
结论从[本批记录](../2026-11-16-command-composer-focus.md)读取。
`archive-manifest.json` 逐项绑定源字节与 gzip 字节。`omitted-artifacts.json`
登记 8 份可重建暂存程序、编译 observer 与缓存，未重复提交二进制。

- `raw/engineering-inputs-r1.json.gz`：718 项完整工程集合，SHA-256
  `c3fa4e8b1a8a0f11f8177334fd0c9927e2f1f45d4875cb61212987fadfc6f056`。
- `raw/composer-red-r1.*.gz` / `raw/composer-green-r1.*.gz`：
  实际编译红例 4 pass / 4 fail，修后 8/0/0；失败不覆盖。
- `raw/full-gate-r1.*.gz`、`raw/final-gate-summary.json.gz`：
  最终统一门禁实际 0/reaped/699.865s；Rust 3218/0/48 ignored、
  rustdoc 14/0、Python 290 run/1 skip，app 1339 名逐名相等，完整输入零漂移。
- `raw/review-source-r1/` 与 `raw/source-review-gate-binding.json.gz`：
  新非作者源码批准及相同输入最终门禁绑定；没有独立重新编译。
- `raw/review-native-r1/`：另一新非作者对实际原件的限定复核。
  `raw/native-notes.md.gz` 保留五轮不完整脚本、前台变化和异常链；
  原生双向到达两列表的反向 Tab 仍 HOLD，marker 不是 raw PTY 写入审计。
- `raw/native-cleanup-final-r2.json.gz`：实际 stop/reap/exit0、三个根进程、
  两端口、私有目录及 84 项 birth 身份回收；含 JSON 身份归一化正控。
  旧 r1 内层 tuple 比较不可用，保留并明确替代。
- `raw/normal-artifacts.json.gz`、`raw/package-r1/package-manifest.json.gz`：
  新 arm64 GUI/MCP 与完整包摘要。没有安装、签名或发行。

两个[实际命令草稿](previews/native-command-draft.png)和
[首个反向 Tab](previews/native-first-reverse-tab.png)预览均保持原字节。
这两个片段不能代签完整路径；`previews.json` 绑定对应原始 PNG。

## 上一源码的精确 CI

`raw/ci-followup/` 逐字节保存此前活动 CI 工作目录；`raw/ci-followup-copy.json.gz`
记录完整 84 文件复制核对。run 43797131194 / attempt1 精确绑定源码
80647edca4baa1a07532f328849f24d43819e864，macOS/Ubuntu success，Windows app
1310/8/2 ignored，八项归档审核准备共同失败。原始 ANSI 与去 ANSI 日志、
取件失败、成功 r08、行号摘录和分析均保留。该结果不移签为本批新源码 CI。
详细结论见[CI 跟进](../2026-11-16-platform-ci-followup.md)。

`verify_binding.py` 对 index 或指定证据提交核对原件、预览、根 metadata 与
718 项当前工程 blob；可另传 `--source-commit` 比较源码和证据两棵 Git 树。
提交、推送与清理以单列实际回执为准。后续源码修改后应读取保存的绑定回执与
精确历史提交，不应放宽本冻结的当前字节检查。

源码与本目录 205 份原件已在 `9d0f886acda4e48058004a28f85ef1fa3c510d4c`
提交并推送，`index-input-binding.json` 和 `source-commit-binding.json`
均零 mismatch。`source-push-readback.json` 保存实际三方一致、
首次 HTTP 408 与成功重试；`delivery-source-*` 另存 14 份无损交付命令原件
和逐项摘要，包含精确新源码 CI `43801985537 / attempt 1` 的首次快照。
这些后补交付回执不冒称早已位于源码提交中。

`cleanup-focus.json` 记录核对归档原字节后删除 213 个临时文件、逻辑
`230,504,949` 字节，旧焦点工作目录 absent，正常 GUI/MCP 与有效 target 保留。
旧 CI 原件目录因下一批归档诊断仍需读取暂留，活动新 CI 原件另位于
`work/command-focus-ci-20261116/`；后续清理和文档交付按各单列回执读取。

文档 `7df6020` 已推送并三方匹配，见 `documentation-push-readback.json`，
docs push run 数为 0；`delivery-followup-*` 另存 11 份后补原件与摘要。
源码/文档两树绑定的实际零 mismatch 回执包含其中。
`cleanup-old-ci.json` 记录后续归档诊断结束后回收旧 CI 的 84 文件、
`1,265,813` 字节，原目录 absent。未集成的
[Windows 诊断草稿](../2026-11-16-windows-archive-diagnosis-draft/README.md)
另存 25 份原件并回收临时副本；作者和两个 reviewer、observer 都已关闭。
正常程序、有效 target 和仍运行的新 CI 原件目录保持。
