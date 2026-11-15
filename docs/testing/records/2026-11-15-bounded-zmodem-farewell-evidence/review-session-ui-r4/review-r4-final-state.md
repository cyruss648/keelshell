# r4 HOLD retained; final live-tree check detected the parent test correction

The original `review-session-ui-r4.md`, machine report, notices, execution receipts and logs remain unchanged. r4 is **HOLD**, with P2-004 open for its exact frozen test input. P2-003 is corrected in r4 production source and the four new targeted cases.

`receipt-post-execution-freeze.json` verified all 571+12 inputs with no drift after the independent test commands. The later `receipt-final-freeze.json` correctly records **verified=false**, with exactly one drift: `crates/keelshell-app/src/ssh_bridge_tests.rs`. The parent had then begun P2-004's correction. There was no binary, metadata or inventory drift, and the r3 HOLD report remained unchanged. No final no-drift assertion is made for the r4 live tree after that correction.

`receipt-r4-superseded-by-live-test-fix.json` records the exact change: replace the old success propagation with the explicit output-cleanup timeout assertion, and strengthen the generic received-event check to exact 128 `x` bytes. Reconstructing the original r4 file in memory reproduces its frozen SHA. Production bridge source remains identical. The correction is assigned to the new r5 freeze and requires its own bound binary execution before closing P2-004.

Parent full-gate-r4 was stopped and reaped: **exit -15 / 235.951 seconds**. Its log hash matches the parent receipt. This supersedes the pending-start status described when the original r4 report was prepared; it does not change that report or turn the stopped gate into a pass. Full bridge r4 remains 14 passed / 1 failed / exit 101.
