#!/usr/bin/env python3
"""Bind the r2 difference review and preserve the original HOLD byte for byte."""

import ast
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

REVIEW = Path(__file__).resolve().parent
HISTORY = REVIEW.parent
ROOT = REVIEW.parents[3]
BASE = ROOT / "work/windows-local-backend-20261115"


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(value, message):
    if not value:
        raise AssertionError(message)


def named_log(text):
    result = {}
    for line in text.splitlines():
        match = re.fullmatch(r"(test_\S+) \(([^)]+)\) \.\.\. (ok)", line)
        if match:
            name = match[2] if match[2].endswith("." + match[1]) else match[2] + "." + match[1]
            require(name not in result, "Duplicate named Python result")
            result[name] = match[3]
    return result


def test_names(path):
    names = set()
    for item in ast.parse(path.read_text()).body:
        if isinstance(item, ast.ClassDef):
            for method in item.body:
                if isinstance(method, ast.FunctionDef) and method.name.startswith("test_"):
                    names.add("test_linux_local_backend." + item.name + "." + method.name)
    return names


def main():
    initial = json.loads((REVIEW / "input-binding-start.json").read_text())
    freeze = json.loads((BASE / "linux-runner-inputs-r2.json").read_text())
    live = {}
    for path, record in initial["inputs"].items():
        data = (ROOT / path).read_bytes()
        copied = (REVIEW / "frozen-inputs" / path).read_bytes()
        require(digest(data) == digest(copied) == record["sha256"], "Input drift: " + path)
        live[path] = record["sha256"]
    require(all(live[path] == checksum for path, checksum in freeze["files"].items()),
            "r2 freeze mismatch")
    for path, record in initial["author_and_native_evidence"].items():
        require(digest((BASE / path).read_bytes()) == record["sha256"],
                "Author/native evidence drift: " + path)
    author_receipt = json.loads((BASE / "logs/linux-runner-python-r3.json").read_text())
    author_log = (BASE / "logs/linux-runner-python-r3.log").read_bytes()
    require(digest(author_log) == author_receipt["log_sha256"], "Author log SHA mismatch")
    require(author_receipt["exit"] == 0 and author_receipt["reaped"] and
            not author_receipt["timed_out"], "Author Python run not successful/reaped")
    expected_names = test_names(REVIEW / "frozen-inputs/scripts/test_linux_local_backend.py")
    original_names = test_names(HISTORY / "frozen-inputs/scripts/test_linux_local_backend.py")
    author_names = named_log(author_log.decode())
    unit_receipt = json.loads((REVIEW / "unit-r2.command.json").read_text())
    unit_log = (REVIEW / "unit-r2.stderr.log").read_text()
    independent_names = named_log(unit_log)
    require(len(expected_names) == 39 and set(independent_names) == set(author_names) == expected_names,
            "Named Python inventory/execution mismatch")
    require(original_names.issubset(expected_names) and len(expected_names - original_names) == 7,
            "Original tests lost or wrong added-test count")
    require(unit_receipt["returncode"] == 0 and unit_receipt["waited_and_reaped"] and
            not unit_receipt["timed_out"] and "Ran 39 tests" in unit_log and "\nOK\n" in unit_log,
            "Independent unit suite incomplete")
    probe_receipt = json.loads((REVIEW / "probes-r2.command.json").read_text())
    probe_names = named_log((REVIEW / "probes-r2.stderr.log").read_text())
    require(len(probe_names) == 8 and probe_receipt["returncode"] == 0 and
            probe_receipt["waited_and_reaped"] and not probe_receipt["timed_out"],
            "Reviewer probes incomplete")
    history = json.loads((REVIEW / "history-manifest-start.json").read_text())
    actual_history = {
        str(p.relative_to(HISTORY)) for p in HISTORY.rglob("*")
        if p.is_file() and not p.is_relative_to(REVIEW)
    }
    require(actual_history == set(history), "Original history file set changed")
    for path, record in history.items():
        require(digest((HISTORY / path).read_bytes()) == record["sha256"],
                "Original HOLD/probe/evidence bytes changed: " + path)
    old_tree = ast.parse((HISTORY / "frozen-inputs/scripts/run_linux_local_backend.py").read_text())
    new_tree = ast.parse((REVIEW / "frozen-inputs/scripts/run_linux_local_backend.py").read_text())
    unchanged_old = [
        ast.dump(node, include_attributes=False) for node in old_tree.body
        if not isinstance(node, ast.FunctionDef) or node.name != "execute"
    ]
    unchanged_new = [
        ast.dump(node, include_attributes=False) for node in new_tree.body
        if not isinstance(node, ast.FunctionDef) or node.name not in ("execute", "validate_binary")
    ]
    require(unchanged_old == unchanged_new, "Unexpected production change outside execute/new validator")
    old_native = json.loads((BASE / "linux-local/native-r3/receipt.json").read_text())
    old_runner_sha = old_native["host_runner_inputs"]["scripts/run_linux_local_backend.py"]
    require(old_runner_sha != live["scripts/run_linux_local_backend.py"], "Old native was relabeled")
    require(old_native["revision"] == initial["baseline"] and
            old_native["totals"] == {"passed": 251, "failed": 0, "ignored": 0},
            "Old native baseline/count mismatch")
    git_argv = ["git", "--no-optional-locks", "rev-parse", "HEAD", initial["baseline"] + "^{commit}"]
    git_result = subprocess.run(
        git_argv, cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        timeout=10, check=True, env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"},
    )
    current_head, baseline = git_result.stdout.decode().splitlines()
    require(baseline == initial["baseline"], "Baseline no longer resolves")
    signed = datetime.datetime.now(datetime.timezone.utc)
    readback = {
        "signed_utc": signed.isoformat(), "inputs": live, "drift": False,
        "baseline": baseline, "current_head_observed": current_head,
        "freeze_sha256": digest((BASE / "linux-runner-inputs-r2.json").read_bytes()),
        "original_history_files_unchanged": len(history),
        "production_ast_outside_execute_and_validate_binary_unchanged": True,
        "old_named_tests_preserved": len(original_names),
        "new_named_tests": sorted(expected_names - original_names),
        "independent_named_tests": independent_names, "reviewer_named_tests": probe_names,
        "git_command": {"argv": git_argv, "returncode": git_result.returncode},
        "old_native_runner_sha256": old_runner_sha,
        "old_native_revision": old_native["revision"],
        "old_native_totals": old_native["totals"],
        "native_execution_with_r2_runner": "NOT_RUN",
    }
    (REVIEW / "input-binding-end.json").write_text(json.dumps(readback, indent=2) + "\n")
    summary = {
        "recommendation": "APPROVE",
        "scope": "r2 Linux runner source difference and Python behavior only",
        "signed_utc": signed.isoformat(), "baseline": baseline, "inputs": live,
        "open_findings": {"P0": 0, "P1": 0, "P2": 0, "P3": 0},
        "closed_findings": ["LINUX-RUNNER-P2-001", "LINUX-RUNNER-P2-002"],
        "independent_python": {"run": 47, "passed": 47, "failed": 0, "skipped": 0,
                               "repository_suite": 39, "reviewer_boundary_probes": 8},
        "independent_rust_tests_executed": 0,
        "old_native_251_remains_old_source_and_old_runner": True,
        "native_execution_with_r2_runner": "NOT_RUN",
        "original_history_files_unchanged": len(history),
        "writes": "Only work/windows-local-backend-20261115/review-linux/r2/",
    }
    (REVIEW / "summary-approve-r2.json").write_text(json.dumps(summary, indent=2) + "\n")
    stamp = signed.astimezone(datetime.timezone(datetime.timedelta(hours=8))).isoformat()
    report = f"""**APPROVE — Linux runner r2 差分只读复核**

签署时间：{stamp}。开放 P0/P1/P2/P3 = **0/0/0/0**。
原两项 P2 在此精确 r2 冻结下关闭。批准范围为两个 Python 脚本的源码差分与 Python
行为，不签署新 runner 的 Linux 原生运行。父提供的冻结文件为
`{BASE / "linux-runner-inputs-r2.json"}`，SHA-256：
`{readback["freeze_sha256"]}`。

| 输入 | 已核对的 SHA-256 |
| --- | --- |
| scripts/run_linux_local_backend.py | `{live["scripts/run_linux_local_backend.py"]}` |
| scripts/test_linux_local_backend.py | `{live["scripts/test_linux_local_backend.py"]}` |
| scripts/run_linux_file_interop.py | `{live["scripts/run_linux_file_interop.py"]}` |
| scripts/openssh_interop.py | `{live["scripts/openssh_interop.py"]}` |

四个输入在起始、冻结副本、执行和签署读回之间零漂移。
原 Git 工程基线 `{baseline}`，本轮开始 HEAD 为该基线；
签署时观察到 HEAD 为 `{current_head}`。
两个新脚本以内容 SHA 签署，不将它们冒称为该 Git 基线已经包含的版本。
完整起止绑定为 [start]({REVIEW / "input-binding-start.json"}) 与
[end]({REVIEW / "input-binding-end.json"})。

**发现关闭与精确行证据**

| 原发现 | 修复位置 | 独立关闭证据 |
| --- | --- | --- |
| LINUX-RUNNER-P2-001：ignored 被误签 PASS | `/Users/hdh/.codex/worktrees/editor-unicode-codec-20261114/keelshell/scripts/run_linux_local_backend.py:525` | named_results 仍要求精确名称集合，execute 进一步拒绝任意 selected 的 ignored。仓库负控与正控均通过；独立 mixed 负控中 required 为 ok、额外 selected 为 ignored，仍为 exit 1 / FAIL，totals=1/0/1。 |
| LINUX-RUNNER-P2-002：finish/备用 signal 异常丢失终态回执 | 同文件 `:551`、`:564`、`:571`、`:591`、`:594` | 清理进入前实际读到 FINALIZING 与 primary error；继承 fallback 抛 PermissionError 后，primary 保留、owned_processes_stopped=false，两个终态回执保存，handlers 恢复。临时持久化失败不跳过清理。 |

上述位置均对应精确 runner SHA `{live["scripts/run_linux_local_backend.py"]}`，
稳定副本为 [runner]({REVIEW / "frozen-inputs/scripts/run_linux_local_backend.py"})。
原始 HOLD、notice-001/002、红回归、冻结和旧报告保持原字节；本轮对
**{len(history)} 份**历史文件逐项摘要与完整路径集合复核，零变化。
没有执行会覆盖旧输出的 probe_failure_paths.py、verify_delivery.py 或旧 finalizer。
本轮新探针全部位于 [probe_r2.py]({REVIEW / "probe_r2.py"})，场景原件保存在
`{REVIEW / "scenarios"}`。

**附加 binary 改动**

同文件 `:268` 的 validate_binary 要求 sha256sum 的摘要格式和目标路径精确匹配，
唯一 ELF Class/Data/Type/Machine 字段、ELF64、小端、EXEC/DYN，以及与前序已核对
VM/kernel/compiler 的架构一致；`:502` 在测试选集执行前实际调用该校验。
仓库三项 binary 测试通过。独立 execute 负控给出 x86-64 ELF、arm64 native tools，
在任何 selected test 执行前返回 FAIL，计数为 0/0/0。
另对六份旧 native-r3 保留 header/hash 文本应用新校验均通过；
这只是旧元数据的兼容性检查，不是新 binary 的构建、hash 或执行验收。

生产源码相对原 HOLD 的 AST 差分，仅 execute 与新增 validate_binary 变化；
Git 归档、公开 registry/checksum 白名单、命令预算、原身份核验/删除流程及所有
其他顶层定义/常量未变。两个继承 helper 的 SHA 也未变。
仓库原有 32 个 named tests 全部保留，新增 7 个；39 个库存名与父原始日志、
本 reviewer 的实际逐名结果完全一致，没有只核总数。

**实际执行**

所有命令由本轮 run_review_command.py 施加 45 秒外层预算，TMPDIR 仅在 r2，
保存实际 argv、stdout/stderr、退出码、elapsed、wait/reap。

| 主命令 | 实测结果 |
| --- | --- |
| `python3 -B -m unittest discover -s scripts -p test_linux_local_backend.py -v` | exit 0；39 passed / 0 failed / 0 skipped；外层 {unit_receipt["elapsed_seconds"]}s；wait/reap=true |
| `python3 -B work/windows-local-backend-20261115/review-linux/r2/probe_r2.py` | exit 0；8 passed / 0 failed / 0 skipped；外层 {probe_receipt["elapsed_seconds"]}s；wait/reap=true |
| `python3 -B work/windows-local-backend-20261115/review-linux/r2/finalize_r2.py` | 本次签署读回、库存/历史核验与报告生成；实际退出码见 finalize-r2.command.json |

本轮独立 Python **47 passed / 0 failed / 0 skipped**，即仓库专项 39 + reviewer 专项 8；
父的 39 次相同专项不叠加，旧轮 32/2 不叠加，合成场景内的 Rust 风格输出也不算 Rust 测试。
8 项 reviewer 边界分别覆盖：

- primary error 在 FINALIZING 中先于清理落盘，finish 异常与 handlers 恢复；
- required 通过而另一个 selected 被忽略，仍拒绝；
- 成功执行后 provisional 写入失败，仍清理且最终 FAIL；
- cleanup.json 写入失败，仍保存主回执 FAIL 并恢复 handlers；
- 最终 receipt 写入失败，旧盘面保持 FINALIZING、stdout 返回 FAIL，不能残留虚假 PASS；
- execute 中拒绝错误 ELF，未执行选定测试；
- 六份旧 header/hash 文本满足新校验，但不改签旧运行；
- 成功执行后 finish 失败，最终 FAIL 且清理未确认。

持久介质持续拒绝写入时，不能保证被拒绝的文件实际存在；已验证这种情况下不宣称 PASS，
不跳过清理、保留可写的证据并恢复 handlers。这是明确的 I/O 边界，不是回执已成功保存的声明。
场景中的预期 exit 1 是 Python 故障注入被正确拒绝，8 项 unittest 自身均为通过。

父原件 `logs/linux-runner-python-r3.log` 的 SHA-256 与其 JSON 回执匹配，
实为 39/39、exit 0、reaped；本轮独立执行另有自己的回执。
本轮 Git 仅开始和签署时两次只读
`git --no-optional-locks rev-parse HEAD {baseline}^{{commit}}`，
均 exit 0。未执行任何 Cargo/rustc/fmt、宿主 target、Podman 或完整 Linux 流程。
本轮没有创建 agent/chat/container/worktree；两个测试外层进程均已取回实际终态并回收，
签署外层的终态另见 finalize-r2.command.json。本轮持久写集仅为 r2 子目录。

**原生证据边界**

旧 `linux-local/native-r3` 的真实 `251 passed / 0 failed / 0 ignored` 仍绑定
**旧 Rust 源码提交 `{old_native["revision"]}` 和旧 runner**
`{old_runner_sha}`。其原 receipt SHA 与本轮起始摘要相同，没有改写。
**父尚未用修复后的 r2 runner 重跑 Linux，本 reviewer 也没有运行 Linux；
新 runner 的原生运行状态为 NOT_RUN。** 不能将原 251 项结果改签成新源码/新 runner 的验收。

本次 APPROVE 仅关闭脚本控制流、错误回执与 native binary 验证的源码/Python 发现。
Linux GPUI/桌面、x64、Windows、独立 rz/sz peer、完整 ACL/权限矩阵、全仓工程门禁、
IME/辅助技术、完整产品和发行验收继续按各自证据签署。
原 [HOLD 历史报告]({HISTORY / "report-hold-r1.md"}) 保持原结论、时间与全部原件。
"""
    (REVIEW / "report-approve-r2.md").write_text(report)
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
