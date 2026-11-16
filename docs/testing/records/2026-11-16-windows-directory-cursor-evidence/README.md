# Windows 独立目录游标验证原件

工程结论与未验收范围见[本批记录](../2026-11-16-windows-directory-cursor.md)。
`originals.tar.gz` 保存 70 份批次原件及四份最终源码，共 74 个文件、
2,223,793 原字节。压缩后 420,430 字节，SHA-256 为
`811f4059b8de563af58e3858e66ba1e56f20243bfea87abfefd2d5d9b4291bf7`。
逐文件路径、长度和摘要见 `archive-manifest.json`。

`batch/` 保留两轮完整门禁、三组专项、Windows cross-Clippy、实时依赖查询、
作者候选及来源获取回执、三份独立静态评审、旧 CI 失败、清理与预期 Windows
库存。九份第三方参考全文未重新分发。原报告和失败回执保持原字节。
`source/` 保存最终四文件；最终完整门禁通过不改写此前窄范围评审的签署范围。

最终 719 项完整非文档工程输入集合 SHA-256 为
`f5ab8e7180b7e8fcd90610100f496e869468bdf0605f7b073c215ec30d4ebd83`。
首轮失败单独绑定基线 `80397acc98d5881cbd177d7259a54fd7f1556c4f`
加作者的两份 Windows 候选，其 719 项集合为
`67337a389845e537823adf5d10671d52b9fd3ec95df43ac1634161c1b5022749`。
从仓库根目录检查：

```sh
python3 -B docs/testing/records/2026-11-16-windows-directory-cursor-evidence/verify_evidence.py
python3 -B docs/testing/records/2026-11-16-windows-directory-cursor-evidence/verify_evidence.py --index
```

验证器还接受 `--source` 完整提交 SHA 和 `--evidence` 完整提交 SHA，
分别绑定工程源码与保存本证据目录的 Git 树。它不解包到磁盘，核对全部成员、
三份评审摘要、两轮门禁原日志及真实退出/回收、首轮与最终完整工程输入集合。
当前完整门禁为 `0/reaped/727.352s`，首轮为 `1/reaped/485.787s`。
这些检查不重新编译，不代表新正常包、安装或原生桌面验收。

预期 Windows 67 名由旧真实 64 名加三项新回归组成，当前实际 Windows 执行
仍为 0。推送本批源码后，应另行保存精确 CI、67 项预检、原完整 gate 及八项
app 归档的实际结果；不能用此处预期清单或交叉检查代签。
