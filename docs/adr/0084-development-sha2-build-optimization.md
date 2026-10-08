# ADR 0084：开发与测试构建的 SHA2 依赖优化

- 日期：2026-10-08
- 状态：根已导入并通过实际 test-profile 编译、完整工程门禁和最终非作者证据复核；0.10.9 Linux flags 与精确新提交 CI 待完成

## 决策

仅在根 `Cargo.toml` 增加 `[profile.dev.package."sha2@0.11"] opt-level = 3`。该 Package ID selector 匹配 0.11.*，不选择 lock 中的 SHA2 0.10.9；它不是直接库依赖版本要求，直接依赖仍为 x.y `sha2 = "0.11"`。保留 Cargo.lock 的实际 patch 版本、debug assertions、overflow checks、现有 Argon2 优化与 release profile。

[Cargo profile 文档](https://doc.rust-lang.org/cargo/reference/profiles.html#overrides)说明 package override 与 test 继承 dev；[Package ID 文档](https://doc.rust-lang.org/cargo/reference/pkgid-spec.html)说明 `name@x.y` 匹配 x.y.*。实际工程的 0.11 编译 flags 与 0.10 不受影响仍须根读回；泛型代码可能按调用者 profile 实例化，不能仅凭配置推断收益。

## 依据与不变量

Linux 选定目录的本地智能体 Ask 路径在实际启动前完整读取并核验可执行文件五次，其中一次创建受控副本；该描述不适用于 Windows 的不同目录 guard，Windows 不具有上述 owned-copy 路径。版本／有效能力准入、原件与副本内容、身份、权限及 launcher 再核验均保留，不减少哈希或改用未校验缓存。目录回归夹具的原 8 秒 Ask 期限、五秒检查、取消与清理策略、测试正文和四线程入口不变。产品请求超时仍由用户配置，不是统一的 8 秒。

自有 50 MiB 文件的单组 macOS/aarch64 对照中，五轮完整 SHA256（含一次完整复制）程序内 wall 从 819,710 µs 降至 138,349 µs；十个完整摘要与独立参考相等。独立依赖图只有 SHA2 0.11.0，使用无歧义的包名 override；本工程用上述限定 selector。该单组测量有顺序／缓存等未测边界，不代表 Linux runner 或完整 Ask。

两次精确 Linux CI 均在目录回归原 8 秒 Ask 期限内、实际启动前超时，但准确慢操作与 runner 哈希后端仍 UNKNOWN。本优化不宣称查明或修复历史根因。外层期限与 blocking worker 生命周期的另项风险不在本决策中修改。

## 验证

根须读回实际 test-profile 的两版 SHA2 编译参数，执行原完整门禁及依赖策略，并保持 Unix 20（Windows 15）项目录控制、默认／2 MiB 各 626 阶段、原篡改拒绝、取消、清理与四线程测试。精确提交三平台 CI、原生产品验收仍需自己的证据。实际记录见[优化验证记录](../testing/records/2026-10-08-development-sha2-build-optimization.md)。

## 根验证更新

实际 macOS 根图中 SHA2 0.11.0 两条 rustc 命令均为 opt-level=3 且 debug-assertions=on，原完整四线程门禁通过 1,812 普通 Rust／10 rustdoc／48 Python及严格检查，原默认／2 MiB 各626阶段完成，输入及收尾已绑定。0.10.9 仅由 Linux 的 oo7 依赖，本机未观察其实际 flags；新提交三平台 CI、完整原生产品和安装仍待验。基线 f3 的 Linux 成功发生在本优化加入前，不能归因于本决策，历史 Timeout 原因保持 UNKNOWN；详细实际结果与限制见上述验证记录。
