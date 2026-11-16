# 初次封存过滤器诊断

这是不可作为完整验收的193成员初次封存。包目录排除条件也匹配到同名前缀的
根层JSON／log回执，导致 `verify-initial-r1.py` 报
`KeyError: package-precommit-r1.json`，外层exit1/reaped。
原archive、manifest、封存器、验证器与完整失败日志均保留。

仅把排除条件限定为目录后代，重新生成上层199成员封存；原件未覆盖。
上层验证器成功重算全部输入、测试计数和独立复核，不重放测试。
