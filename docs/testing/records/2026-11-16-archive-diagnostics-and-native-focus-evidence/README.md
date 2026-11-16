# 归档准备诊断与原生焦点证据

结论见[本批记录](../2026-11-16-archive-diagnostics-and-native-focus.md)。
本目录保存 318 份选定原件，原字节 `13,796,970`，gzip 字节 `5,148,518`。
`archive-manifest.json` 逐项记录原件与压缩件的完整摘要；失败记录没有覆盖。
全部 167 份导航 AX 原件、初始输入失败、四个焦点终点和两张起始图均已保留。
`omitted-artifacts.json` 单列可重建程序/资产及未用于验收的常规中间截图；
不把未保存的中间截图称为本目录原件。下一片作者活动目录尚未归档或清理。

- `raw/source-inputs-r1.json.gz`：718 项完整工程输入，集合 SHA-256
  `e967a7619be613cca5c3fc103f4a6d3c8ad2ad746caacc32386cf169820dd1ff`。
- `raw/full-gate-r1.*.gz`、`raw/final-gate-summary.json.gz`：
  实际 0/reaped/704.319s，Rust `3220/0/48 ignored`、rustdoc `14/0`、
  Python `290 run / 1 skip`；app 1,341 名与门禁逐项相等，完整工程输入零漂移。
- `raw/archive-tests-r1.*.gz` 保留错误 library target 的 Cargo 101；
  `raw/archive-tests-r2.*.gz` 为正确 binary target 的 `12/0/0`；
  格式预检失败也保留，最终统一门禁格式通过。
- `raw/review-source-r1/`：新非作者源码 APPROVE、P0–P3 全 0，
  独立执行父预构建的 12 项专项，未独立重新编译。
- `raw/review-native-r1/`：另一新非作者限定原生证据 APPROVE、P0–P3 全 0，
  亲自看六张 PNG，独立提取 167 份完整 AX 值；实际测试/UI 重放 0。
- `raw/focus-native-r1/`、`raw/native-notes.md.gz`：Frost light 中文、
  双侧各一项的反向 32 / 正向 35 步完整观察、准确草稿、早期不完整场景、
  seed/marker 读回及 76 项身份清理。四张未修改预览在 `previews/`。
- `raw/package-r1/package-manifest.json.gz`：复用上批正常二进制暂存，
  不能把 manifest 当前 HEAD/dirty 当作新编译；原生静态检查通过。
- `raw/dependency-audit/` 和 `raw/toolchain-cli-r1.json.gz`：
  默认 Rust/Cargo 实际 1.100.0；37 包中 16 实时核实最新、21 UNKNOWN，
  所有直接 registry 显式要求符合 x.y；没有更改锁文件。
- `raw/ci-followup/`：精确 9d0f886、Quality 43801985537/attempt1 终态。
  Ubuntu success，macOS 固定截止测试 Elapsed，Windows 同八项缺审失败；
  原始及去 ANSI 日志保留，不能称 Windows 已修复。

`verify_binding.py --index --check-current` 核对提交前 index；
提交后用精确 40 位 `--source-commit` 和 `--evidence-commit` 核对两棵树。
它逐项验证 gzip/原字节、预览、工程路径集合及源码 Git blob。
后续工程输入变化时，不应放宽本冻结或把本批结果移签到新输入。
交付、推送和临时副本清理回执另行追加，不自称已存在于较早源码提交。
