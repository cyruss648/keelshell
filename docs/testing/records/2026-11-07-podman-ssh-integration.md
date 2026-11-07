# Podman/OpenSSH 真实联调记录 — 2026-11-07

## 范围与结论

本次在独立 worktree
`/Users/hdh/.codex/worktrees/podman-ssh-integration-20261107/keelshell`
中完成真实 Podman/OpenSSH 联调。范围只包含测试容器、SSH 认证、命令执行、
SFTP 列表与最小文件往返、错误认证以及资源清理；没有修改 Rust 源码、Cargo
manifest 或锁文件。

最终结论：**通过**。Podman 引擎可用，最终隔离 OpenSSH 容器真实接受 SSH
公钥认证；系统 OpenSSH 客户端的命令输出和 SFTP 列表/上传/下载通过；KeelShell
现有 3 个 ignored OpenSSH 互操作用例通过。最终容器、网络、临时镜像、临时密钥、
临时数据目录和独立 Cargo target 均已清理，并逐项核验不存在。

这份记录只证明当前 macOS/Podman machine 上的真实 TCP SSH/SFTP 互操作。它不替代
Windows/Linux 原生桌面、生产 SSH 服务、真实外部 AI provider 或完整 ignored
互操作测试集的验收。

## 工作树与前置阅读

- 分支：`feature/podman-ssh-integration-20261107`
- 测试基线：`bd3b1ed4dcb063959d2bcee3254ee26fc0d76145`
- 已读取：`AGENTS.md`、`docs/HANDOFF.md`、`docs/testing/SHARED_TARGET.md`
- 已读取相关实现和测试：
  - `scripts/openssh_interop.py`
  - `scripts/test_openssh_interop.py`
  - `crates/keelshell-session/tests/openssh_interop.rs`
  - `crates/keelshell-session/tests/ssh_loopback.rs`
  - `crates/keelshell-session/tests/fixtures/file_transfer_review.rs`
  - `crates/keelshell-session/tests/fixtures/directory_transfers.rs`
  - `crates/keelshell-core/tests/openssh.rs`
- Rust 源码、测试源码、manifest 和 lockfile 在本轮保持不变。

本轮没有复用共享 Cargo target。KeelShell 的 Cargo 测试使用了独立的
canonicalized 临时 `CARGO_TARGET_DIR`，运行结束后随临时根目录删除。

## 环境与隔离资源

Podman 检查结果：

| 检查 | 实际结果 |
| --- | --- |
| `podman --version` | `podman version 6.1.4` |
| Podman machine | `podman-machine-default`，`Running=true`，9 CPU，16 GiB |
| Podman engine | `6.1.2`，`linux/arm64`，远程 socket `unix:///run/user/501/podman/podman.sock` |
| SSH 客户端 | `/usr/bin/ssh`、`/usr/bin/sftp`、`/usr/bin/scp`、`/usr/bin/ssh-keyscan` |

最终运行使用的隔离前缀和随机端口如下：

| 资源 | 值 |
| --- | --- |
| 项目随机 token | `d2d0ce80a3` |
| 项目前缀 | `keelshell-ssh-20261107-d2d0ce80a3` |
| 网络 | `keelshell-ssh-20261107-d2d0ce80a3-net` |
| 容器 | `keelshell-ssh-20261107-d2d0ce80a3-openssh` |
| 临时镜像 | `localhost/keelshell-ssh-20261107-d2d0ce80a3:1.0` |
| 本地映射 | `127.0.0.1:65032 -> container:22` |
| 账户 | `keelshell` |
| SSH banner | `SSH-2.0-OpenSSH_10.0` |
| 容器主机密钥指纹 | `SHA256:WFlmhN9HgW4IOfc31OxQHYH6314D/uk+kyYTzKMsH9U` |

基础镜像为 `docker.io/library/alpine:3.22`。Podman engine 实际解析到
`linux/arm64`，镜像记录如下：

- image ID：`sha256:062964a69725bb96611dae2ac911a5a1ab3f8d32fe6f096f267d77f09e55a99c`
- platform digest：`sha256:e233f2058b0628266762f745a2c17e623b418e0090f47d223909d1f9a7d4367e`
- manifest repo digests：`sha256:bfe7ceb9f09ec0566c48fb93356e45a47a00be5a1dd14fa5b909e7709c66b034`、
  `sha256:e233f2058b0628266762f745a2c17e623b418e0090f47d223909d1f9a7d4367e`
- image size：`8,833,655` bytes

临时 OpenSSH image 的构建结果：

