# CORE-R1-01 第一阶段：Rust 回归已就绪

日期：2026-11-15（Asia/Shanghai）。
状态：`TEST_SOURCE_READY / PARENT_RED_RUN_PENDING / IMPLEMENTATION_NOT_CHANGED`。

父任务可集中编译并执行下列新增回归。当前作者仅在
`crates/keelshell-core/tests/zmodem.rs` 末尾追加测试及私有辅助函数，
并写入本交接文件；尚未修改 `crates/keelshell-core/src/zmodem/codec.rs`。
原始 `review-core-r1/notice-01.md` 及失败证据保持原件。

## 新增测试名

| 测试名 | 覆盖 |
| --- | --- |
| `receiver_missing_farewell_preserves_utf8_before_repeated_hex_zfin` | 初始有效 FIN 后整块输入 `个` + 有效 HEX FIN + `$ `，显式缺失 farewell 收尾 |
| `receiver_missing_farewell_preserves_utf8_before_repeated_hex_zfin_bytewise` | 同一缺失 farewell 输入逐字节喂入 |
| `receiver_missing_farewell_preserves_utf8_before_repeated_hex_zfin_at_every_split` | 同一缺失 farewell 输入的每个二段分片边界，含空首段及空末段 |
| `receiver_actual_oo_preserves_utf8_before_repeated_hex_zfin` | 同一输入追加真实 OO，整块输入后严格 Finished |
| `receiver_actual_oo_preserves_utf8_before_repeated_hex_zfin_bytewise` | 真实 OO 路径逐字节喂入，包含两个 O 之间的边界 |
| `receiver_actual_oo_preserves_utf8_before_repeated_hex_zfin_at_every_split` | 真实 OO 路径每个二段分片边界 |
| `receiver_finish_accepts_parity_tolerant_hex_protocol_pads` | 初始 FIN 的合法 parity pad：单个、两个、混合高低位及八个 pad；每个二段边界后真实 OO |
| `receiver_missing_farewell_strips_repeated_hex_zfin_with_parity_pads` | 相同 parity pad 组合的重复 FIN；逐字节及每个二段边界；缺失 farewell |
| `receiver_actual_oo_strips_repeated_hex_zfin_with_parity_pads` | 相同 parity pad 组合的重复 FIN；逐字节及每个二段边界；严格 OO |

前六项通过真实公开 `Receiver`、`encode_header` 和 `feed` 路径构造现场。
普通终端字节必须精确等于 `个$ `（十六进制 `e4b8aa2420`），同时逐项核对
完整事件序列：重复 FIN 仅产生一次 FIN 回复，终态仅产生一次
`Finished` 或 `FarewellMissing`，无重复 FIN 泄露为终端字节，也无额外 retry。
缺失 farewell 保持 `is_finished() == false` 并检查收尾幂等；
真实 OO 保持严格完成，随后不能降级为缺失 farewell。
parity 用例另检查 `tail $ ` 字节保持及协议 pad 不泄露。

根据未修正源码，预计前六项暴露 `0xaa` 被裁掉的问题；后三项用于防止通过
关闭 parity pad 支持规避问题。这里没有执行 Rust，不能将预计结果写为实测红绿。

## 本阶段静态核对

- 原候选测试文件的前 `57568` 字节逐字节保留，SHA-256 仍为
  `ecbed49a5e72db3c93e4427e26ec3e8ac8fd1209436719c48778f7624583bbf4`；
  原候选 83 项测试均未改动。
- 另从当前 HEAD 读取原有测试，65/65 项测试正文逐字节一致。
- 新增 9 项，源码测试名库存为 92 项且无重名；这是静态源码库存，
  不是编译后的测试库存。
- 当前测试文件 SHA-256：
  `462d490a2c742fc8717ecb9c24d42606097bf5ebdb0f2b8d8b1c5f20104b93ae`。
- `codec.rs` 与本阶段开始时摘要一致：
  `0344f2e97a0be81ed638aea9e3972527a12b6af6a66738d989473a5e0558ee3d`。

## 验证归属与下一步

本作者未运行 Cargo、rustc、rustfmt、Clippy 或任何测试二进制；
Rust 用例实际执行数为 0。没有读取或变更唯一共享 target，没有创建 worktree、
提交或 push，没有覆盖其他作者的独立文件。

