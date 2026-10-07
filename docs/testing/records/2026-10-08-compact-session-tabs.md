# 小窗口会话标签修复

2026-10-08。基线本地提交 `219093c`，其43路径功能切片已通过自己的工程与限定原生检查。此新修复不能继承其通过结论。

## 已证实的问题

先前独立原生审查逐张读回12份macOS窗口原图；英文小窗口会话关闭×被标签容器裁切，是既有P2。根已接受该发现；本次以可滚动标签、标题省略、不收缩的关闭按钮、明确的前后导航及小窗口图标工具栏修复。

## 当前状态

四项真实GPUI回归和一项嵌入图标回归已实际通过；固定工具链整仓格式、x.y策略、全工作区all-targets严格Clippy实际通过。新的非作者已复核reveal、静态图标、旧绘制索引修正和原工具栏限定证据；与自动更新整合后的最终800输入完整门禁及同源码标准macOS包实际通过，新宽窗口SSH导航／关闭／草稿保留已观察。英文原生小窗口保持开放，不继承旧主线结果；详见[组合记录](2026-10-08-update-toolbar-main-combination.md)。

新的非作者静态发现语言切换和resize后的旧bounds会掩盖close裁切；已补语言绑定、布局后的有界下一帧reveal及保持同一活动标签的回归。默认图标包也缺少部分应用图标；新的非作者继续发现AllAssets默认debug构建仍读取编译时registry目录。已补debug-embed特性及实际source返回Borrowed静态字节的断言，直接依赖8.12/原锁定patch8.12.0保持；加载回归及宽窗口原生分屏／AI／更新图标已观察。原生小窗口图标布局另验。

首次三项GPUI运行实际失败：两项通过，手动滚动用例假设移动20像素即可裁切close未成立。改为由实际viewport与close间距决定移动量，保留确实裁切、一般重绘保持滚动和同标签重选完整显示的断言。额外原始MouseDown/Up反例不调用会自动重绘的window.click：前一标签关闭后激活旧绘制close，原生产代码实际因索引2/长度2越界失败。现按原EntityId解析选择/关闭、关闭阻止冒泡；重复激活已移除目标也不关闭剩余会话。相同旧帧场景与全部四项GPUI实际通过，未发送任何命令草稿。首次新增反例还因测试导入遗漏编译失败，原日志保留。

实际范围均在忽略的`work/compact-session-tabs-author-20261008-v1`：`focused-tabs-v2`四项/15.866秒，`focused-assets-v1`一项/0.377秒，`clippy-v1`86.334秒，各actualwait0、788源输入前后相同、所属组及私有TMP已回收。失败源/日志保留于`focused-tabs-v1`、`stale-close-before-fix`及`stale-close-before-fix-v2`。全依赖离线metadata曾因未缓存的非当前目标crate退出101；Cargo.lock仅增加app到已锁8.12.0的依赖边，限定实际编译使用--locked成功。

父提交精确219的Quality37657109077已结束：macOS/Linux通过，Windows失败于Claude Code受控后代端口的3秒准备期限；未记录该case的Ask返回/清理终态，原因保持UNKNOWN。根已完整读回三个job原日志，不能把平台CI或本切片本机检查当作Windows桌面验收。

源输入、实际检查结果、失败与修复在发生后记录。OS原生逻辑几何、VoiceOver、IME与Windows/Linux桌面仍需要各自证据。

相关决策：[ADR 0080](../../adr/0080-scrollable-session-tabs.md)。
