# Buzz compatibility proof

[`buzz-main-proof.json`](buzz-main-proof.json) is the machine-readable report from a real run against an untouched checkout of `block/buzz` at the exact commit recorded in `buzzSha`.

The proof uses Buzz's actual Postgres migrations, Redis pub/sub, relay ingest/query/count/deletion handlers, authenticated WebSockets, Nostr signature checks, NIP-29 private-channel membership, and current kind-9 message behavior. It does not mock the relay or read its database from Comb.

## What passes

1. Three disposable identities create a private That’s Cool launch channel.
2. Six signed source messages are published.
3. Comb obtains stable exact counts before and after retrieval and verifies all six signatures.
4. Comb publishes a signed proposal with exact source receipts.
5. A separate human reviewer key signs approval.
6. Comb publishes the ratified record.
7. An uninvited identity cannot read the private channel.
8. The owner deletes a primary source.
9. Comb confirms the source is unavailable, publishes an empty-body invalidation, and deletes its derived record.
10. Repeated local receipt writes remain idempotent.

Coverage is intentionally reported as `self-attested`. Current Buzz can enforce channel access for the underlying events, but it does not yet understand or enforce Comb's proposal/review semantics.

## Reproduce

Prerequisites are a current Rust toolchain, Postgres, and Redis. Start an isolated Buzz relay from a clean checkout with automatic migrations and the object-store probe disabled for the message-only test:

```bash
DATABASE_URL=postgresql://<user>@localhost/<disposable-db> \
REDIS_URL=redis://localhost:6379 \
BUZZ_BIND_ADDR=127.0.0.1:4300 \
RELAY_URL=ws://127.0.0.1:4300 \
BUZZ_HEALTH_PORT=4301 \
BUZZ_METRICS_PORT=4302 \
BUZZ_AUTO_MIGRATE=true \
BUZZ_AUDIT_ENABLED=false \
BUZZ_GIT_CONFORMANCE_PROBE=false \
cargo run -p buzz-relay
```

Then, from the Comb repository:

```bash
cargo run -p comb-cli -- prove \
  --relay ws://127.0.0.1:4300 \
  --buzz-sha "$(git -C /path/to/buzz rev-parse HEAD)" \
  --report tests/e2e/buzz-main-proof.json \
  --store tests/e2e/buzz-main-proof.db
```

The SQLite file is ignored. The JSON report contains no private keys or source bodies.
