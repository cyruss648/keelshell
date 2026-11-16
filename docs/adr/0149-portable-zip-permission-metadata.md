# ADR 0149：跨平台 ZIP 私有权限元数据

状态：采用。日期：2026-11-16。

## 现场问题

精确源码 `f094de6ad2789d168f5a24bacb316c98b83608ac` 的 Windows 后端预检和
完整应用测试通过，后续 core archive 测试在重新编码危险来源权限时失败：
读回 `0664`，原安全断言要求 `0600`。原件和终态见
[根目录发布记录](../testing/records/2026-11-16-windows-archive-root-publication.md)。

当前锁定的 zip 9.6.0 默认根据创建者平台选择 ZIP 元数据 system；
Windows 使用 DOS。仅设置 `unix_permissions` 不会改变这个默认值，
读者会按照声明的 system 解释 external attributes。
本机将输出显式改为 DOS 的受控负例也精确复现 `436 != 384`，
这是元数据解释问题，不应通过放宽私有权限断言解决。

## 决定

归档是平台无关的内存数据，不承载创建机器的原始权限。编码器共用一份明确
`System::Unix` 的 ZIP options，普通文件继续设置 `0600`，目录继续设置 `0700`。
这只固定 ZIP 权限字段的解释空间，不更改本地文件 ACL、来源审核、staging、
无覆盖发布、完整读回、取消或未知结果隔离。TAR 的 mode/uid/gid 规则不变。

新增回归逐项核对文件与目录的 reader system、完整类型与 mode，
并直接检查 central-directory 原始 creator OS 和 external attributes。
完整 bundle roundtrip 继续验证成员及 bytes。既有危险权限／ownership
重新编码断言保持原要求。

Windows CI 在耗时的完整应用门禁前编译并执行完整 archive codec target，
先强制核对既有安全用例和新回归的具体库存名。session 早期预检扩大到整个
library，保留三项 archive 必需名字并加入三项 Windows Zmodem 名字。
任何缺失或执行失败继续使 job 失败，原完整工程门禁只在预检成功后运行；
没有重试、忽略或把未运行范围改签为通过。

## 验证边界

宿主两项 DOS 负例均实际失败；生产字节精确恢复后重新编译，archive
`41/0/0`、session library `288/0/4 ignored` 和 app archive `13/0/0`，
346 名与库存逐项相等，719 工程输入零漂移。
这些结果不代签新修订的 Windows 原生执行或完整产品门禁。
三平台后续精确 CI、桌面及发行仍分别验收，见
[本批记录](../testing/records/2026-11-16-portable-zip-metadata.md)。
