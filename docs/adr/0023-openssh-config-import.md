# ADR 0023: Reviewable OpenSSH configuration import

## Context

Users often already maintain SSH endpoints in an OpenSSH configuration. Importing that file must not execute shell syntax, expand an environment value, read arbitrary paths, or silently turn a conditional rule into a different destination.

## Decision

KeelShell accepts a bounded, deterministic subset from clipboard text: exact `Host` aliases, `HostName`, `Port`, `User`, `IdentityFile`, `ProxyJump`, and explicitly selected `Include` contents. Include files are supplied by the caller as a literal path-to-content map; the parser never reads the filesystem. Exact aliases are indexed before jump references are resolved, so an unresolved `ProxyJump` fails closed. Credentials remain agent references or private-key paths and never include secret bytes.

Wildcard and conditional blocks, proxy commands, shell expansions, and other connection-semantic directives are skipped into a typed warning report. The connection manager exposes the report to the UI, which imports only validated entries and tells the user how many source items need review. Import is all-or-nothing for the connection library; a parser or route error leaves the previous state unchanged.

## Consequences

The importer is safe to use with a copied configuration and provides a useful first pass for common profiles. It does not claim to be a complete OpenSSH evaluator. Users must review skipped directives and configure unsupported proxy or conditional behavior explicitly in the connection editor. A future file-picker workflow may add caller-selected Include contents without changing parser policy.
