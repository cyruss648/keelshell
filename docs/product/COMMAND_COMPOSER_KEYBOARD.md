# 命令输入框键盘 / Command composer keyboard

命令输入框持有焦点时，Tab 和 Shift+Tab 按当前窗口的可见焦点顺序向后和
向前导航，不缩进或反缩进命令。空草稿、中文、多行和选中文字均保留原字节。
导航不执行或发送命令，执行仍需原有审核流程。

输入法正在组合文字、弹窗已活动或焦点属于运行按钮时，命令栏不会接管动作。
其他输入框、远程草稿编辑器和终端继续使用各自的按键行为。弹窗内的焦点
限制仍由统一模态管理负责。

While the command composer owns focus, Tab and Shift+Tab follow the rendered
window's forward and reverse focus order. Navigation preserves the draft,
including indentation, Chinese text, line breaks and selections. It does not
send or execute the command.

Active text composition, a live modal or another focused control retains its
existing input path. Editors keep indentation and the terminal keeps its own
keyboard handling. Themes share the same behavior and semantic token system.

实际验证边界见
[批次记录](../testing/records/2026-11-16-command-composer-focus.md)；
GPUI 回归不代表真实 IME、VoiceOver 或多平台桌面验收。
