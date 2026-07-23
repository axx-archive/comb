## Problem

Buzz has excellent signed primitives for conversations, channels, human/agent identity, workflows, and approvals, but no durable event contract for turning a resolved discussion into evidence-backed project knowledge.

That gap is showing up from several directions:

- #2433 asks for a local knowledge read/write agent.
- #242 proposes a Human Signal Layer that can be queried historically.
- #1702 proposes quorum-gated finality for resolved threads.

An agent can summarize a channel into an ordinary message today, but the relay cannot distinguish a proposed artifact from prose, enforce that its evidence belongs to the same channel, verify that a human owner/admin ratified it, or project supersession without last-write-wins ambiguity.

## Narrow proposal

I would like to draft two new **channel-scoped signed event types**:

1. An append-only **knowledge proposal** containing a bounded set of claims, exact source-event references, optional supersession, and an explicitly self-attested coverage declaration.
2. A parameterized-replaceable **knowledge review** signed by a human reviewer and keyed by proposal ID. Its body stays empty; tags carry `approve` or `reject`.

The relay would validate:

- exactly one `h` channel tag;
- every source and superseded proposal exists, is non-deleted, and has exactly that channel;
- payload evidence IDs exactly equal the signed `source` tags;
- only a current human owner/admin may publish a counted review;
- agents, unknown principals, and ordinary members fail closed;
- strict bounds on claims, source IDs, and payload size.

This should require no SQL migration, side table, new HTTP endpoint, search service, or LLM/provider dependency. Old clients can ignore the new kinds. The generator remains an independent agent; Buzz gains a general protocol primitive any compatible agent can use.

## Deliberate V1 boundaries

- One channel only. Cross-channel synthesis needs a future audience-intersection or encrypted-view model.
- Coverage is generator-signed and **self-attested**, not relay proof of completeness.
- Source deletion makes support unavailable but cannot magically erase derived prose already delivered to clients.
- No private-DM collection.
- No huddle recording in this change.
- No event-kind numbers until maintainers indicate the appropriate registry placement.

## Working compatibility proof

I built [Comb](https://github.com/axx-archive/comb), an independent Apache-2.0 implementation by That's Cool, to test the product and trust model without requiring a Buzz fork.

The deterministic proof passed against untouched `block/buzz@acfbb1bb6af54cb29cb152496ff43b8285dcb8cf` using Buzz's actual Postgres migrations, Redis, authenticated WebSockets, private-channel enforcement, count/query handlers, signed events, and deletion path:

- 6/6 stable self-attested source coverage;
- outsider access denied;
- human reviewer signed their own review;
- deleted source became unavailable;
- dependent Comb record invalidated and deleted;
- restart receipt remained idempotent.

Proof report: https://github.com/axx-archive/comb/blob/main/tests/e2e/buzz-main-proof.json

Interactive explanation: https://comb-for-buzz.ajh-archive.chatgpt.site

Compatibility mode uses structured kind-9 messages, which validates that the product can work today but intentionally does **not** claim relay-enforced knowledge semantics.

## Proposed contribution sequence

If this direction fits Buzz:

1. I will open a focused protocol/relay PR with the two event types and exhaustive core/SDK/relay E2E coverage.
2. A separate PR can add a native desktop knowledge card and human review UI.
3. A later RFC can make locally consented, speaker-signed huddle transcript segments ordinary proposal sources—without central recording.

## Maintainer guidance requested

1. Does a channel-scoped knowledge proposal/review primitive belong in Buzz's event registry?
2. Should human ratification be owner/admin-only in V1, or should the proposal carry a snapshotted approver policy?
3. Would you prefer a docs-only NIP PR before implementation?
4. Where should maintainers reserve the event kinds if the shape is accepted?
