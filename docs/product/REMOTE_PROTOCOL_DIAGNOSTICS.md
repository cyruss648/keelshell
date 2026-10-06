# 远程协议诊断 / Remote protocol diagnostics

Status: implemented old-base candidate with a limited nonauthor review; current-main integration gates, CI and native desktop acceptance pending. The [test record](../testing/records/2026-10-06-remote-protocol-diagnostics.md) separates actual protocol services, controlled GPUI rendering and platform acceptance.

## 操作方式

打开已认证 SSH 会话的主机状态面板，展开“协议诊断”，选择 DNS、TLS 或 HTTP(S)。输入主机名/IP；TLS 另有端口，HTTP(S) 使用完整 URL。点击“预览诊断”查看执行 SSH 主机、完整目标和实际操作，再点击“确认探测”。修改输入、切换模式、替换会话或修改保存的连接路线后，旧审核失效，需要重新预览。诊断不会在连接、语言或主题切换时自动运行。

结果保持在该面板内存中，包括远程解析地址、实际连接地址、已验证证书及单调时钟分段耗时。取消只撤销本次结果的接纳；它不表示远端进程已瞬间退出。历史结果不会被当作新会话的检测结果。界面跟随中文/英文及 System/Light/Dark 设置。诊断不进入 AI 上下文、遥测、连接元数据或任务日志。

## Protocols and meaning

| Mode | Actual remote operation | Meaning and boundary |
|---|---|---|
| DNS | The target's `socket.getaddrinfo` system resolver, with IPv4/IPv6 and at most sixteen unique addresses | This includes hosts/NSS and configured resolver behavior. It is not a direct authoritative-DNS query and cannot identify which source answered. |
| TLS | Remote resolution, TCP connection and a verified TLS handshake, using the remote Python/OpenSSL default trust store | Reports negotiated version/cipher and leaf subject, issuer, validity dates, up to sixteen SAN entries and SHA-256 DER fingerprint. Hostname, expiry and trust rejection remain failures; no insecure retry. |
| HTTP(S) | Remote resolution/connection, optional verified TLS, and exactly one HTTP HEAD | Reports HTTP/1.0 or HTTP/1.1 status and bounded header timing. A 302 or 403 is an observed status, not service health or a successful business request. No redirect is followed. |

Timings cover completed resolution, connection, verified TLS and HEAD response headers. Failed phases have no invented duration. Total time includes worker startup and confirmed reap on the remote host; it is not the desktop's SSH round-trip time. If addresses or certificate display text are truncated, the result explicitly says so. Certificate text is escaped before display; raw headers, cookies, response body, redirect targets and peer exception strings are excluded.

## Environment and bounds

The current adapter requires a POSIX SSH exec environment and Python 3.8 or later with its normal standard library, sockets and TLS support. KeelShell checks capability during the explicit request and never installs packages. Missing `python3` is reported separately from unsupported Python/POSIX features; configure the remote environment separately, then preview again. Windows as a desktop client can diagnose a compatible remote target. Windows SSH server environments require a later adapter and are not accepted by this one.

The fixed remote supervisor gives the worker eight seconds overall, including resolution, rather than resetting the limit for each address or phase. It launches one owned process group, terminates that group on deadline and actually waits up to one second for exit. A confirmed remote timeout is distinguished from `cleanup_unknown`; unknown cleanup disables further diagnostics on that captured panel and requires a fresh authenticated session. The SSH envelope is ten seconds, with existing bounded channel cleanup afterward. Closing an SSH channel or cancelling a desktop operation alone cannot prove immediate remote process termination. The supervisor still bounds its worker while the remote exec environment remains operational; remote host failure or forcible termination of the supervisor is outside that confirmation boundary.

Inputs are immutable validated requests. Hosts accept ASCII DNS names or numeric IPv4/IPv6 literals; control characters, whitespace, shell syntax, scoped IPv6 and port zero are rejected. HTTP URLs must explicitly use `http` or `https` and must not contain userinfo, query strings or fragments. Percent escapes are checked; encoded controls, authority delimiters, backslashes and malformed escapes are rejected. Request text is serialized and passed as a quoted base64 argument to fixed code, never interpolated into shell syntax.

Each operation captures the authenticated connection and checks current saved destination/route/trust before preview, dispatch and result adoption. Deletion, route changes, reconnect replacement, stopped panels and stale results fail closed. Execution occurs on a transport worker outside the GPUI thread. Combined stdout/stderr output is capped at 32 KiB; framing, wire version, addresses, certificate text, phase roles and timings are validated before rendering.

## Remaining scope

Custom CA selection, client certificates, authenticated URLs, query strings, cookies, bodies, custom headers, redirects, explicit proxies, HTTP/2/3, UDP diagnostics and remote Windows adapters remain open. This feature is an explicit protocol probe, not a general network scan or application-health monitor. Controlled service tests and headless layout checks do not establish native macOS/Windows/Linux, accessibility or customer deployment acceptance.

Primary behavior references: [Python socket/getaddrinfo](https://docs.python.org/3/library/socket.html#socket.getaddrinfo), [default TLS context and certificates](https://docs.python.org/3/library/ssl.html#ssl.create_default_context), [HTTP client](https://docs.python.org/3/library/http.client.html), [signal callback execution](https://docs.python.org/3/library/signal.html#execution-of-python-signal-handlers), and [subprocess communicate timeout](https://docs.python.org/3/library/subprocess.html#subprocess.Popen.communicate).
