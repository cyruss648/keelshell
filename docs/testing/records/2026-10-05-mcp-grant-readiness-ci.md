# MCP 重授权测试同步提交的三平台 CI

精确提交 `df8cf8be4ea66f1186d511ef84c202709bd4fd4e` 的 [Quality37314626129](https://github.com/cyruss648/keelshell/actions/runs/37314626129) completed/success。API head 与 macOS26 ARM64、Ubuntu24.04 x64、Windows2025 x64 的实际 checkout 全部一致。新非作者从 API/原ZIP/job日志/Unix artifact 核对，根另完整读回保存。原[文件提案首次CI失败](2026-10-05-mcp-file-proposals-ci.md)保持，不推定其具体时序原因。

| 平台 | 普通 Rust / rustdoc | ignored | 脚本 | 打包 |
| --- | --- | --- | --- | --- |
| macOS | 1083 / 8 | 11 | 6通过 | 57通过 |
| Linux | 1083 / 8 | 12 | 6通过 | 57通过 |
| Windows | 1063 / 8 | 11 | 5通过、1跳过 | 53通过、4跳过 |

三平台20个相关唯一名称/23次实际执行全部通过，含原失败重授权名称与三项新屏障在 app 和 ssh_loopback 的六次实际执行。默认/显式2MiB控制器另实际success，不重复加入普通统计。两项供应商CLI选择仍ignored；Unix各9项真实系统OpenSSH通过，Windows相关步骤跳过。Unix清理观察到61/69出生身份消失、TMP删除，仅声明已观察身份，不覆盖观察间隔内完全脱离的未知进程。

原ZIP 314,510字节 SHA256 `76516757b4798ab238322a4e3c0da522d5906df3d308cf58d246254a85f879f4`，35成员CRC/精确三原job日志读回通过。非作者168 proof/169归档由根逐bytes/hash核验，manifest `33e9297215a4219c8665fd58794717dd7590f37cabea4dc27b743de242deca7a`，1,194,253字节 archive `b893f1abe0bc70efbd7f3723679f95f0bfda19431ef2c7fecfdefee5ce92826c`。初始 doc 计数、Windows路径匹配和 ignored/controller 归属解析错误保持，最终以实际 stdout 边界校正，不视为产品测试失败。

本记录关闭 df8 的源码 CI，不提供新 GUI、供应商八工具、跨平台原生、正式 Release/安装更新或后续 API F 的验收。见[同步修复](2026-10-05-mcp-grant-readiness.md)。
