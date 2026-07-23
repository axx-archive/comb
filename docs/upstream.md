# Buzz upstream plan

Comb should contribute a general primitive to Buzz, not ask Buzz to merge a branded agent or a parallel knowledge database.

## Sequence

1. Prove the compatibility demo on untouched Buzz main and publish the exact tested SHA.
2. Open an RFC issue: **Evidence-backed knowledge crystallization as channel-scoped signed events**.
3. Ask maintainers to select/reserve event kinds; do not allocate numbers unilaterally.
4. After maintainer alignment, open PR 1 for the protocol and relay kernel.
5. Follow separately with a native desktop reader/reviewer.
6. Consider consented speaker-signed huddle transcript segments only after the base primitive is accepted.

## Proposed first PR

Two channel-scoped signed event types:

- an append-only knowledge proposal with bounded claims, exact evidence tags, optional supersession, and explicitly self-attested coverage;
- a parameterized-replaceable human review keyed by reviewer and proposal, with an empty body and `approve` or `reject` tags.

Relay validation would require:

- exactly one channel tag;
- every source and superseded proposal to exist, be non-deleted, and share that channel;
- exact equality between payload evidence IDs and signed source tags;
- the reviewer to be a current human owner/admin, never an agent, unknown identity, or ordinary member;
- strict caps on claims, source IDs, and payload size.

The change needs no SQL migration, side table, HTTP endpoint, search indexer, or model dependency.

## Explicit non-goals

- No LLM or Comb service inside Buzz.
- No multi-channel synthesis.
- No server-side recording.
- No claim that self-attested coverage proves completeness.
- No automatic cascade deletion claim.
- No event-kind numbers before maintainer agreement.

## Later layers

- A native knowledge card with source links, visible support state, and conflict-safe supersession.
- CLI/MCP conveniences for proposal and review.
- Locally consented, speaker-signed transcript segments linked to the parent huddle and channel.
- A future audience-intersection model for permission-safe cross-channel synthesis.