建议父任务先确认新编译库存包含上述 9 个名字，再通过
`preserves_utf8_before_repeated_hex_zfin` 名称过滤执行六项红回归，
并保留三个 parity 兼容用例的实际结果。原 65 项及此前 18 项不改写。
父完成红回归并通知后，本作者才开始 `codec.rs` 的窄实现修复；
有界尾部、重复 FIN 不泄露、实际 OO 与缺失 farewell 的语义仍需保持。
集中格式、编译、门禁与非作者复核由父负责；本文件不表示问题已关闭或验收通过。

## 第二阶段：窄实现已就绪，等待父集中验证

状态：`PARENT_RED_CONFIRMED / NARROW_FIX_SOURCE_READY / PARENT_CORE_92_PENDING`。

父已授权开始实现。实际读取父运行的
`logs/core-utf8-red-r1.json` 与 `.log`：exit `101`，
`0 passed / 6 failed / 0 ignored / 86 filtered out`。
六项均实际得到终端字节 `[228, 184, 36, 32]`，期望为
`[228, 184, 170, 36, 32]`；原始红回归日志保持不变。

本阶段仅修改 `codec.rs`：在 farewell 状态且尚无头部候选时，
对可能的 parity pad `0xaa` 最多回看四字节。如果它属于有效 UTF-8 字符或
合法的未完整 UTF-8 前缀，就按普通终端字节保留，不作为新协议头的起点。
后续真正的 pad 仍可开始重复 FIN；通用 parity header 解析没有修改。
该检查不分配内存、不扫描整个保留尾部，原有尾部、头部及噪声预算与验证后
重复 FIN 裁剪逻辑均保持。

全部 92 项测试源码及断言与父红回归时逐字节相同，测试文件 SHA-256 仍为
`462d490a2c742fc8717ecb9c24d42606097bf5ebdb0f2b8d8b1c5f20104b93ae`。
本阶段 `codec.rs` SHA-256：
`946c35e4b1dedb506c8e18197e1eec5fd88b19a8f547c3c194dc303ddaf0326b`。
只执行静态源码、摘要核对及 `git diff --check`（exit 0）；没有执行
Cargo、rustc、rustfmt、Clippy、测试二进制或共享 target 操作。

父任务可立即执行全部 92 项 core 集中验证，重点确认六项红回归转绿、
三个 parity 用例仍通过、原有有界尾部及重复 FIN 无泄露断言通过；
最终格式、工程结论与非作者复核仍由父完成，本作者不签未经执行的绿色结果。

## 追加：Finishing 取消尾部回归已就绪

状态更新为 `NARROW_FIX_SOURCE_READY / CANCEL_TEST_SOURCE_READY / PARENT_CORE_94_PENDING`。
按父最新要求，完成窄修后又在测试文件末尾追加两项，覆盖完整输入、
逐字节及两字节分片：

- `receiver_finishing_cancel_preserves_utf8_tail_after_repeated_zfin`：
  初始 FIN 后输入 `个` + 有效重复 HEX FIN + `$ `，再本地取消；
  精确事件为 `[Send(CANCEL), Cancelled, Terminal("个$ ")]`，
  不含 `Finished` 或 `FarewellMissing`。检查重复取消幂等、不能转缺失 farewell，
  后续终端输出只包含新字节。
- `receiver_cancel_after_actual_oo_does_not_repeat_utf8_tail`：
  同一输入追加实际 OO，先精确输出 `[Send(FIN), Finished, Terminal("个$ ")]`；
  随后两次取消均无输出，后续终端输出不重复已交付尾部。

前 92 项源码的全部 `62915` 字节保持逐字节一致；原 HEAD 65 项测试正文仍
65/65 一致。当前源码库存为 94 个唯一测试名，测试文件 SHA-256：
`c2d939d74e809b4cc55e337276f3a6b64a205cf3c47e78d00322edf567f4ece3`。
`codec.rs` 摘要仍为第二阶段的
`946c35e4b1dedb506c8e18197e1eec5fd88b19a8f547c3c194dc303ddaf0326b`。

本作者没有修改 `receiver.rs`、session 或 app 文件；取消实现与生产 pump 修复
由父完成。静态摘要、源码库存及 `git diff --check` 已通过，仍未运行
Cargo、编译器、格式化器、测试二进制或共享 target 操作。
父现在应核对并集中执行 94 项 core 测试；本作者源码写入已完成。
