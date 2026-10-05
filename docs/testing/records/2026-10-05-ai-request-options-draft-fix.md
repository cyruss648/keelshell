# 2026-10-05 AI 保留草稿已知秘密 D 修复

- 基线：`23310ee13286adb488a451277addf49b05cd476b`。
- 状态：D 作者完整门禁与 MAC15 双程序开发编译/只读检查通过，但 fresh 非作者发现请求头名称披露 P1，D 不整合。原冻结候选及全部证据保持；后续修复独立记入 [E 记录](2026-10-05-ai-request-options-metadata-fix.md)。未提交或推送，实际原生验收待完成。
- 范围：仍保留但无法建立有效用途绑定的请求头/代理草稿，及其上下文、回复、目录和模型输入秘密边界。其它功能、CLI、供应商、云、原生 GUI、生产主机与用户凭据不在本轮执行范围。

## 原失败与隔离

C 新独立复核发现重复名 inactive header 原值仍保留在 masked 控件，却被用途槽重建遗忘，随后被另一配置的 model/Test 接纳。C 原字节、失败及 133 份证明由根保持；D 不重跑原受系统中断的审查探针，也不追溯将其标为通过。原 A/B 五路径与目录反例仍作为普通正式回归保留，断言原字节不改。

D 完整应用相同基线的 C 候选，继承产品实现。根在静止 C reviewer cache 上只读 APFS clone 到 D 私有 target：232.054 秒、589168 regular files、6700 目录、119132132102 logical bytes，0 shared inode/0 symlink/0 other，source before/after 一致，关键二进制 SHA 一致；完成后确认 D 可独占使用。收据在 ignored `work/request-options-draft-fix-evidence-20261005/cache-clone.json`，没有继续读取源 cache。

根指定的冻结 C 失败完整目录已复制至 D 的 `lineage-review-C-failed`，136 个文件逐 bytes/SHA 校验，原133 proof/134 archive members按原manifest完整读回一致。原 manifest SHA `b528c1fd63455b7e0ffc6e2712592da490c29a31c919df64f6ba1b2760ccb577`、archive SHA `75d6fdfdca7f4ceca2b50ae8ce284aa970221011c44edbb8f55aa07c447acba3` 保持；C 仍为 FAIL，0旧探针执行。

所有后续命令只用 D target 与绝对私有 TMP；证据目录/临时目录 mode 0700，日志/收据 mode 0600。独立 runner 记录 owned PID、限时退出、输入前后 size/SHA、原始日志 hash；超时只终止该进程组。每个 HTTP 探针使用一次性自有回环，不读取系统凭据或生产配置。

## 实现与正式回归

草稿秘密独立 zeroizing 保存，不把有效投递和“仍是已知秘密”混成一个槽。所有仍保留 header 值及 proxy raw/Basic 参加全配置秘密集合；vault 回填同时刷新签名，替换后释放旧值。输入名称、引用、用途或整批 metadata 无效仍保留秘密。清空、删行、删 profile、换目的地/用途释放值，pending UI 清空不恢复旧值。去重后辅助 `len` 计唯一已知值，`is_empty` 包含 draft-only cache。

新增正式单元回归验证 draft-only 的非空/替换/清空、无 header/proxy 投递能力、冻结旧快照与4096项/单值1MiB/总8MiB超界失败关闭。GPUI 回归覆盖 blank/reserved/duplicate/invalid UUID 草稿、masked 控件保留、已有目录移除、cancel/旧 callback、清空/删除/改用途与排队目的地变化、vault 回填及有效绑定。真实控件输入生成的重复名草稿进入三条 API 和两条 Local preparation/reply 生命周期；没有启动 CLI。一次性 HTTP 夹具应观察单 GET、秘密目录拒绝和手填 model 不产生后续连接。超限 InputState 应保留原值并拒绝其它有效 profile 的新操作。

## 作者命令证据

| 收据 | 状态 | 范围 |
| --- | --- | --- |
| d-fmt-write-1 | failed | 新测试 module 路径未显式匹配 include 上下文，未写格式；原日志保留 |
| d-fmt-write-2 | write-only | cargo fmt exit0，输入变化为预期格式写入，runner90不计检查通过 |
| d-focused-drafts-1 | failed | 新测试 glob 引入同名 test macro 导致递归，改为明确 imports；原日志保留 |
| d-focused-drafts-2 | failed | 未验证继承草稿对 Zeroizing 使用 Ord/Display；改为 Zeroizing Vec 按 borrowed str 排序去重/as_str，原日志保留 |
| d-fmt-write-3/4/5 | write-only | 各 cargo fmt exit0，预期格式写入，runner90不计检查通过 |
| d-focused-drafts-3 | failed | 43通过/1失败；新增 Local fixture 缺少受支持认证配置，尚未进入审核，原日志保留 |
| d-focused-drafts-4 | passed | 44正式普通/GPUI回归，0失败；11.246秒/inputs unchanged，包含11项D新增回归；与后续整仓不累加 |
| d-fullcheck-1 | passed | 338.138秒/inputs unchanged；41普通harness共1109 passed/0 failed/11 ignored，4doc harness共8 passed；Python6、fmt、strict workspace/all-targets Clippy、x.y和默认/显式2MiB控制器分别通过，future均4624 bytes |
| d-macos-build-1 | passed | 40.099秒/inputs unchanged；显式MACOSX_DEPLOYMENT_TARGET=15.0命令编译app/MCP，未启动 |
| d-macos-inspect-1 | failed | app实际minos15.0，MCP继承缓存仍minos11.0；原二进制size/SHA和load-command输出保留，不能称本次双MAC15产物 |
| d-mcp-private-clean-1 | completed | 3.641秒/inputs unchanged；仅D私有target的MCP缓存15445文件/1.6GiB清理，不触碰源cache/主树 |
| d-macos-build-2 | passed | 4.088秒/inputs unchanged；同MAC15环境实际重编MCP及app，未启动 |
| d-macos-inspect-2 | passed | 两者实际arm64 Mach-O且minos15.0；SHA与完整load commands冻结，0程序启动 |

11个ignored是原2项供应商CLI选择和9项OpenSSH，不计执行通过。默认controller和显式2MiB controller分别通过，不重复计入普通harness数量。既有block0.1.6 future-incompatibility提示保留。最终开发app SHA为`2718a73e0342f7baed37e8795b98e1a882e42de9c5e1b36908b7e871eb5ca8e0`，MCP SHA为`27f1a51413253ee552c326f5a3c7d0bdfca79e2449be2ab46c6b483292bfaa24`，二者minos15.0；没有启动程序或打包安装。

最终产品/ADR/测试记录补全文字后，checked工程输入保持同一bytes/SHA；完整组合patch、逐文件source/inputmanifest、完整C失败lineage与D全部失败/通过证明、source/proof archive和seal一起冻结。作者不再写源或运行Cargo，不自行提交/推送；D target/TMP保持独立供根后续只读clone与清理。新的非作者审查与根整合仍须独立执行。

编译与GPUI/回环证据不代表实际原生窗口、目标供应商/云、Windows/Linux桌面、正式Release或安装更新验收。
