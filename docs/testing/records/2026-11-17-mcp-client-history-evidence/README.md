# MCP 模板与未知记录原件

工程基线 `f4562ad17efc918976738b241352e7a4f8af434e`；
冻结 777 个非 docs 输入，aggregate
`b34aedb8d859f1ca0b97934b707c71de8200b3ffd8a53ab602d323809932553e`。
实际结果和边界见[本批记录](../2026-11-17-mcp-client-history.md)。

`originals.tar.gz` 为确定性归档：260 个普通文件、3,457,053 逻辑字节；
压缩后 654,067 字节，SHA-256
`2070356dc083d8d098bd8475123633661325fe6782806465a11ac7a9b105d75b`。
`manifest.json` 逐件记录原路径、成员名、长度和摘要；归档生成时逐件读回核对。
没有提交测试二进制、运行凭据或正常应用包。

| 入口 | 内容 |
| --- | --- |
| `parent/candidate-inputs-r1.json` | 精确输入库存 |
| `parent/final-gate-r1/`、`parent/gate-counts.json` | 完整门禁原日志、实际 wait/exit 回执与分组计数 |
| `parent/final-gate-r1-inventory-r2/` | 96 普通套件逐名库存、摘要和结果相符 |
| `parent/final-gate-r1-inventory/`、`parent/inventory-r1-failure.txt` | 初轮工装失败、交错输出和原解析器 |
| `parent/app-mcp-r1/` 至 `r4/` | 编译、首提案布局原失败和最终专项 |
| `parent/template-parser-r2/` | 实际生产 renderer 输出及独立完整 TOML/JSON 解析 |
| `parent/dependencies-r1.json` | 37 包/113 个直接声明的官方快照 |
| `review/REPORT.md`、`review/review-result.json` | 新非作者的限定源码/功能批准、62 项预编译执行和未验边界 |
| `parent/binary-copy-cleanup.json` | 核对后回收的三份自有可执行副本 |

正常工程门禁通过 Rust 3455/0/68 ignored、15 rustdoc 和 Python 主 526 run/3 skipped。
reviewer 没有独立编译；其 62 项不叠加成完整门禁计数。原生操作/截图均为 0，
没有已安装客户端配置加载、供应商八工具业务、三平台桌面或发行验收。
归档中的绝对路径是当次原回执，不作为可执行维护入口。

提交后可离线核对精确 Git 输入及本归档和隧道 CI 归档：

```sh
python3 -B docs/testing/records/2026-11-17-mcp-client-history-evidence/verify_git_binding.py \
  --commit <exact-40-character-feature-commit>
```

该命令不运行产品测试、网络或 Cargo。临时展开副本在 Git 绑定后按完整文件集、
长度和摘要核对回收；唯一 target 和正常旧包另行保留。
