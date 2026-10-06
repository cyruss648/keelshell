# ADR 0065: Bounded read-only workflow history

Status: implemented in an isolated candidate; independent review and native acceptance pending.

任务和已授权的定时计划仍然只在当前进程中存在。用户需要在任务完成后和重新打开应用时查看结果，但结果记录不能成为执行、恢复、重连或重放的授权。

Completed tasks need a readable history across application restarts. An audit record must not reconstruct an action or grant execution authority. The transient review, captured authenticated connection, schedule ticket and transport receipt remain the only existing admission/completion boundaries.

## Decision

- Add a separate domain ledger, `AppState.workflow_audits`. Each run stores a stable UUID, recording time, manual origin or finite schedule UUID/index/intended time, task UUIDs, opaque captured target UUIDs and fixed outcome enums. It stores no task names, command text, output, host addresses, credentials, custom parameters, command/parameter digests, review fingerprints, options or recovery tickets. Target IDs can be ephemeral and are not resolved to a current endpoint.
- Keep confirmed success, nonzero exit, explicit remote rejection, unknown outcome, cancellation before admission, failed prerequisites, failure-policy skips and fixed pre-admission reasons distinct. Cancellation observation does not prove that an admitted remote process stopped.
- Convert only an exact complete aggregate matching the reviewed plan, options, ordered task/target IDs and inner transport task IDs. Dependency skips must reference a declared prerequisite. Missing/mismatched aggregate receipts yield conservative unknown task results. A schedule's ledger status cannot prove task success.
- Record each terminal never-admitted schedule occurrence once. Missed windows, previous-run occupancy, cancellation and invalidation remain separate. Pending/due/running slots are not completed task results. No schedule or command is persisted.
- Retain at most 100 completed runs, 128 tasks per run and 2048 tasks in total. Evict complete oldest records by insertion order; wall-clock rollback does not reorder or deduplicate history. Loading validates at most 2048 task rows. A 100 × 128 worst-shape input is accepted through bounded FIFO insertion, leaving 16 complete runs; bypassing the retention API with an oversized ledger is rejected. The existing 4 MiB application-state cap remains unchanged.
- An identical stable run receipt is idempotent. Conflicting run IDs or duplicate finite occurrence identities are rejected atomically. Pending receipts are also checked before retention can hide an older conflict.
- Save through the existing asynchronous `StateStore` lease. Keep sanitized pending receipts until an actual save succeeds; show unsaved state on failure. Modal closure, a later successful save or explicit **Retry saving history** can flush them. There is no automatic error retry loop. Pending results survive hiding/replacing a panel, but unsaved results do not survive process exit.
- Reserve all 32 potential schedule occurrences before a new confirmation when the pending queue exceeds 68 records. Already armed occurrences retain their authorization. The pending queue is capped at 100 × 128 task results; an invariant violation reports that the new result was not saved instead of silently evicting it.
- Provide a read-only **Task history** view inside the workflow panel. Show a bounded run list and at most one run's 128 task details. Saving metadata is the only retry action; the history contains no execute, restore, reconnect or replay control. It can be opened without an active SSH session.

## Consequences and verification

中文和英文界面共同使用固定结果映射。旧状态文件没有新字段时读取为空；重启只读取已保存结果，不建立执行句柄或定时器。记录不导出到连接配置，也不上传。现有批量审计保持独立。

The field is backward compatible with old state documents. Restart loads only saved results and creates no workflow handle or scheduler. History stays in the local application state; connection export and existing batch audit remain separate.

Meaningful tests cover actual isolated state-store readback, worst-shape retention/disk bounds, wall-clock rollback, conflicting/repeated receipts, all projected outcome categories, incomplete/mismatched aggregates, real GPUI save errors and retries, modal close, restart, finite occurrence deduplication and read-only controls in both languages. Headless GPUI and typed receipts are engineering evidence; cross-platform native product acceptance remains open.

The candidate composes the previously frozen finite scheduling interfaces. Its scheduling dependency patch and task-audit increment are delivered separately so integration can apply the audit only after accepting the scheduling layer. The encrypted profile-sync implementation is not part of this change.
