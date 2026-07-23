# Comb

**Independent, open-source DLC for [Buzz](https://github.com/block/buzz).**

Comb turns authorized Buzz conversations into reviewable organizational memory. Every proposed claim carries signed source receipts. Humans ratify or reject proposals with their own Buzz identities. When evidence becomes unavailable, Comb invalidates the dependent memory instead of quietly presenting stale prose as truth.

> **Status: working experimental compatibility build.** The deterministic proof passed against untouched `block/buzz@acfbb1bb6af54cb29cb152496ff43b8285dcb8cf`: stable 6/6 self-attested coverage, private-channel outsider denial, independently signed human review, deletion-driven invalidation, and restart idempotence. Comb currently speaks Buzz's public protocol using ordinary channel messages; the proposed upstream protocol would let Buzz enforce the knowledge semantics at ingest.

## The idea

Buzz captures the work. Comb keeps what the work taught the team.

- **Receipts, not vibes.** Claims point to exact signed Buzz events.
- **Humans ratify meaning.** Comb proposes; authorized people approve or reject.
- **Time stays visible.** Corrections supersede earlier knowledge without rewriting history.
- **Permissions still matter.** The first release is intentionally channel-local. No workspace-wide backdoor, private-DM collector, or database access.
- **Forgetting is a feature.** Deleted or inaccessible evidence invalidates dependent claims.

The deterministic That's Cool demo follows a product decision from discussion, through a counterargument and signed approval, into a ratified memory record, then proves that deleting source evidence invalidates the record.

## Architecture

Comb is a standalone Rust service and CLI. It connects to Buzz over authenticated WebSockets, reads only channels where its own Buzz identity is a member, and publishes signed events back into the same channel.

```text
Buzz channel events
        |
        v
  comb-buzz adapter ---- verifies signatures, scope, and coverage
        |
        v
  comb-engine ---------- proposes, reviews, ratifies, supersedes, invalidates
        |
        +---------------> local metadata store (IDs and digests, not source bodies)
        |
        v
signed Comb events in the original Buzz channel
```

See [the architecture contract](docs/architecture.md) for security boundaries and [the upstream plan](docs/upstream.md) for the proposed Buzz primitive.

## Workspace

| Crate | Responsibility |
| --- | --- |
| `comb-core` | Versioned artifact schemas, canonical digests, and validation |
| `comb-engine` | Deterministic proposal/review/ratification/invalidation state machine |
| `comb-buzz` | Authenticated public-protocol adapter for Buzz |
| `comb-store` | Minimal restart/idempotence metadata store |
| `comb-cli` | Fixture and operator commands |
| `combd` | Long-running channel worker |

## Local checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd site && npm ci && npm test && npm run build
```

The Buzz compatibility test is run against a clean checkout pinned to an exact upstream commit. Its report records that SHA, the relay version, signed event IDs, access-control assertions, and invalidation result.

Run the deterministic proof against an available Buzz relay:

```bash
cargo run -p comb-cli -- prove \
  --relay ws://127.0.0.1:4300 \
  --buzz-sha <exact-buzz-commit> \
  --report tests/e2e/buzz-main-proof.json \
  --store tests/e2e/buzz-main-proof.db
```

The disposable SQLite store is ignored. See [the complete reproduction notes](tests/e2e/README.md).

## Upstream relationship

Comb is designed to be useful without a Buzz fork. The first proposed upstream change is deliberately smaller than Comb: a channel-scoped knowledge proposal plus a replaceable human review event, with relay validation for same-channel evidence and owner/admin ratification. The generator remains independent.

That separation lets Buzz gain a general primitive that other agents can use without absorbing Comb's product opinions or an LLM dependency.

## STRIDE

Comb came from a bigger question: **what if the whole workplace remembered?**

Comb was developed while building STRIDE, an open operating system for how humans and agents talk, decide, make, and remember together. Comb brings one sharply scoped part of that intelligence architecture to Buzz.

## Built by That's Cool

That's Cool is a small lab for ideas we can't leave alone.

## License and independence

Licensed under Apache-2.0. Buzz is an open-source project by Block, Inc. Comb is an independent project by That's Cool and is not affiliated with, sponsored by, or endorsed by Block.
