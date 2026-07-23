# Architecture contract

This document defines what Comb may claim and what the first release must enforce.

## Product boundary

Comb's target product contract is an evidence-backed, channel-local organizational memory agent. It converts authorized Buzz activity into proposed knowledge, obtains independently signed human review, publishes a ratified record, and invalidates that record when its evidence is deleted or becomes inaccessible.

Comb is not a workspace-wide surveillance bot, a Buzz database reader, a private-DM collector, a huddle recording service, or a custodian of user private keys.

## Current implementation status

The deterministic knowledge kernel (`comb-core`, `comb-engine`, and `comb-store`) and the real Buzz protocol compatibility path (`comb-buzz` and `comb-cli`) are tested separately. The proof harness controls disposable owner, agent, reviewer, and outsider identities so it can verify signatures and relay behavior without external credentials. It does not prove that a human clicked an approval UI.

`combd` currently authenticates, checks authorized channel state, and fails closed when access is lost. Folding live Buzz events through the kernel and publishing governed records from the long-running worker remains integration work. The production trust model below is therefore a contract for that integration, not a claim about the disposable proof harness.

## Trust model

1. Comb owns one Buzz identity and key. It never holds or uses a reviewer's key.
2. An owner or admin explicitly adds Comb to each source channel.
3. Every source query includes that channel's `h` scope.
4. Every proposal, review, record, and invalidation is published into the evidence channel.
5. Comb revalidates channel membership and source access before proposing, before publishing a ratified record, and whenever a record is refreshed or read.
6. Losing channel membership stops processing and publication for that channel.
7. Cross-channel synthesis is out of scope until an audience-intersection model can be enforced.

## Evidence and claims

Each evidence reference records the relay, community, channel, event ID, kind, author, source creation time, semantic role, content digest, and exact locator. The source body is not copied into Comb's local metadata store.

Claims have one of four states:

- `asserted`: backed by resolvable primary evidence;
- `inferred`: derived and explicitly labelled as interpretation;
- `unsupported`: no longer adequately supported;
- `superseded`: replaced by a newer ratified claim without rewriting history.

Conflicting reviews produce a contested proposal. Last-write-wins is not a governance rule.

## Time

Comb distinguishes source creation, observation, and record publication times. A claim may also have `validFrom` and `validUntil`. Historical views never bypass current deletion or authorization checks.

## Coverage

Compatibility-mode coverage is self-attested by Comb. A bounded window may be called complete only when:

- the window ends before capture begins;
- exact counts before and after capture agree;
- pagination retrieves the same number of events;
- every event signature validates;
- every evidence reference resolves;
- no query, membership, adapter, or limit error occurred.

This is not relay-issued proof. The UI and documentation must say `self-attested coverage` until Buzz exposes a signed query snapshot or equivalent primitive.

## Deletion and forgetting

If source evidence is deleted or becomes inaccessible, Comb:

1. marks each dependent claim unsupported;
2. publishes a body-free invalidation receipt containing only IDs and digests;
3. removes its own derived compatibility messages using Buzz's deletion path where authorized;
4. does not silently regenerate the same claim from substitute evidence.

Derived prose already delivered to clients cannot be cryptographically un-read. Comb therefore promises invalidation and minimization, not magical deletion propagation.

## Compatibility protocol

The first release uses signed Buzz kind `9` messages so it works without a fork. Each event includes exactly one `h` channel tag plus custom tags:

```text
["comb", "proposal|review|record|invalidation", "v1"]
["comb-id", "<stable id>"]
["e", "<source event id>", "", "source"]
["e", "<prior proposal id>", "", "supersedes"]
```

These messages demonstrate the end-to-end product interaction, but the current relay does not understand their semantics. Same-channel evidence, reviewer authority, and invalidation are therefore verified by Comb rather than enforced by Buzz.
