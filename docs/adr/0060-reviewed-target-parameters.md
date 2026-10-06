# ADR 0060: Reviewed per-target literal parameter mappings

Status: author-validated candidate; independent review, root integration and native acceptance pending.

The existing five metadata markers and shell-safe snippet parser form the syntax
contract. User markers reuse that grammar rather than introducing expressions or
unquoted substitution. A separate domain type validates bounded transient mappings,
refuses reserved/duplicate names and hides supplied values from Debug/errors.

UI derives names from source text and explicitly synchronizes each captured target's
fields. A workflow validates the union of parameters across tasks for one target,
then selects the exact task subset; this prevents silently accepting unused values.
Repeated markers reuse one mapping. Empty values require explicit selection.

Every final command remains in an immutable review. Fields/options/source changes
expire it. Batch dispatch captures SSH instances and uses the workflow's equivalent
same_connection / route / metadata checks, preventing a same-endpoint replacement
from receiving an older review. No resolver reconnects or schedules an unreviewed run.

Parameterized batches suppress persisted audit records entirely, including command
fingerprints; ordinary metadata/literal batches retain their prior audit behavior.
No user value, command, output or derived digest is added to history, configuration,
task logs or persistence. UI memory and the in-flight reviewed run retain necessary
plaintext only for their current lifetime. No new third-party dependency is needed.

Tests distinguish real GPUI render/event behavior and actual isolated TCP/SSH requests
from OpenSSH shell interoperability and native desktop acceptance. Windows/Linux
native evidence remains separate from portable compilation and fixtures.
