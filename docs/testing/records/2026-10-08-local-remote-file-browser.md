# 本地／远程文件浏览候选记录

日期：2026-10-08。本记录保留 v5 隔离候选的限定源码、文件回归和工程检查。之后已精确导入根主副本，根完整门禁、新标准 macOS 包及宽窗口 picker／审核式双向 SFTP 实际验证通过；这些独立结果见[主树整合记录](2026-10-08-local-file-browser-main-integration.md)，新的非作者组合源码／工程／宽窗口原生证据复核无 P1/P2；精确提交 CI 与其它原生范围仍开放。
下方 v1–v4 离线状态为历史准备，当前实际运行另列。

基线为精确 `a4c15c04772b11a70a592ae7c95132b4bea4d818` 加已由根审核的 43 条输入变化：
785 文件／15,644,131 字节，完整 BASE_AFTER map SHA-256
`45daecebcc44d1005ff2d5ff739dc61a86f351ca0340e9497eb6d0d5ee2ff268`。
这些继承变化不计入本功能 delta。专用分支 `feature/local-file-browser`；managed worktree
已创建，但 attachment 因 identity 上限失败，不声明 attached。封存作者包时主树未由此候选修改；后续根整合不覆盖原作者封包和失败材料。

## v1–v4 历史离线准备

完成源码读取、限定 diff/hash、官方本地 GPUI API 核对和离线实现。
分工 scanner 作者只写 local_catalog，另检查 app 整合的窄静态范围；其自身 scanner
未获独立批准。静态发现并修正 picker 回复过期后 pending 无法释放的候选风险。
随后补正 pending 时不渲染的传输输入测试入口，改用仍可见的本地导航 Input 真键盘入口。
离线 v3 也把新控件的普通字符串 tooltip 改为复用现有 live-locale builder，
防止提示展开后语言切换仍显示旧文本；v2 源快照保留，未声称实际复现或通过。
这些均不是实际执行反例或验收。

v3 新非作者静态审查实际提出两个 P2：LB-TEST-01 为 `TransferSpec::download(remote, local)`
测试参数反向；LB-UX-02 为远程 Edit 子按钮冒泡触发行选择，撤回刚创建的弃脏草稿审核。
v4 将下载来源／目标改为正确类型顺序，远程 Open/Edit 和本地 Open/Use 在子动作开始处
停止冒泡。新加两个 GPUI 测试定义：真实绘制后直接分派 MouseMove／MouseDown／MouseUp，
指针对之间不重绘，不直接调用动作处理器。远程场景用实际键盘改草稿，检查准确 Read
提案保持、取消原草稿保持、确认才打开新文件及远程原字节零写；本地场景检查 Open
清空旧目录选区后不会被父行恢复，Use 只填输入而不替父行选择。它们均尚未运行。

旧生产反例将采用相同 v4 测试文件和其它源码，只在 remote/local 两个 view 文件还原 v3
未隔离冒泡的生产处理器；下载参数类型修复保持，以便实际编译进入行为。反例源码包与
v3 原包分别封存。该 v4 计划在封存时为 **UNRUN**；实际 v5 同案例反例及修后结果见下文。

v4 封存时没有运行 Cargo、rustfmt、Clippy、测试、GPUI、SSH、供应商 CLI、网络、模型、Cua
或原生程序，也没有 feature commit；当时所有新测试定义均为 NOT_RUN。
26 个 scanner 条件测试定义（平台实际数量不同），4 个排序／目标名称行为测试、10 个 GPUI 场景
定义；原 files harness 和所有原断言／预算保持，只新增 browser_tests 注册。

## 后续门禁

1. 获得根明确共享 target 释放授权后，先格式检查、依赖版本策略和 strict Clippy，再编译新专项；执行同测试源码旧生产反例及 v4 指针案例。
2. 运行原 files 全部测试、原 4 个展开／Textarea 测试和原最小预算矩阵；不减少断言、
   不把 body64、compact48 或 tools28 改小以迁就布局。
