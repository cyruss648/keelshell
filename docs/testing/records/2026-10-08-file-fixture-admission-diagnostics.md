# 文件夹具入组诊断候选 — 2026-10-08

状态：诊断已进入本机 `feature/browser-platform-controls`，完整四线程工程检查与打包测试实际通过；新增七项控制均通过。v1 静态复核的五处 expect lint 冲突改用既有 Checked，原断言保持，全 workspace／all targets 严格 Clippy 实际通过。新的非作者最终复核无 P1/P2 阻断，新提交后的精确 CI 尚待完成。
不将诊断实现或格式检查当作原 CI 根因确认或任何平台原生验收。

精确基线 `7c11954761b75243d82e1b52282aeb822e271b12` 的
[Linux Quality job](https://github.com/cyruss648/keelshell/actions/runs/37683724603/job/113006039888)
在 `wrapped_file_actions_still_review_the_exact_selected_remote_target` 中发生
120 秒 FixtureGroup admission timeout。该 app 704 passed、1 failed、2 ignored，
855.47 秒，后续工程门禁和 Linux OpenSSH 未执行。
等待发生在创建该用例的 SSH 夹具、目录与窗口之前，不能表述为上传或审核断言失败。
原完整日志 448,864 字节，SHA256
`419935a19c9f0d5484c06d35a99e8c43308dc02b8e973bc983e8a89fc0d4e33a`。
原准确原因和原实际 group owner 仍为 **UNKNOWN**；同时运行的布局矩阵耗时
仅构成调查方向，没有 owner／阶段时间证据把它认定为原因。

## 候选范围

仅在原 `#[cfg(test)]` 的 FixtureGroup 路径加入观察模块：

- 固定测试函数标签、进程内 request/group 数字身份和相对 Instant 时间线。
- 真实 permit 取得、同一 App 复用、最后 group owner Drop、实际 queue 清理启动、
  最后 queue Arc 等待、queue close／scheduler join、实际 permit drop 和 fail-closed。
- timeout／admission closed 后输出固定 JSON 快照，再保留原 Checked 失败。
  request-created 不等同于 Tokio 首次 poll／FIFO 入队；request scope drop 也不声称
  独立 Tokio future 已完成取消。

原 semaphore、120 秒入组和清理期限、四线程检查入口、原业务正文与断言保持。
没有增加重试、忽略、串行绕过、早释放或改变失败结果。原私有 spawn 失败控制的
结构体仅补 `UNOBSERVED` 身份字段，不改变其行为断言。

观察者只存 Weak 或数字，不持有 permit、runtime、queue 或 FixtureGroup 强引用。
记录使用 try_lock；竞争／poison 会丢失诊断并计数，不能阻塞准入或 Drop。
事件 ring 限 256，pending metadata 限 64，group metadata 限 8；活跃元数据
独立于事件 ring，任何元数据容量或记录丢失均显式标记不完整。队列 strong count
是第一次实际 try_unwrap 失败的样本，不冒称快照时的实时 queue 引用数。
标签仅接收编译期函数名／固定内部值，限制字符和长度；无文件路径、命令、主机、
日志内容或凭据。序列化和输出发生在释放观察锁之后，输出错误不覆盖原失败。
输出字节受结构上限约束，但没有宣称操作系统 stderr 写入有硬截止期限。
事件时间是操作后记录的单调采样时间，不能用不同线程的记录顺序替代精确操作时刻。

## 候选控制与验证状态

七项私有 observer 控制均在本机正式检查中实际通过：

1. ring 淘汰保留活跃 owner／pending metadata，观察 Weak 不保持真实 group。
2. request scope drop 去除 pending，不伪造 acquired。
3. 诊断锁竞争不消耗／释放真实私有 semaphore permit，并显式记录丢证据。
4. pending metadata 上限和不完整标记，不淘汰活跃 owner。
5. 诊断 mutex poison 不改变真实私有 permit；输出仍 best effort。
6. group 已死而 queue 清理 metadata 保持到明确释放边界。
7. BrokenPipe 输出不 panic，非法路径形状标签退回固定值。

本次同一 813 输入的正式检查实际通过 1,776 项普通 Rust、10 项 rustdoc、6 项 Python、格式／依赖策略和严格 Clippy；22 ignored 未执行。新增七项 observer、原三项 FixtureGroup 和真实多 App／worker 生命周期控制包含在应用 712 项通过的四线程运行中。57 项打包测试也实际通过。原日志、输入相等、actual wait 和资源清理见[平台记录](2026-10-08-file-browser-platform-ci.md)。

后续实际 Linux 运行再用新时间线调查 owner／FIFO 累积／真实清理阶段，不能以 macOS 通过或新一次 Linux 成功补造这次历史失败的原因。七项 observer 私有控制包含受控元数据事件，不能冒称完整真实 Linux 生命周期重现。此次没有改变生产行为、启动 GUI 或增加原生验收。

完整基线输入、候选 preimage/postimage、diff、原始失败与正式检查材料保留在忽略的 `work/browser-platform-controls-main-20261008-v1`。没有删除历史证据。
