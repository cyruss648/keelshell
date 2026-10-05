# 2026-10-05 AI 全配置 Basic 派生秘密 P1 修复

- 基线：23310ee13286adb488a451277addf49b05cd476b；先应用冻结 A 补丁。
- 范围：只修非当前配置代理 Basic 派生秘密遗漏，不增加产品功能。
- 状态：B 作者门禁通过，但 fresh 独立复核确认新的目录/Test P1，禁止整合；C 限定修复另记，main 门禁 / 原生窗口验收待完成。
- 不提交、推送或启动 GUI/模型；不修改 root、A、reviewer 源码或 cache。

## 保留原反例

独立 probe 2998 字节，SHA256 `020049030c4a611a5d83146231b71de7ad49bf5c4d920575235c692949b3f732`，源码与原断言不改。A 的 13 次失败、初次 clone timeout 及最终通过证明以完整原归档/manifest/hash保留；独立反例 stdout/stderr 原字节保留。五路径 negative 不能用旧工程通过或活跃配置脱敏替代。

## 构建隔离与结果

独立 managed worktree、0700 evidence/TMP、0600 收据；parent 授权从静止 A target 只读 APFS clone 至本树 target，不接触 root/reviewer target。fresh clone 完成：96.599s 复制＋23.954s 全量核验；562386 files / 6646 dirs / 113791747638 bytes一致，0共享inode、size mismatch、symlink，源不变。复制后停止读取 A target，所有 probe、wrapper、原失败归档已独立复制B，无外部 include! 或编译路径。root可归档A与review-A，原证明另外已核验复制。

| 收据 | 结果 | 说明 |
| --- | --- | --- |
| inactive-basic-b-negative | failed / exit101 | 82.696s，inputs unchanged；原反例五true，0send/CLI/job |
| inactive-basic-b-positive | failed / exit101 | 手工lock依赖行未消歧已有base64多版本，--locked正确拒绝；未运行测试，未升级依赖 |
| inactive-basic-b-positive-2 | 3 passed | 70.078s，inputs unchanged；原反例五false，slot替换/清除和metadata、reply/建议脱敏、clear后迟到结果拒绝 |
| b-fullcheck-1 | passed | 344.237s，inputs unchanged；x.y / Python6 / fmt / strict Clippy / 44 Rust harness1098passed、11ignored / 普通与小栈控制器 |
| b-macos-build-1 | passed | 38.126s，inputs unchanged；显式部署版本15.0的app及companion，独立B target，不启动程序 |
| b-native-inspection-1 | passed | 0.167s，inputs unchanged；只读 file / vtool / SHA256，arm64 Mach-O，未启动程序 |

库与app显式保留裸Basic及Basic前缀两种形式；BoundSecret派生项不实现Serialize/Debug，Zeroizing所有权随槽移除/替换释放。app只增加现有base64.workspace=0.22，lock明确0.22.1，无版本升级。accepted reply在revision gate后全配置脱敏，迟到结果不进入UI/建议。格式化 command exit0 的 wrapper input-change/90为有意格式写入，不计检查通过。原probe通过fullcheck fmt后仍字节一致。

11ignored与A同范围：2vendor CLI+9OpenSSH；1098计数含8doc tests，控制器另记，不重复计focused。B新增失败及A的13次失败/clone timeout、独立negative stdout/stderr及完整review proof原归档均留存。工具等待每次不超过60秒，长命令后台运行，不覆盖失败。

只读构建产物检查：app 181024192 bytes，SHA256 `68ffdd82f30f9f8617fbb2c204913e66e232910530cf8e810ec749a4268d6fcc`，Mach-O arm64 / minos 15.0；companion 16653808 bytes，SHA256 `c5c8895b38bdb79b0cc1fcd8ca474c31f5f43f94220f4f326d9a5c95ab1f36c7`，Mach-O arm64 / minos 11.0。SDK 27.0。既有依赖 block 0.1.6 的 future-incompatibility 提示保留，不冒充新的零告警结论。

作者交付包括相同基线的完整组合 patch、逐文件源 SHA/大小/status/baseline manifest、全部 A 与 review-A 证明原归档、B 每次命令/退出码/耗时/输入哈希/日志与证明 archive/seal。原反例在 fullcheck 后仍与 2998-byte 原源码完全一致；不依赖旧 A/reviewer 编译路径。最后检查后无作者自有 cargo/clone/GUI 进程，B target/TMP 可在 root 保存证明后清理，源码和冻结证明保留供 fresh review。

## 验收边界

prepare 反例是未发请求的上下文证据。受控 GPUI、TCP fixture、编译和 macOS build 不等于实际 macOS 窗口、Windows/Linux 原生、真实供应商或有效证书代理验收。B 还须 fresh 非作者复核及 root 整合后原生验收。

后续 fresh 复核：原五路径修复、inactive vault/slot/迟到回复等限定补充检查通过，B 仍被新的 5801-byte 目录反例否定。另一 auth-None Chat 配置实际 GET 接收非活动代理裸 Basic 为 model ID，生产 InputState/sync/Test 实际 POST model=该秘密，GET 1 / POST 1、owned server joined 后负面断言失败。118 份完整复核证明 / 119 个归档成员原字节保留，不能将 B 的 1098 通过或构建迁移为缺陷关闭。见 [C 修复记录](2026-10-05-ai-request-options-catalog-fix.md)。