3. 新专项验证启动本地空、系统 picker 仅单目录、旧 picker 不覆盖草稿、单层 no-follow、
   symlink／特殊类型、4097／编码／时间／取消边界、原生路径编码、最新导航采用。
4. 从真实本地条目点击使用，审核前远程目标不存在；确认后受控 SFTP 全字节读回。
   真键盘编辑可见导航字段撤回未执行审核；只读续传准备期间改输入不重建旧提案。
5. 实际已开始并暂停的上传，在隐藏条目和修改输入后保留原 stop/session/job owner，
   继续传输仍向原目标写原内容；独立目标／隔离／MutationLease 原回归继续运行。
6. 新布局测量覆盖 900×580／1440×900、zh/en、明暗主题、有／无 AI；两侧真实数据行
   和固定导航可达，原工具／审核／编辑预算与两轴滚动保持。
7. 正式主树组合门禁与新非作者源码／实际行为复核后才能批准整合。macOS 原生 picker、
   浏览／双向传输／完整审核与实际资源清理须另记；Windows/Linux 原生和新 CI 亦独立。

保留 scanner before/after metadata 的非精确对象锚及 5 秒协作软界限，不能把 metadata
快照或 GPUI 控件测试写成传输安全重新授权、真实平台系统 picker 或原生验收。
多选／拖放／批量目标预览仍 OPEN；本候选仅完成日常单选导航设计和待验证实现。


## v5 实际限定运行

应用为 binary，所有 app 运行都使用 `cargo test -p keelshell-app --bin keelshell-app --locked`，
不使用无效的 `--lib`。作者独占共享构建目录，未运行根完整门禁、原生应用、网络供应商
或安装；每轮有 600 秒 owned Popen 上限、实际 wait、独立 0700 TMPDIR、完整源副本、
前后 map 与原始日志，不设置 `RUST_MIN_STACK`。

最初 v4 `current-painted-v1` 实际 101：本地指针场景在默认测试线程发生 SIGABRT 栈溢出，
远程场景尚未到达；原有空浏览单项也实际 101。实际 `nm`／`llvm-objdump` 显示外层
`FilesPanel::render` 调试构建 prologue 保留 1,182,256 字节，嵌套本地 builder 另有大帧。
v5 仅把远程列表／表头构建抽到 `remote_browser_view` 返回 AnyElement；所有 ID、
listener、选择、审核、样式和垂直预算保持。外层实测 prologue 降为 879,840 字节，
独立远程 helper 为 302,672 字节；这只是当前 macOS 调试二进制的静态量，实际默认栈
通过由下方测试单独证明，不代替平台原生验收。

LLDB 动态尝试没有产生 stop/backtrace，不据此推断崩溃栈。113.703 秒后核对精确
PPID／command，终止自有 test/debugserver/leader；leader actual wait 为 -9，三 PID／
组已消失。作者不是两个孙进程的 wait 父进程，不能声称其 actual Join。两个 SIGABRT
和 LLDB 的原 scratch 保留，不能写成所有私有目录都已回收。

隔离旧冒泡变体保留 v5 相同渲染拆分、全部相同测试与其它 791 输入，只在两个 view
各去掉一条 `cx.stop_propagation()`。它是隔离 LB-UX-02 的旧行为变体，不是未拆分的
完整 v3 直接运行。旧两个指针案例均实际断言失败；精确还原全部 793 输入后，新两个
案例均通过：取消保持原草稿，确认才打开新文件，原远程字节零写，本地 Open 不恢复
旧目录选择，Use 不触发父行选择。原 v3 参数反向源码与 v4 栈失败材料分别保留。

