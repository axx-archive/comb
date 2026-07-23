# Contributing

Comb is early and the trust boundary matters more than feature count.

Before opening a pull request:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cd site && npm ci && npm test && npm run build
```

Changes must preserve channel scoping, independent reviewer signatures, evidence resolvability, explicit coverage language, and deletion invalidation. New cross-channel, private-message, transcript, or model-provider behavior requires a design issue before implementation.

Please keep upstream Buzz changes separate from Comb changes. A Buzz contribution should be useful to any compatible client or agent and must follow Buzz's own contribution guide.
