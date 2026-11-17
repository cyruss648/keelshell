# ADR 0161 — 监控读取的及时取消

日期：2026-11-17。状态：已实现，本机完整工程门禁和非作者限定复核通过；
实际平台、原生桌面与发行分别验收。
基线：`4c8654f219779f48afd485bec8ff1b284905fbf0`。
延续[按项能力](0109-connection-local-monitor-capabilities.md)和
[Windows 只读监控](0156-windows-read-only-monitor.md)。

## 问题

旧 generation 检查可阻止迟到观察发布，并在 channel open 后阻止尚未发送的命令。
但已发送的只读请求和等待平台探针锁的请求仍要等待远端响应或原 SSH／锁超时，
之后才返回取消。暂停或显式重检不应继续占用这些本地等待。

## 决定

在连接所属的 CapabilityCache 内，用 generation 保留撤权事实，用 Notify 唤醒全部
旧请求。等待者先注册通知，再核对 generation，避免检查与等待之间丢失唤醒；
撤权早于 future 首次轮询也必须可见。新 generation 不继承旧取消。

只读 exec 和平台探针锁均优先选择取消分支。保留 dispatch 前的 generation 检查，
并在取得结果后再次检查；取消不写回能力缓存，不把取消缓存成缺失命令，
也不自动重放旧请求。暂停保留已完成观察；显式重检清空观察与平台身份。

取消 exec future 复用已有 PendingChannel／channel-open 独立 owner：
已打开和迟到打开的通道都有界关闭，不遗弃 late confirmation。
正常关闭只针对所属通道；若 CLOSE 无法进入协议队列，现有 owner 可中止共享连接。
本地取消与请求关闭不能证明远端进程已终止。普通 exec、审核式进程终止和不确定写入
保持原来的结果合同，不通过本机制重试或推定成功。

## 验证与边界

受控认证 SSH 用明确的 exec／open 与 CLOSE 事件核对撤权前后的行为，
涵盖资源、进程、端口、身份探针、锁等待和新旧 generation；缓存与通道断言分别核对。
此类协议夹具不是实际 Windows API／Windows OpenSSH、原生桌面或发行验收。
精确执行、失败与非作者结论见
[本批记录](../testing/records/2026-11-17-monitor-read-cancellation.md)。
