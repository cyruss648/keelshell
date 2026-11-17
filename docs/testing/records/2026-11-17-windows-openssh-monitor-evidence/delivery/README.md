# Windows SSH 与原生工具栏的文档交付

工程为`660e90d6f7c7111853db7b73f4ca2344f633700d`；
本批验收文档、510成员CI原件及44成员原生原件已经通过普通提交
**`c088062baacc0c44ccfbc98de2a817fbc2a62869`** 推送。
本地／tracking／远端三方SHA一致，读回工作区干净：
提交`0/reaped/0.105633s`、推送`0/reaped/6.593515s`、
读回`0/reaped/0.920101s`。

`originals.tar.gz`保存 **65成员／146,313字节**，SHA-256：
`6c7563eb05dafbd80a5c90efffca135d63872259f28efdf6534207484a5cfa76`。
包含提交前765工程输入及16个改动文档的Git index绑定、
提交／推送／三方读回、CI verifier的中间失败与最终成功、
第一批清理结果，以及42个未在先前archive出现的辅助原件。
这些辅助原件包括首轮观察器、工装源码、第一次未选中interop artifact的下载回执，
以及扩展interop校验时误用workspace汇总器的失败；原失败没有覆盖。

已按`c088062`中的archive及原ZIP成员逐字节核对并回收：

- 原生reviewer、首版CI reviewer、新Windows reviewer副本；
- 510成员CI stage和精确修复CI观察器；
- 合计5目录、827文件、17,535,741字节。正常包6文件及唯一target的device／inode前后不变。

封存时余下4份自有工作副本共720文件、31,356,477字节。
它们所需的42个补充文件现全部包含在本archive；其余内容已在既有Git archive或ZIP成员中。
最终回收仍须在本archive提交后重新逐字节核对，不能仅凭目录名删除。
最终文档推送和后续回收的执行回执保留在`work/windows-ssh-finalize-20261117/`，
维护入口使用本页及上级CI／原生archive，不依赖待回收或已回收的展开路径。

离线复核本次已推送的文档、原件和第一批清理：

```sh
python3 -B docs/testing/records/2026-11-17-windows-openssh-monitor-evidence/delivery/verify.py
```

实际`0/reaped/0.192644s`，产品测试重放0。
完整工程门禁保持既有r2结果，工程输入765项不变。
真实SSH、CI、原生子集和完整产品的边界从[CI终态](../ci-completion/README.md)、
[工具栏原生子集](../../2026-11-17-native-toolbar-evidence/README.md)与
[当前交接](../../../../HANDOFF.md)读取；没有新增Windows执行或完整桌面验收。