- image ID：`sha256:006047eb131f1617093111861c7ac321e78f082033c5941015998f1b1bee4b14`
- local repo digest：`sha256:f39cef419a591d3671d6dbca3558e1ddbb84acd4ec62dc6fc4d6b7e737181b4c`
- image size：`16,050,316` bytes
- Alpine packages：`openssh-server`、`openssh-client`，构建输出显示
  `10.0_p1-r11`

容器配置使用临时生成的 Ed25519 client key。公钥通过只读 bind mount 进入容器，
入口脚本复制到 `~keelshell/.ssh/authorized_keys`；密码认证、keyboard-interactive、
TTY、TCP forwarding、agent forwarding、X11 forwarding 和 tunnel 均关闭，SFTP
使用 `internal-sftp`，`StrictModes` 保持开启。Alpine `adduser` 默认锁定新账户，
入口脚本只在这个临时测试容器中移除该账户的 shadow lock；密码认证仍保持关闭。
容器中的 host key 每次启动时临时生成。

基础镜像 `docker.io/library/alpine:3.22` 是外部共享依赖，保留在 Podman 本地
存储中，没有对它执行删除或 prune，避免删除其他任务可能使用的共享资源。此次
创建且拥有的自定义镜像使用了上述项目名前缀，并在清理阶段删除。

## 实际执行命令

以下命令按记录顺序执行；随机 token、端口和临时根目录均为本次运行生成。密钥
文件路径在记录中用 `<temporary-root>` 表示，没有打印私钥内容。

环境检查：

```text
podman --version
podman machine list --format '{{.Name}}|{{.Running}}|{{.CPUs}}|{{.Memory}}'
podman info --format '{{.Version.Version}}|{{.Host.OS}}|{{.Host.Arch}}|{{.Host.RemoteSocket.Path}}'
podman ps --all --format '{{.ID}}\t{{.Names}}\t{{.Status}}'
```

镜像、网络和容器：

```text
ssh-keygen -q -t ed25519 -N '' -C '' -f <temporary-root>/client_ed25519
ssh-keygen -q -t ed25519 -N '' -C '' -f <temporary-root>/wrong_ed25519
podman pull docker.io/library/alpine:3.22
podman image inspect docker.io/library/alpine:3.22 --format '{{.Id}}|{{.Digest}}|{{.Architecture}}|{{.Os}}|{{json .RepoDigests}}|{{.Size}}'
podman build --pull=never --tag localhost/keelshell-ssh-20261107-d2d0ce80a3:1.0 <temporary-root>/image-context
podman image inspect localhost/keelshell-ssh-20261107-d2d0ce80a3:1.0 --format '{{.Id}}|{{json .RepoDigests}}|{{.Size}}'
podman network create --label keelshell.test=keelshell-ssh-20261107-d2d0ce80a3 keelshell-ssh-20261107-d2d0ce80a3-net
podman run --detach \
  --name keelshell-ssh-20261107-d2d0ce80a3-openssh \
  --network keelshell-ssh-20261107-d2d0ce80a3-net \
  --publish 127.0.0.1:65032:22 \
  --volume <temporary-root>/client_ed25519.pub:/run/secrets/client.pub:ro \
  localhost/keelshell-ssh-20261107-d2d0ce80a3:1.0
podman exec keelshell-ssh-20261107-d2d0ce80a3-openssh cat /etc/ssh/ssh_host_ed25519_key.pub
ssh-keyscan -T 3 -p 65032 -t ed25519 127.0.0.1
ssh-keygen -lf <temporary-root>/container_host_ed25519.pub -E sha256
```

系统 OpenSSH 命令和文件操作：

```text
ssh -o BatchMode=yes -o IdentitiesOnly=yes \
  -o StrictHostKeyChecking=yes \
  -o UserKnownHostsFile=<temporary-root>/known_hosts \
  -o ConnectTimeout=5 -o ConnectionAttempts=1 \
  -i <temporary-root>/client_ed25519 -p 65032 \
  keelshell@127.0.0.1 \
  "printf 'podman-command-ok'; printf 'podman-warning' >&2"

sftp -b <temporary-root>/sftp.batch \
  -i <temporary-root>/client_ed25519 -P 65032 \
  -o BatchMode=yes -o IdentitiesOnly=yes \
  -o StrictHostKeyChecking=yes \
  -o UserKnownHostsFile=<temporary-root>/known_hosts \
  -o ConnectTimeout=5 -o ConnectionAttempts=1 \
  keelshell@127.0.0.1

ssh -o BatchMode=yes -o IdentitiesOnly=yes \
  -o StrictHostKeyChecking=yes \
  -o UserKnownHostsFile=<temporary-root>/known_hosts \
  -o ConnectTimeout=5 -o ConnectionAttempts=1 \
  -o PreferredAuthentications=publickey -o NumberOfPasswordPrompts=0 \
  -i <temporary-root>/wrong_ed25519 -p 65032 \
  keelshell@127.0.0.1 true
```