| 准确运行 | actual wait / 结果 | 原始日志 SHA-256 |
| --- | --- | --- |
| old-painted-v5 | 101；两项预期行为反例失败，16.017 秒 | `feeee5155bb36d0098ab69bc9793f0d3f4daefc3af68bc38a566c630f9c8114f` |
| new-painted-v5 | 0；同两项通过，10.359 秒 | `e38efc9b1655e2c898a7f96aa354baa9cdde7ab18270d222873fafd1ada0e226` |
| browser-v1 | 0；十项 GPUI／受控 SFTP 通过，5.214 秒 | `2cda4d4fb52f1dfda841fa73d191fbd990b09cb5e3e7280e71eeebbb539c7ae5` |
| browser-pure-v1 | 0；四项排序／目标名称通过 | `16cb1ab4c381051336374daacecde0e3eb2b31f3c926a3f04adc4e48bc0ee82d` |
| catalog-v3 | 0；macOS 适用二十四项通过，11.314 秒 | `0cea1671929a26b50d71f73677d3655e47571baa6b6561fa308bef48b9471f29` |
| files-v1 | 0；文件领域一百三十六项四线程通过，68.863 秒 | `03d3bc40c7950729c15be36a9e82132204f4a8e8bb6f05e3a54a2f588f48cbd4` |
| strict-v2 | 0；全 workspace／all-targets／locked／-D warnings，21.898 秒 | `239f258f1ae1ac32f0b359acffbecdcef8cf8c46f61f4155a194ca125958c281` |
| fmt-v6 / policy-v1 | 0；格式和 x.y 策略通过 | policy `47498699907bbee5b4053f0f74c3ec525343147d17fb717a89bf2984f5c9d189` |

macOS 非 UTF-8 夹具的原 `catalog-v1` 实际 23 过／1 失败：`File::create` 返回 EILSEQ92，
尚未调用 scanner。之后 `catalog-v2` 新拒绝分支误假定 metadata 也返回 92，实际为
ENOENT2，再次 23 过／1 失败；两次原始日志／源输入均保留。最终只改该测试：先强
断言 OsString／PathBuf／metadata_root_path 原字节，仅 macOS 且实际创建 raw92 才进入
明确拒绝分支，断言未创建路径 metadata NotFound／raw2、scanner typed RootMetadata
与实际 metadata 错误精确相等及原 native 路径保持。其它创建错误仍失败；可接受该编码
的文件系统仍运行原真实 listing／name／path 断言。最终日志实际输出 macOS 拒绝分支。
本机未创建或读回非法字节文件，不继承 Linux／其它 Unix 的该项业务证明。

四十个条件定义未增加或删减，本机适用三十八项（24 scanner、4 pure、10 GPUI），已被
当前 `files-v1` 全部覆盖；两项 Windows 条件定义未执行。不同平台和拒绝分支具有不同
实际覆盖，不称“四十种同样行为通过”。旧／新冒泡、browser 与纯排序发生在编码测试
修正前；这些准确生产及测试文件与最终八个 Rust 文件相等，只有 local_catalog 的该
测试段随后改。最终 catalog／files／strict／fmt／policy 同一完整 793 输入／15,758,250
字节前后相等；这些成功运行无 survivor、scratch 空且删除，旧失败 scratch 有意保留。

首轮格式检查实际 1 及 formatter 输入变化均保留；格式只改声明 Rust 路径，未修改继承
基线范围。新非作者正在独立消费源码、所有真实原日志、资源终态和最终文案，不以作者
结果代替批准。最后仅七份状态 Markdown 更新，Rust、manifest、lockfile、工具链和其它
输入均保持与最后运行相等；最终 map／限定 feature delta 和原始证据留在忽略的 work。

仍 OPEN：根整合后的完整门禁、新提交 CI、macOS 原生 picker／本地浏览／双向传输与
完整审核、真实原生小窗口与辅助技术、Windows/Linux 原生及非法编码接受文件系统
分支、硬取消阻塞 syscall／整体应用退出有界、多选／拖放／批量目标。无 feature commit
或推送，未把 fixture、受控 GPUI、编译或反汇编写成原生验收。
