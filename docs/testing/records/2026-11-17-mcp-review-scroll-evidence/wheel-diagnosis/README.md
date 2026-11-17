# 原生滚轮对照的限定封存

只签未修改的正常 macOS arm64 r1 包及工程
`39d9041f350e30788b39121959d2ae7f7efea043`。
详细动作和边界见[本轮记录](../../2026-11-17-mcp-native-wheel.md)。
此补充不改变上层旧 archive，也不签后续固定目标栏的源码或界面。

`originals.tar.gz` 保存 80 个普通文件，逻辑长度 4,987,390 字节；
压缩长度 2,530,841 字节，SHA-256：
`c8f4f9d3b8d988bd4a41ed4a6767f1890e33133d57f963ea78c3f24e829180e2`。
manifest 记录每项原来源、长度、摘要和安全 member 名称。
`probe/` 保存十组未经编辑 JPEG／完整 AX、正常包输入绑定、实际客户端／应用／
联调清理回执及自有 helper；`review/` 保存新非作者全部报告和断言历史。
五个隔离运行库／配置文件只记录排除原因与摘要，不发布其正文。
私钥已回收；临时 MCP 能力未持久化，包和 target 没有被复制进 archive。

## 复核与结论

Zeno the 2nd 实际打开全部十张原图、读取十份完整 AX，
最终仅对旧包的英文／浅色／宽窗导航和拒绝路径限定 `APPROVE_BOUNDED`。
开放 P0／P1／P2／P3 均 0；旧“原生滚轮不可用”的宽泛 P2 在此准确包范围关闭，
余下为方向映射原因 UNKNOWN、物理输入及完整原生矩阵未验。
没有生产代码修复滚轮的结论。

最终 r3 离线断言 907 通过／0 失败，真实 `0/reaped/0.446137s`；
核对 777 个准确旧 Git 输入及六个未改变包文件。
首轮 888 项中 3 个“当前工作树应干净／等于旧源码”的断言失败、
补充 791 项中 2 个“仅一个源码变化”的过窄假设失败均保留；
补充命令 exit0 不能代替其断言成功。r3 只核对不可变原件，
并把并行新功能的当前工作树变化作为观察，不把重叠运行累加为唯一覆盖。
可选 Pillow 测量因缺模块失败，未安装依赖，也未宣称像素测量成功。
reviewer 已由父级关闭。

首客户端的 280 秒 marker 失败保留；后续只读三调用确认准确既有提案 rejected、
原文件完整 80 字节及 SHA 不变，没有提案重放。应用正常退出、授权撤销、
自有容器／网络／派生镜像／私钥目录身份核对回收均有原件。
清理日志保留 sshd 连接关闭及 `kill(15): Operation not permitted` 消息，
没有把成功回收写成 stderr 全空。

## 维护

在仓库根目录只读执行：

```sh
python3 -B docs/testing/records/2026-11-17-mcp-review-scroll-evidence/wheel-diagnosis/verify.py
python3 -B docs/testing/records/2026-11-17-mcp-review-scroll-evidence/wheel-diagnosis/verify.py --bind-source
python3 -B docs/testing/records/2026-11-17-mcp-review-scroll-evidence/wheel-diagnosis/verify.py --commit FULL_40_HEX_COMMIT
```

默认逐字节检查 archive、成员、三测试差异、原图／AX配对、拒绝读回及退出回执。
`--bind-source` 用本地 Git 对象核对准确 39d 的 777 工程输入；
`--commit` 同时核对本目录交付文件，不拿新的源码或文档 HEAD 代签旧包。
无网络、解包执行、产品测试、应用启动或 target 操作。
