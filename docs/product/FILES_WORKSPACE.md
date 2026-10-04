# 文件工作区 / Files workspace

文件操作使用当前已认证 SSH 会话的 SFTP 通道，远端路径与终端工作目录独立。
上传、下载、目录创建、重命名、删除、权限修改和内容保存均保留已有审核流程；
目录比较只读取当前成功加载的 canonical 远端目录与显式填写的本地目录。

名称、权限和本地传输路径草稿在语言切换时保留。文件与传输操作按可用宽度换行，
AI 侧栏打开后仍可以查看全部操作标签。待确认时审核内容与确认/取消按钮暂时替换
操作工具栏；取消会恢复原有草稿。提示文字在已展开的状态下也按当前语言重新绘制。
默认中文/跟随系统外观规则继续沿用应用设置。

Files use the active authenticated SSH session's SFTP channel. The remote path
is separate from the terminal working directory. Upload/download, creation,
rename, deletion, permissions and content saving retain their explicit review
flows. Directory comparison reads the last successfully loaded canonical remote
directory and the explicitly entered local directory.

Language changes preserve name, permissions and local-path drafts. File and
transfer controls wrap to the available width, including with the assistant
open. A pending review temporarily replaces the action toolbar with its complete
text and Confirm/Cancel controls; cancellation restores the drafts. Visible
application hints read the live language when redrawn. Chinese and System
appearance remain the defaults.

实现与验证边界见 [ADR 0043](../adr/0043-responsive-files-and-live-tooltip-translations.md)
和 [测试记录](../testing/records/2026-10-05-files-responsive-tooltips.md)。布局/协议测试
不等同于 Windows/Linux 原生桌面、真实客户 SSH 或所有模态可访问性验收。
