# AI 加密凭据专项验证 — 2026-10-03

## 范围

AI 设置新增显式加密保存、主密码解锁、清除临时密钥和解除关联。默认密钥仍仅驻留内存；只保存不透明引用，保存与解锁不触发网络请求。设计及跨文件事务边界见 [ADR 0009](../../adr/0009-ai-encrypted-credentials.md)。

## 已执行

- `cargo test -p keelshell-core --test ai_profile_storage --locked`：8 项通过。验证 vault Bearer 引用可作为支持的元数据，Environment 引用仍被当前适配器拒绝，并保留真实配置存储回归。
- `cargo test -p keelshell-app --locked ai_ -- --test-threads=2`：18 项通过，含既有 AI 设置回归以及同批工作区集成测试。日志 `work/ai-vault-focused-tests-2.log`。
- `cargo test -p keelshell-app --locked assistant::tests:: -- --test-threads=2`：10 项通过。未解锁的引用无法预览；密钥、引用或目标变化使先前审核失效；既有真实回环 HTTP 延迟请求仍通过。日志 `work/ai-vault-assistant-tests.log`。
- `cargo clippy -p keelshell-app -p keelshell-core --all-targets --locked -- -D warnings`：通过，日志 `work/ai-vault-clippy-3.log`。第三方 `block 0.1.6` 的未来编译器兼容提示仍存在，不是本次严格 Clippy 失败。
- 修改文件使用 Rustfmt；专项 `git diff --check` 通过。未增加依赖。

## 关键证据

1. 真实凭据文件和配置文件都不含测试 API Key 或主密码。重新创建存储对象后，正确主密码能解锁；错误主密码拒绝，改目标地址拒绝。
2. 密文 payload 拒绝配置身份、地址、API 风格与认证方式变化；名称、模型 ID 变化允许。每次保存使用新 UUID，旧引用仍可解密；取消发生在准入前不创建文件。
3. GPUI 设置真实启动后台 KDF/文件写入，等待 mailbox 返回后更新草稿。操作期间 Apply 被拒绝，主密码框已清空；完成后没有 Apply 保存或模型网络动作。
4. GPUI 走过错误主密码、焦点回到密钥框、重新打开解锁输入、正确主密码成功。清除临时密钥保留不透明引用，锁定状态点击连接测试不会创建网络任务。
5. 修改地址后，即使旧文本框的 Change 事件尚未处理，后续名称修改也不能恢复旧密钥。修改认证清除密钥及引用；名称/模型修改保留。旧异步结果和已关闭草稿无法回填。
6. 工作区集成验证取消草稿不改变已应用密钥/引用，只有 Apply 才对助手生效。

## 保留的失败与验证边界

早期 `work/ai-vault-check*.log` 保留并行实现期间的未完成模块/签名错误，以及测试宏导入、测试 trait 缺失；均已修复。`work/ai-vault-clippy.log` 保留测试中 unwrap lint 与同批文件的 test module 排序问题，后续日志记录修复通过，不删除失败证据。

本记录证明本机 macOS 上的 Rust/GPUI 受控测试与隔离文件行为；不代表真实模型服务、Windows/Linux 原生 UI 或本轮完整三平台 CI 已验收。实际桌面截图、整仓门禁、提交后的 Quality 结果由本轮集成记录补充。