SFTP batch 内容为：

```text
mkdir integration
put <temporary-root>/upload-source.bin integration/upload.bin
ls -l integration
get integration/upload.bin <temporary-root>/downloaded.bin
bye
```

KeelShell 互操作命令使用 canonicalized 临时根目录和独立 Cargo target。下面将
实际临时根目录写成 `<temporary-root>`，三条命令实际都传入完整环境变量：

```text
env CARGO_TARGET_DIR=<temporary-root>/cargo-target \
  CARGO_BUILD_JOBS=2 \
  TMPDIR=<temporary-root>/tmp \
  KEELSHELL_OPENSSH_USER=keelshell \
  KEELSHELL_OPENSSH_PORT=65032 \
  KEELSHELL_OPENSSH_HOST_KEY=<temporary-root>/container_host_ed25519.pub \
  KEELSHELL_OPENSSH_CLIENT_KEY=<temporary-root>/client_ed25519 \
  KEELSHELL_OPENSSH_ROOT=/home/keelshell \
  RUST_BACKTRACE=1 \
  cargo test -p keelshell-session --test openssh_interop --locked \
  openssh_checked_content_reads_inspect_regular_files_and_refuse_truncation \
  -- --ignored --exact --test-threads=1

env CARGO_TARGET_DIR=<temporary-root>/cargo-target \
  CARGO_BUILD_JOBS=2 \
  TMPDIR=<temporary-root>/tmp \
  KEELSHELL_OPENSSH_USER=keelshell \
  KEELSHELL_OPENSSH_PORT=65032 \
  KEELSHELL_OPENSSH_HOST_KEY=<temporary-root>/container_host_ed25519.pub \
  KEELSHELL_OPENSSH_CLIENT_KEY=<temporary-root>/client_ed25519 \
  KEELSHELL_OPENSSH_ROOT=/home/keelshell \
  RUST_BACKTRACE=1 \
  cargo test -p keelshell-session --test openssh_interop --locked \
  openssh_acknowledged_pause_stops_writes_and_cancel_preserves_the_session \
  -- --ignored --exact --test-threads=1

env CARGO_TARGET_DIR=<temporary-root>/cargo-target \
  CARGO_BUILD_JOBS=2 \
  TMPDIR=<temporary-root>/tmp \
  KEELSHELL_OPENSSH_USER=keelshell \
  KEELSHELL_OPENSSH_PORT=65032 \
  KEELSHELL_OPENSSH_HOST_KEY=<temporary-root>/container_host_ed25519.pub \
  KEELSHELL_OPENSSH_CLIENT_KEY=<temporary-root>/client_ed25519 \
  KEELSHELL_OPENSSH_ROOT=/home/keelshell \
  RUST_BACKTRACE=1 \
  cargo test -p keelshell-session --test openssh_interop --locked \
  openssh_batch_exec_preserves_results_policy_and_unknown_timeout \
  -- --ignored --exact --test-threads=1
```

清理命令：

```text
podman rm --force keelshell-ssh-20261107-d2d0ce80a3-openssh
podman network rm keelshell-ssh-20261107-d2d0ce80a3-net
podman rmi localhost/keelshell-ssh-20261107-d2d0ce80a3:1.0
podman container exists keelshell-ssh-20261107-d2d0ce80a3-openssh   # 期望 exit 1
podman network exists keelshell-ssh-20261107-d2d0ce80a3-net          # 期望 exit 1
podman image exists localhost/keelshell-ssh-20261107-d2d0ce80a3:1.0 # 期望 exit 1
```

## 结果计数与证据

### 最终运行

最终运行项目为 `keelshell-ssh-20261107-d2d0ce80a3`，总耗时约 71.890 秒。
所有等待均有界：镜像 pull 180 秒、镜像 build 240 秒、容器启动 45 秒、SSH
readiness 30 秒、系统 SSH 20 秒、SFTP 30 秒、三个 Cargo 测试分别为 240/300/180
秒；清理操作每项 30 秒。

