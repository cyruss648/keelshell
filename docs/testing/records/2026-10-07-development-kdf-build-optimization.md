# 开发构建的固定 KDF 优化 — 2026-10-07

当前更新：新的非作者已完成完整源码与证据核对，并独立通过四项阶段观察回归及原期限的保存资料审核用例。结论限于开发构建优化和观察器，历史 Linux 失败根因未知。主线组合和原生验收另记，以下保留作者原时点。

## 范围

独立候选基于 `e69575bc19afe03e80708e03d4993768c8f5dfb0`，仅在根 `Cargo.toml` 为 Argon2 依赖设置开发 profile 的 `opt-level = 3`，另包含测试专用同步阶段观察器。应用锁定依赖、所有 core 源码和 release profile 不变。原 Argon2id、64 MiB、3 次迭代、单通道与密文策略不变，既有等待期限和通过断言不变。见 [ADR 0075](../../adr/0075-development-kdf-build-optimization.md)。

## 独立 Linux ARM64 成本对比

固定 Rust 1.98.1，自有单 CPU、2 GiB、无网络容器，隔离 Cargo/target/TMP；同一公共核心探针和同一锁定依赖，只有测量项目的开发 profile 变化。各三次的中位数：

| 操作 | 原 debug | Argon2 opt-level=3 | 原耗时 / 优化后耗时 |
| --- | ---: | ---: | ---: |
| 空 vault 创建 | 1.150373 秒 | 0.487518 秒 | 2.360 |
| 已有共享密文读取 | 1.154786 秒 | 0.500097 秒 | 2.309 |
| 已有共享密文批准 | 2.313371 秒 | 0.998367 秒 | 2.317 |

批准后实际读取各自 local store，并检查 peer 本地元数据未写入固定测试明文密码。两次运行 actual 0，39.497 秒和 17.372 秒均包含编译；上表仅为具体操作的单调时钟耗时。成功 raw 分别 3,686 / 1,096 字节，SHA-256 为 `f04b6c11b4506b956a10b292c352d792294417cf40e290fa59ec40b991f74163` / `f5dc432cdcc3ae3471c354be731e3c862f10cf6dd6421f5149926e27d3d65d0b`。

共享密文及本地元数据不含固定测试密码的断言，由另行执行的[既有 core 配置同步回归](../../../crates/keelshell-core/tests/profile_sync.rs)独立覆盖；两次成本探针没有读取共享密文，不能把该回归断言归到成本探针。此次修订只校准本记录和 ADR 的两处证据范围，原 v1 冻结包、探针、raw、实际 wait 与测试执行结果均保留，不重新运行。

首次离线源准备退出 101 已保留。所有运行都只等待和清理自身数字 PID/PGID 与明确 owner 容器；Podman 客户端残留的空 TMP 目录在独立跟进中删除，不追溯改写最初结果。容器测量不代表 Linux x86 CI、目标原生桌面、客户机器或模型验收。

## 现有基线与后续回归

优化前的观察器四项用例、原保存资料批准及定时用例和严格工程检查实际通过；失败与成功分别见 [阶段观察记录](2026-10-07-profile-sync-runtime-observation.md)。原测试源码中的 18 秒和 45 秒限制以及全文磁盘、认证会话和捕获路由断言未改变。

本项目优化配置的限定作者回归实际通过：

| 运行 | 实际结果 | 包含编译的运行器耗时 |
| --- | --- | ---: |
| app test 编译，保留实际 rustc 命令 | 退出 0，Argon2 `opt-level=3`、debug assertions on、debuginfo 1 | 80.603 秒 |
| 同步面板、已有两项 workspace 同步与四项阶段观察 | 11 passed / 0 failed / 0 ignored | 24.738 秒 |
| 原保存资料批准及捕获定时 | 1 passed / 0 failed / 0 ignored | 45.558 秒 |
| vault 与双客户端同步 | 33 + 6 passed / 0 failed / 0 ignored | 86.439 秒 |
| pending 恢复、取消、丢弃与 replay 单元边界 | 5 passed / 0 failed / 0 ignored | 11.386 秒 |
| 独立 wire、审核发布与设备审计同步隔离 | 10 + 10 + 1 passed / 0 failed / 0 ignored | 71.582 秒 |
| 格式、x.y 策略、workspace all-targets 严格 Clippy | 各实际退出 0 | 1.467 / 0.075 / 85.543 秒 |

共 12 项 app（含两项纯观察）和 65 项 core；限定运行没有执行其它被 filter 排除的测试，也不是完整 workspace test gate。原保存资料测试源码与期限未改；成功记录的核心服务耗时为 Inspect 约 0.418 秒、Apply 约 0.898 秒，完整定时与全文磁盘、捕获 SSH 路由断言实际通过。这是新的受控 GPUI/TCP 测试样本，不追溯证明旧失败原因。

上述九个运行前后 741 份输入完全相同，随后仅更新结果文档。首七项使用自有 COW mac-target 与单构建任务；最后两项 core 回归使用自有工作树默认 target 和 Cargo 默认构建调度，均显式单测试线程，未使用共享主树目标。所有数字 PID/PGID 实际 wait/reap，并复读进程组和私有 TMP 已消失。原调试链接器的 compact unwind 边界提示与 block future-incompat 提示保留，不表述成无警告链接。主项目锁定依赖及全部 core 输入仍与精确 e695 基线逐字节相同。

完整冻结源码、feature 前后图、成本探针、实际编译参数、所有 raw/receipt 和失败材料封存在忽略目录 `work/linux-sync-runtime-diagnosis-v1/`。新非作者复核及主线组合未完成。精确历史 Linux 18 秒失败仍为原因未知，后续主线目录控制器 8 秒失败是另一未进入应用同步的范围；不据此宣称已修复任一历史 CI 根因。新的提交级三平台 CI 与原生验收继续开放。
