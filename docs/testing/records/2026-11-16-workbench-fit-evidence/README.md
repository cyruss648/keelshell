# 工具栏与文件失败诊断的持久证据

本目录保存本批实际命令、五文件快照、编译库存及非作者静态评审，见
[测试记录](../2026-11-16-workbench-fit-and-file-diagnostics.md)。

`originals.tar.gz` 含 44 个普通文件，压缩 278,264 字节，正文合计
1,444,653 字节。SHA-256：
`acceca73826df3dd3d1ea7eff805899b673a5f4a90e01f35dfd63644aebc5333`。
`manifest.json` 列出逐成员来源、长度和摘要；归档保留原件与已明确命名的派生
摘要，tar/gzip 时间固定，不含运行态、凭据、app/MCP 二进制、图标或其他任务文件。

719 项工程输入冻结集合为
`1a647ee1fbdfde06a8e9e664dce17a82246644aa5f5ef5008c10fc66236d2495`。
完整门禁实际 `0/reaped/764.801s`，Rust `3224/0/48 ignored`、rustdoc `14/0`，
app `1341/0/2 ignored`，1,343 名逐项相同、输入零漂移。归档保留 1600 首候选
越界失败及诊断 helper 首次类型编译失败；后续成功不覆盖它们。
非作者静态批准限于五文件，P0–P3 均 0，独立测试/构建/GUI 均 0。

正常包由当前冻结真实构建，包清单、五文件摘要、实际构建和结构检查回执均保留；
构建时源码未提交，包内 `build.commit=null`，不能以当时文档 HEAD 单独证明源码。
CUA 因 Mac 锁屏受阻，实际 GUI 操作/截图均 0。自有进程按身份 SIGTERM，
实际 `241/reaped`，运行态已删除；包保留给解锁后的原生检查。
Windows 四项旧失败仍待新精确 CI 诊断，归档不授予 Windows 修复、原生或发行通过。

从仓库根运行，只读校验当前工程输入和全部归档字节；不解压、不运行被归档的
代码，也不执行 Cargo、应用、GUI 或网络：

```sh
python3 -B docs/testing/records/2026-11-16-workbench-fit-evidence/verify.py
```

也可使用 `--source` 加完整提交 SHA，核对精确 Git 源码。后续源码变更时，
当前模式应失败；应核对本批的真实交付提交，不能重新签署旧冻结。
父的首次校验实际 `0/reaped`，成员/源码 mismatch 均为 0、测试重放为 0。