| 类别 | 结果 |
| --- | --- |
| Podman 引擎/机器检查 | 通过 |
| 基础镜像 pull 与 inspect | 通过 |
| 临时 OpenSSH image build 与 inspect | 通过 |
| 隔离 network create / container run | 通过 |
| SSH banner readiness | 通过，`SSH-2.0-OpenSSH_10.0` |
| container host key 读取与 `ssh-keyscan` 比对 | 通过 |
| 系统 SSH 命令 stdout/stderr | 通过：`podman-command-ok` / `podman-warning` |
| SFTP mkdir、list、31 字节 upload/download | 通过，下载字节完全一致 |
| 错误私钥路径 | 通过预期失败：exit `255`，`Permission denied (publickey)` |
| KeelShell `openssh_checked_content...` | 1 passed |
| KeelShell `openssh_acknowledged_pause...` | 1 passed；覆盖真实上传、暂停、取消、续传和 SSH exec |
| KeelShell `openssh_batch_exec...` | 1 passed；覆盖 stdout/stderr、非零退出、停止策略和超时未知态 |
| 最终运行 Cargo 失败 | 0 |
| 最终运行超时 | 0 |

`openssh_acknowledged_pause_stops_writes_and_cancel_preserves_the_session` 使用现有
测试中的 8 MiB 上传数据；最终测试输出为 `ok`，实际测试本身耗时 `0.88s`。

### 中间失败证据

中间失败没有被覆盖掉，且每次都在失败后执行了自己的清理：

1. 前两次使用项目名前缀
   `keelshell-ssh-20261107-1151594874` 和
   `keelshell-ssh-20261107-4b3b3d83b8`。容器启动、SSH banner、host key
   读取和 keyscan 均通过，但认证返回：

   ```text
   keelshell@127.0.0.1: Permission denied (publickey).
   ```

   随后的同前缀 debug 容器服务端日志确定根因：

   ```text
   User keelshell not allowed because account is locked
   ```

   入口脚本随后仅为临时测试账户移除 shadow lock，并继续关闭密码认证。

2. 修正账户锁定后，项目名前缀
   `keelshell-ssh-20261107-002c8362f5` 的系统 SSH、系统 SFTP 和第一个
   KeelShell 文件检查用例均通过。第二个 KeelShell 传输用例首次失败为：

   ```text
   Error: Invalid("local directory ancestor is a symlink or is not a directory")
   ```

   原因是 macOS `tempfile` 默认路径经过 `/var` symlink；没有修改产品规则或 Rust
   测试，而是将本轮测试根 canonicalize 到 `/private/var` 后精确重跑。最终运行的
   三个 KeelShell 用例全部通过。

3. 这两类失败是测试夹具修正的证据，不代表最终联调失败。所有中间项目名、容器、
   网络、临时自定义 image、密钥和临时目录均在各自 `finally` 清理中删除。

## 资源清理

最终运行的清理命令全部成功：

- `podman rm --force ...-openssh`：exit `0`
- `podman network rm ...-net`：exit `0`
- `podman rmi localhost/...:1.0`：exit `0`
- `podman container exists ...-openssh`：exit `1`，按预期不存在
- `podman network exists ...-net`：exit `1`，按预期不存在
- `podman image exists localhost/...:1.0`：exit `1`，按预期不存在
- canonicalized 临时根目录：`temporary_root_removed=true`
- 独立 Cargo target、client/wrong key、host key、known_hosts、SFTP batch、
  upload/download 文件：随临时根目录删除

清理后对两个本轮使用的前缀执行只读检查：

```text
podman ps --all --filter name=keelshell-ssh-20261107-
podman network ls --filter name=keelshell-ssh-20261107-
podman images --filter reference=localhost/keelshell-ssh-20261107-*
podman ps --all --filter name=keelshell-ssh-debug-
podman network ls --filter name=keelshell-ssh-debug-
podman images --filter reference=localhost/keelshell-ssh-debug-*
```

以上命令均没有输出自有容器、网络或临时镜像。共享基础镜像的保留边界已在
“环境与隔离资源”中说明；没有执行全局 `podman prune`，也没有删除其他项目资源。

## 未验证边界

- 没有修改或新增 Rust 测试，因此没有声称新增产品能力。
- 没有运行完整 `crates/keelshell-session/tests/openssh_interop.rs` 的全部 ignored
  用例；本次只选择与容器环境兼容且能形成最小真实 SSH/SFTP/exec 证据的 3 项。
- 依赖本机路径的 `openssh_resume_rejects_a_replaced_inode_without_writing_either_file`
  和带宿主机绝对路径假设的场景没有在容器中强行运行。
- 没有验证主机密钥变更、断线重连、代理/跳板、端口转发、键盘交互认证、SSH agent、
  真实外部服务器差异或生产凭据。
- 没有验证 Windows/Linux 原生窗口、IME、VoiceOver、文件管理器拖放或 UI 端到端流程。
- 没有验证外部 AI provider、真实 Agent 客户端或 Podman 之外的容器运行时。
- 容器内部 SFTP 使用 OpenSSH `internal-sftp`；这不证明所有第三方 SSH/SFTP 服务端
  的兼容性。
