# 共享 Cargo target 的源码归属

共享第三方依赖缓存可以减少编译成本，但不同 checkout 或无`.git`源码副本
不能直接把同一target中的已有应用产物作为当前源码验收。
复制文件保留的mtime与旧依赖追踪路径可能导致错误复用；
源码摘要相同、Cargo返回0、甚至正常程序包含新界面文字，都不能证明测试二进制
已经覆盖当前全部切片。2026-11-06组合验证曾实际缺少知识分类新用例，
原件保留在[阶段记录](records/2026-11-06-monitor-knowledge-categories.md)。
机制依据为[Cargo官方fingerprint说明](https://doc.rust-lang.org/stable/nightly-rustc/cargo/compiler/fingerprint/index.html)：
普通包使用源码与dep-info的mtime判定新鲜度，绝对路径并非统一纳入fingerprint。
该说明支持归属切换时的保守失效策略；本次异常的具体判断仍以保存的清单、
依赖路径与实际编译原件为准。

## 切换归属

1. 先结束并实际wait前一个Cargo命令，确认共享槽已释放。
   构建、Clippy、测试及清理均不能与另一作者并行使用该target。
2. 保留需要的旧日志、测试清单、fingerprint、dep-info及包摘要。
   在新源码目录、同一`CARGO_TARGET_DIR`下使本项目产物失效：

   ```sh
   cargo clean --locked --offline \
     -p keelshell-app -p keelshell-core -p keelshell-session \
     -p keelshell-ai -p keelshell-mcp
   ```

   这只针对列出的项目包；不删除整个共享target、第三方缓存、工作树或运行中的应用。
3. 在新目录重新构建，保留实际编译路径和退出回执。应用可用
   `cargo test -p keelshell-app --locked -- --list`，其他包按具体测试目标列出清单；
   核对当前平台应有的新用例，再执行有意义专项或完整门禁。
   不只核对用例总数，也核对本片的具体用例名及实际运行结果。
4. 同一源码目录连续验证且输入未变化时沿用有效产物。
   再次切换归属、发现遗漏或来源不明时重新执行上述失效流程；
   不能通过补写成功状态、放宽断言或合并重复用例计数掩盖问题。

## 交付

新开发包必须来自当前冻结输入的重新构建，并核对包内完整文件与二进制摘要。
测试二进制和正常应用程序分别核对，不能相互代签。
已退出0但清单不完整的旧运行保持原退出事实，同时标明不可作为完整验收依据；
后续修订使用新编号，不覆盖旧日志、旧冻结或旧包。
真实原生、精确提交CI与发布安装仍分别验收。
