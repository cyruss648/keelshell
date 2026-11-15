#!/usr/bin/env python3
"""Sign the original HOLD without applying it to the parent's in-progress fixes."""

import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess

REVIEW = Path(__file__).resolve().parent
ROOT = REVIEW.parents[2]
DELIVERY = ROOT / "work/windows-local-backend-20261115/linux-local"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    initial = json.loads((REVIEW / "input-binding-start.json").read_text())
    handoff = json.loads((DELIVERY / "handoff.json").read_text())
    frozen = REVIEW / "frozen-inputs"
    records = []
    for item in initial["inputs"]:
        path = item["path"]
        if path.startswith("scripts/"):
            if path.endswith("test_linux_local_backend.py"):
                source = DELIVERY / "python-r2" / Path(path).name
            else:
                source = DELIVERY / "native-r3/host-runner-inputs" / Path(path).name
        else:
            source = ROOT / path
        data = source.read_bytes()
        if digest(data) != item["sha256"]:
            raise AssertionError("Original evidence changed: " + str(source))
        target = frozen / path
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists():
            raise AssertionError("Refusing to replace frozen inputs")
        target.write_bytes(data)
        current = digest((ROOT / path).read_bytes())
        records.append({
            "path": path, "reviewed_sha256": item["sha256"],
            "frozen_copy": str(target), "frozen_copy_sha256": digest(target.read_bytes()),
            "live_sha256_at_signing": current,
            "live_matches_original_review": current == item["sha256"],
        })
    argv = ["git", "--no-optional-locks", "rev-parse", "HEAD",
            initial["baseline"] + "^{commit}"]
    result = subprocess.run(
        argv, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        timeout=10, check=True, env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
    )
    current_head, baseline = result.stdout.decode().splitlines()
    if baseline != initial["baseline"]:
        raise AssertionError("Original baseline no longer resolves")
    signed = datetime.datetime.now(datetime.timezone.utc)
    readback = {
        "signed_utc": signed.isoformat(), "baseline": baseline,
        "current_head_observed": current_head, "inputs": records,
        "git_command": {"argv": argv, "returncode": result.returncode},
        "scope": "Historical HOLD of original handoff; no signature of parent fixes",
    }
    (REVIEW / "input-binding-end-hold-r1.json").write_text(
        json.dumps(readback, indent=2) + "\n"
    )
    audit = json.loads((REVIEW / "delivery-audit.json").read_text())
    if audit["status"] != "PASS":
        raise AssertionError("Final retained-evidence audit is not PASS")
    unit = json.loads((REVIEW / "unit-r1.command.json").read_text())
    probes = json.loads((REVIEW / "probes-r1.command.json").read_text())
    if unit["returncode"] != 0 or probes["returncode"] != 1:
        raise AssertionError("Unexpected unit/probe outcome")
    review_time = signed.astimezone(datetime.timezone(datetime.timedelta(hours=8)))
    live_drifts = [r["path"] for r in records if not r["live_matches_original_review"]]
    summary = {
        "status": "HOLD", "historical_report": True,
        "signed_at_asia_shanghai": review_time.isoformat(),
        "baseline": baseline, "reviewed_host_inputs": handoff["host_inputs"],
        "open_findings": {"P0": 0, "P1": 0, "P2": 2, "P3": 0},
        "findings": [
            {"id": "LINUX-RUNNER-P2-001", "line": 503,
             "notice": "notice-001-required-tests-ignored.md"},
            {"id": "LINUX-RUNNER-P2-002", "line": 530,
             "notice": "notice-002-cleanup-error-loses-receipt.md"},
        ],
        "independent_python": {"run": 34, "passed": 32, "failed": 2, "skipped": 0},
        "independent_rust_tests_executed": 0,
        "retained_author_native_tests_verified": {
            "run": 251, "passed": 251, "failed": 0, "ignored": 0,
        },
        "retained_evidence_audit": {"status": "PASS", "checks": len(audit["checks"])},
        "live_input_drift_paths_at_signing": live_drifts,
        "parent_fixes_reviewed": False,
        "writes": "Only work/windows-local-backend-20261115/review-linux/",
        "cargo_rustc_container_agent_chat_worktree_commands_executed": 0,
    }
    (REVIEW / "summary-hold-r1.json").write_text(json.dumps(summary, indent=2) + "\n")
    runner_copy = frozen / "scripts/run_linux_local_backend.py"
    fixture_copy = frozen / "scripts/run_linux_file_interop.py"
    report = f"""**HOLD — 新非作者 Linux runner 严格只读审查（原冻结 r1 历史报告）**

签署时间：{review_time.isoformat()}。P0/P1/P2/P3 = **0/0/2/0**。
本报告保留原始 HOLD，不批准父正在进行的修复。父已明确实际读到 notice-001/002，
会在修复后提供最终 SHA；该候选需另行差分复核。本轮其余审查已经完成。

审查范围为 `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell`
中的 `scripts/run_linux_local_backend.py`、`scripts/test_linux_local_backend.py`、
`work/windows-local-backend-20261115/linux-local/README.md`、`handoff.json` 及所需原件。
已完整读取 AGENTS.md、docs/HANDOFF.md、docs/testing/STRATEGY.md 和 RTK 指令。
Rust 工程基线为 `{baseline}`。起始 HEAD 与该基线一致；
签署时仅观察到 HEAD `{current_head}`，不将父的其他并行改动纳入本报告。
两个新 Python 脚本按下表 SHA-256 签署，不能将它们冒称为基线提交里已存在的脚本。

| 冻结宿主输入 | 本次审查 SHA-256 |
| --- | --- |
| scripts/run_linux_local_backend.py | `{handoff["host_inputs"]["scripts/run_linux_local_backend.py"]}` |
| scripts/test_linux_local_backend.py | `{handoff["host_inputs"]["scripts/test_linux_local_backend.py"]}` |
| scripts/run_linux_file_interop.py | `{handoff["host_inputs"]["scripts/run_linux_file_interop.py"]}` |
| scripts/openssh_interop.py | `{handoff["host_inputs"]["scripts/openssh_interop.py"]}` |

原始冻结的完整副本保存在 [frozen runner]({runner_copy}) 等
`{frozen}` 路径。起止读回见 [起始绑定]({REVIEW / "input-binding-start.json"}) 与
[签署读回]({REVIEW / "input-binding-end-hold-r1.json"})。
签署时与原冻结不同的实时路径为：{json.dumps(live_drifts, ensure_ascii=False)}。
这些差异只记录为父修复中的实时观察，不改写原报告，也未在本轮获批。

**开放发现**

1. **P2 / LINUX-RUNNER-P2-001：必需测试被 ignored 后仍可 PASS。**
   原位置
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_local_backend.py:503`，
   相关统计 249–265、最终 PASS 519；稳定行证据为
   [冻结 runner:503]({runner_copy}:503)。
   当必需名称仍在库存，但测试被标为 `#[ignore]` 或平台条件忽略时，
   Rust harness 返回零且逐名输出 ignored。当前门禁只检查非零返回码、timeout 和 failed，
   因此该行为并未实际执行也能被签成 PASS。独立注入完整 execute 路径，
   保留全部必需名，实得 `exit 0 / PASS / passed 0 / failed 0 / ignored 8`。
   应要求必需名称得到实际 ok，或拒绝这些验收选集中任何 ignored，并加执行层回归。
   [立即 notice]({REVIEW / "notice-001-required-tests-ignored.md"})、
   [错误 PASS 原回执]({REVIEW / "probe-all-ignored-receipt.json"})。

2. **P2 / LINUX-RUNNER-P2-002：清理异常可跳过两个终态回执。**
   原位置
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_local_backend.py:530`；
   继承的 fallback 位于
   `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_file_interop.py:245`。
   稳定行证据为 [冻结 runner:530]({runner_copy}:530) 与
   [冻结 fixture:245]({fixture_copy}:245)。
   若 stop_owned 的身份观察失败，随后 fallback signal_owned 再遇 OSError/PermissionError，
   继承的 finish 会向外抛错。execute 在回执写入和 signal handler 恢复之前无保护调用 finish，
   导致主失败丢失、cleanup.json/receipt.json 均不存在、清理状态无终态证据。
   独立注入真实继承 finish，实得 PermissionError 且两个回执均缺失。
   应保护清理及最终持久化/handler 恢复，保留 primary error、cleanup error 和未确认状态，
   并新增执行层回归。[立即 notice]({REVIEW / "notice-002-cleanup-error-loses-receipt.md"})、
   [实际观察]({REVIEW / "probe-cleanup-error.json"})。

两项探针均以合成 command/process 记录替代外部操作，未运行 Cargo、rustc、Podman 或真实容器。
它们证明的是控制流缺陷，不把 mock 执行计作 Linux 原生验收，也不宣称 native-r3 泄漏了资源。

**其余审查与原件核验**

- 精确归档：只见固定提交 `git archive` 的 Cargo.toml、Cargo.lock、crates 输入，
  不复制父正在漂移的工作树。独立解压保留 gzip，在内存中核对 tar comment 的提交、
  精确 ls-tree 路径集合、573 个 blob ID 与 cat-file 原文字节；573/573 一致。
  运行后的原始 sha256sum 日志也逐项匹配 573 个文件。
  原 tar SHA-256 为 `1588c0927d10cecf45839b30639ac14a54d5dc6e8f26aee82e97d99128236a3c`；
  清单 SHA-256 为 `3263de3f6412d10570cf68dcb281ee5301110d902db4e8ff95ba8eb95e8522fe`；
  gzip SHA-256 为 `df4dcc055cfc4f86eff7dbbfb921f62b897fc43670eb71bf15e6c1dade2c5957`。
- Registry：源码仅按官方 crates.io 锁项构造明确的 crate 文件名和 sparse-index 路径；
  不遍历或复制 Cargo home/config/credentials。crate 归档在打包前比对锁定 checksum；
  非官方 source、缺失 checksum、摘要错误、普通链接和越界祖先的专项均已通过。
  独立从归档 Cargo.lock 对账 978 个公开锁项、949 个 cached checksum、29 个缺失项、
  855 个唯一 sparse 路径；不把缺失缓存改签成完整工作区离线就绪。
  index 原始内容只记录摘要，公开性的依据是显式公开缓存根与锁定包路径白名单，
  不把该机制描述成在线来源认证。
- 工具链和执行绑定：原始日志为 Rust/Cargo 1.98.1、
  aarch64-unknown-linux-gnu、Linux aarch64，与 VM arm64 一致；
  六个 ELF header 均为 AArch64。编译、库存、选集、实际执行 argv 和逐名结果一致，
  使用 UID 1000、只读 snapshot、容器内 /linux-target 与 /linux-cargo，
  --locked/--offline，无宿主 target 挂载、网络下载或 --ignore-rust-version。
- 251 个执行名按 package/target/name 去重；七组分别为 25、1、25、18、102、40、40，
  `251 passed / 0 failed / 0 ignored`，native-r3 实际外层耗时 55.609 秒。
  该 251 是作者原件中核验的结果，本 reviewer 的独立 Rust 执行数为 0；
  不累加 r1/r2 的重复执行。六份库存的完整数量分别为
  core archive 40、core PAX 40、core Zmodem 102、
  session archive 18、session lib 187、session Zmodem 25。
- 有界回收：原件中的 native-r3 46 条命令都有正预算、真实返回码、
  timed_out=false 与 owned_processes_stopped=true；四次容器身份读回均同时匹配完整 ID、
  唯一名称、fixture label 与 owner label。rm 使用完整 ID，随后 exists 返回 1。
  最终原件另保留 probe/r1/r2/r3 四个 owned 容器的 absence 读回。
  Python 现有专项实际执行有界命令与其子孙进程的回收，也覆盖创建歧义和拒绝外来身份。
  该正常路径证据不关闭上列清理异常 P2。
- 保留失败：probe 中 `.Id` 模板的两次 exit 125/原始 stderr、后续 `.ID` 恢复和清理均在；
  Python 作者修复前的 `2 errors / exit 1` 与最终 32 项成功分别保留。
  独立对 `evidence-manifest.json` 列出的 401 份原件、7,073,210 字节全部核对摘要成功。

上述核验结果见 [delivery-audit.json]({REVIEW / "delivery-audit.json"})。
审核没有重读用户真实缓存内容或重新构造 registry.tar；该传输 tar 已由作者清理，
故 949 个历史 payload 的证据是原 registry manifest、明确源码检查与合成缓存专项，
不能冒称本 reviewer 已重新逐字校验这 949 个真实包。
容器中的测试二进制亦随清理回收；本轮核验其保留 checksum/readelf/库存/执行日志，
没有再次独立 hash 或运行已回收的 binary。

**实际执行与计数**

以下主命令均通过同目录 `run_review_command.py` 捕获 argv、45 秒外层预算、
stdout/stderr、真实 exit、elapsed、wait/reap。所有 TMPDIR 和写入均限制在 review-linux。

| 标签 | 主命令 | 实测结果 |
| --- | --- | --- |
| unit-r1 | `python3 -B -m unittest discover -s scripts -p test_linux_local_backend.py -v` | exit 0；32 passed / 0 failed / 0 skipped；外层 1.182s |
| probes-r1 | `python3 -B work/windows-local-backend-20261115/review-linux/probe_failure_paths.py` | exit 1；2 tests / 2 failures，独立期望行为均呈红；外层 0.123s |
| evidence-r1 | `python3 -B work/windows-local-backend-20261115/review-linux/verify_delivery.py` | exit 1；审核器误把 handoff 的 executable SHA 当成 JSON 文件 SHA；已保留失败原件 |
| evidence-r2 | 同上，纠正字段理解后复跑 | exit 0；7 组证据核验全部 PASS；外层 0.144s |
| final-readback | `python3 -B work/windows-local-backend-20261115/review-linux/finalize_historical_review.py` | 本次签署读回与历史报告生成；外层实际终态由 final-readback.command.json 单独记录 |

共 5 次捕获的专项/核验主命令；Python 行为用例共 34 次：32 passed、2 failures、0 skipped。
证据核验的 7 组是检查项，不冒充额外测试。两个 audit 尝试内部各有三条只读 Git 子命令：
`git --no-optional-locks rev-parse HEAD <baseline>^{{commit}}`、
`git --no-optional-locks ls-tree -rz --full-tree <baseline> -- Cargo.toml Cargo.lock crates`、
`git --no-optional-locks cat-file --batch`（573 个对象经 stdin）。
另有起始和签署时各一次 rev-parse，合计 8 次只读 Git 进程；未修改 Git 状态。
普通读取文件/rg 命令不计作测试或专项主命令。

首次 audit 的错误属于 reviewer 核验器，不计作新的产品发现；
修正前的 `delivery-audit-r1.json`、evidence-r1 日志与 exit 1 都保留。
本轮没执行 Cargo/rustc/fmt、宿主 target、整套 Linux 或任何 Podman 命令，
没创建 agent/chat/container/worktree，没修改源码、主文档或 Git。
我方所有持久写入均为指定 review-linux 目录内的报告、冻结副本、探针及日志。

**建议与证据边界**

原冻结 runner 仍为 **HOLD**，开放两项 P2；P0、P1、P3 显式为零。
现有 native-r3 固定基线的 Linux arm64 后端与所列 codec 原件绑定核验通过，
两项控制流缺陷不抹除其历史 251/251 成功事实。
最终修复的源码/测试 SHA 与针对性差分回归尚待父提供并另签；
本报告不预先批准该修复，也不要求重做作者整套 Linux 流程。

不签 Linux GPUI/桌面、x64、Windows、新独立 rz/sz peer、完整 ACL/权限继承矩阵、
IME/辅助技术、全仓工程门禁、发行或完整产品验收。父正在集成的 Windows 文件与其他并行
改动不属于本轮审查。原 strict peer HOLD 与其他未验收范围保持各自原状态。
"""
    (REVIEW / "report-hold-r1.md").write_text(report)
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
