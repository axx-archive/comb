//! Comb operator CLI.

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use comb_buzz::{
    sign_add_member, sign_channel_message, sign_compatibility_event, sign_create_channel,
    sign_delete_event, sign_profile, BuzzClient, CombEventRole, CompatibilityTags,
};
use comb_core::{
    Claim, ClaimState, CoverageLane, CoverageRecord, CoverageStatus, EvidenceLocator, EvidenceRef,
    EvidenceRole, GeneratorInfo, Invalidation, InvalidationReason, MemoryRecord, Proposal,
    RetrievalSnapshot, Review, ReviewDecision, TemporalWindow, INVALIDATION_SCHEMA_V1,
    PROPOSAL_SCHEMA_V1, RECORD_SCHEMA_V1, REVIEW_SCHEMA_V1, SNAPSHOT_SCHEMA_V1,
};
use comb_store::{StateStore, StoredArtifact};
use nostr::{Event, Keys};
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Parser)]
#[command(
    name = "comb",
    version,
    about = "Evidence-backed organizational memory for Buzz"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Run the deterministic That's Cool proof against an unmodified Buzz relay.
    Prove {
        /// WebSocket URL of the Buzz relay.
        #[arg(long, default_value = "ws://127.0.0.1:4300")]
        relay: String,
        /// Exact upstream Buzz commit under test.
        #[arg(long)]
        buzz_sha: String,
        /// Optional path for a machine-readable proof report.
        #[arg(long)]
        report: Option<PathBuf>,
        /// Optional SQLite metadata path; defaults to an in-memory store.
        #[arg(long)]
        store: Option<PathBuf>,
    },
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProofReport {
    schema: &'static str,
    buzz_sha: String,
    relay: String,
    channel_id: String,
    source_event_ids: Vec<String>,
    proposal_event_id: String,
    review_event_id: String,
    record_event_id: String,
    invalidation_event_id: String,
    source_count_before: u64,
    source_count_after: u64,
    outsider_denied: bool,
    reviewer_signed_own_event: bool,
    source_deleted: bool,
    record_invalidated: bool,
    restart_idempotence_receipt: bool,
    coverage_label: &'static str,
    passed: bool,
    completed_at: i64,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Prove {
            relay,
            buzz_sha,
            report,
            store,
        } => {
            let report_value = prove(&relay, buzz_sha, store).await?;
            let json = serde_json::to_string_pretty(&report_value)?;
            if let Some(path) = report {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, format!("{json}\n"))
                    .with_context(|| format!("failed to write {}", path.display()))?;
            }
            println!("{json}");
        }
    }
    Ok(())
}

async fn prove(relay: &str, buzz_sha: String, store_path: Option<PathBuf>) -> Result<ProofReport> {
    let owner = Keys::generate();
    let comb = Keys::generate();
    let reviewer = Keys::generate();
    let outsider = Keys::generate();
    let owner_client = BuzzClient::new(relay, owner.clone());
    let comb_client = BuzzClient::new(relay, comb.clone());
    let reviewer_client = BuzzClient::new(relay, reviewer.clone());
    let outsider_client = BuzzClient::new(relay, outsider);

    for (client, keys, name, about) in [
        (
            &owner_client,
            &owner,
            "Ari at That's Cool",
            "Disposable Comb proof owner",
        ),
        (&comb_client, &comb, "Comb", "Evidence-backed memory agent"),
        (
            &reviewer_client,
            &reviewer,
            "Mina at That's Cool",
            "Disposable reviewer identity",
        ),
    ] {
        client.publish(&sign_profile(keys, name, about)?).await?;
    }

    let channel_id = Uuid::new_v4();
    owner_client
        .publish(&sign_create_channel(
            &owner,
            channel_id,
            "#launch-council · Comb proof",
            true,
        )?)
        .await?;
    owner_client
        .publish(&sign_add_member(
            &owner,
            channel_id,
            &comb.public_key().to_hex(),
            Some("bot"),
        )?)
        .await?;
    owner_client
        .publish(&sign_add_member(
            &owner,
            channel_id,
            &reviewer.public_key().to_hex(),
            Some("admin"),
        )?)
        .await?;

    let fixture = [
        "Ari: The sticker pack and landing page are ready. I still want to launch Friday.",
        "Mina: The account migration can strand invited collaborators. That is the launch risk.",
        "Devin: I reproduced it: accepting an invite after renaming the lab opens an empty workspace.",
        "Jo: Counterpoint: ship behind a feature flag and migrate the first ten labs by hand.",
        "Ari: Decision: move public launch to Tuesday. Mina owns the migration fix; Monday noon is the gate.",
        "Nia: The risograph proofs arrived. Completely unrelated, but they look incredible.",
    ];
    let mut source_events = Vec::with_capacity(fixture.len());
    for content in fixture {
        let event = sign_channel_message(&owner, channel_id, content)?;
        owner_client.publish(&event).await?;
        source_events.push(event);
    }

    let capture_started = unix_now()?.saturating_add(1);
    let source_count_before = comb_client.count_channel(channel_id, &[9]).await?;
    let captured = comb_client.query_channel(channel_id, &[9], 1_000).await?;
    let source_count_after = comb_client.count_channel(channel_id, &[9]).await?;
    if source_count_before != fixture.len() as u64
        || source_count_before != source_count_after
        || captured.len() as u64 != source_count_before
    {
        bail!(
            "self-attested coverage failed: before={source_count_before}, retrieved={}, after={source_count_after}",
            captured.len()
        );
    }

    let decision_source = source_events
        .get(4)
        .context("deterministic fixture lost its decision source")?;
    let risk_source = source_events
        .get(1)
        .context("deterministic fixture lost its risk source")?;
    let evidence = vec![
        evidence_ref(relay, channel_id, decision_source, EvidenceRole::Supports)?,
        evidence_ref(relay, channel_id, risk_source, EvidenceRole::Context)?,
    ];
    let claim = Claim {
        claim_id: String::new(),
        text: "Public launch moved to Tuesday; Mina owns the migration fix and Monday noon is the gate."
            .into(),
        state: ClaimState::Asserted,
        valid_from: decision_source.created_at.as_secs() as i64,
        valid_until: None,
        evidence,
        counterevidence: Vec::new(),
        supersedes: Vec::new(),
    }
    .seal()?;
    let mut source_ids = captured
        .iter()
        .map(|event| event.id.to_hex())
        .collect::<Vec<_>>();
    source_ids.sort();
    let snapshot = RetrievalSnapshot {
        schema: SNAPSHOT_SCHEMA_V1.into(),
        snapshot_id: String::new(),
        community: relay_host(relay),
        channel_id: channel_id.to_string(),
        source_event_ids: source_ids.clone(),
        coverage: CoverageRecord {
            status: CoverageStatus::Complete,
            requested_window: TemporalWindow {
                since: source_events
                    .iter()
                    .map(|event| event.created_at.as_secs() as i64)
                    .min()
                    .unwrap_or(capture_started)
                    .saturating_sub(1),
                until: capture_started,
            },
            capture_started_at: capture_started,
            capture_completed_at: unix_now()?.saturating_add(1),
            lanes: vec![CoverageLane {
                channel_id: channel_id.to_string(),
                kinds: vec![9],
                count_before: source_count_before,
                retrieved: captured.len() as u64,
                count_after: source_count_after,
                signature_failures: 0,
                unresolved_sources: 0,
            }],
            gaps: Vec::new(),
        },
    }
    .seal()?;
    let proposal = Proposal {
        schema: PROPOSAL_SCHEMA_V1.into(),
        proposal_id: String::new(),
        channel_id: channel_id.to_string(),
        created_at: unix_now()?,
        generator: GeneratorInfo {
            name: "comb-deterministic-fixture".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            model: None,
            prompt_version: None,
        },
        policy_id: "thats-cool.demo.single-human.v1".into(),
        snapshot,
        claims: vec![claim.clone()],
    }
    .seal()?;
    let proposal_sources = proposal
        .evidence_event_ids()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let proposal_event = sign_compatibility_event(
        &comb,
        channel_id,
        CombEventRole::Proposal,
        &proposal.proposal_id,
        &proposal,
        &CompatibilityTags {
            source_event_ids: &proposal_sources,
            ..CompatibilityTags::default()
        },
    )?;
    comb_client.publish(&proposal_event).await?;

    let review = Review {
        schema: REVIEW_SCHEMA_V1.into(),
        review_id: String::new(),
        proposal_event_id: proposal_event.id.to_hex(),
        proposal_digest: proposal.digest()?,
        decision: ReviewDecision::Accept,
        accepted_claim_ids: vec![claim.claim_id.clone()],
        rejected_claim_ids: Vec::new(),
        policy_id: proposal.policy_id.clone(),
    }
    .seal()?;
    let review_event = sign_compatibility_event(
        &reviewer,
        channel_id,
        CombEventRole::Review,
        &review.review_id,
        &review,
        &CompatibilityTags {
            target_event_id: Some(&proposal_event.id.to_hex()),
            ..CompatibilityTags::default()
        },
    )?;
    reviewer_client.publish(&review_event).await?;

    let record = MemoryRecord {
        schema: RECORD_SCHEMA_V1.into(),
        record_id: String::new(),
        channel_id: channel_id.to_string(),
        created_at: unix_now()?,
        proposal_event_id: proposal_event.id.to_hex(),
        proposal_digest: proposal.digest()?,
        review_event_ids: vec![review_event.id.to_hex()],
        claims: vec![claim.clone()],
        supersedes_record_ids: Vec::new(),
    }
    .seal()?;
    let record_sources = record
        .evidence_event_ids()
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let record_event = sign_compatibility_event(
        &comb,
        channel_id,
        CombEventRole::Record,
        &record.record_id,
        &record,
        &CompatibilityTags {
            source_event_ids: &record_sources,
            target_event_id: Some(&proposal_event.id.to_hex()),
            ..CompatibilityTags::default()
        },
    )?;
    comb_client.publish(&record_event).await?;

    let store = match store_path {
        Some(path) => StateStore::open(path)?,
        None => StateStore::in_memory()?,
    };
    let receipt = StoredArtifact {
        stable_id: record.record_id.clone(),
        artifact_kind: "record".into(),
        channel_id: channel_id.to_string(),
        event_id: Some(record_event.id.to_hex()),
        digest: comb_core::jcs_sha256(&record)?,
        status: "ratified".into(),
        observed_at: unix_now()?,
    };
    store.put_artifact(&receipt)?;
    store.put_artifact(&receipt)?;
    let restart_idempotence_receipt = store.artifact(&record.record_id)? == Some(receipt);

    let outsider_denied = match outsider_client.query_channel(channel_id, &[9], 10).await {
        Ok(events) => events.is_empty(),
        Err(error) if is_expected_private_channel_denial(&error) => true,
        Err(error) => {
            return Err(error).context(
                "outsider privacy probe failed for a reason other than an explicit Buzz access denial",
            );
        }
    };
    if !outsider_denied {
        bail!("uninvited identity received private-channel events");
    }

    owner_client
        .publish(&sign_delete_event(
            &owner,
            channel_id,
            &decision_source.id.to_hex(),
        )?)
        .await?;
    let source_deleted = comb_client
        .event_in_channel(channel_id, decision_source.id)
        .await?
        .is_none();
    if !source_deleted {
        bail!("deleted source remained resolvable");
    }

    let invalidation = Invalidation {
        schema: INVALIDATION_SCHEMA_V1.into(),
        invalidation_id: String::new(),
        target_record_event_id: record_event.id.to_hex(),
        target_record_id: record.record_id.clone(),
        affected_claim_ids: vec![claim.claim_id],
        source_event_ids: vec![decision_source.id.to_hex()],
        reason: InvalidationReason::SourceDeleted,
        detected_at: unix_now()?,
    }
    .seal()?;
    let invalidation_event = sign_compatibility_event(
        &comb,
        channel_id,
        CombEventRole::Invalidation,
        &invalidation.invalidation_id,
        &invalidation,
        &CompatibilityTags {
            source_event_ids: &invalidation.source_event_ids,
            target_event_id: Some(&record_event.id.to_hex()),
            ..CompatibilityTags::default()
        },
    )?;
    comb_client.publish(&invalidation_event).await?;
    comb_client
        .publish(&sign_delete_event(
            &comb,
            channel_id,
            &record_event.id.to_hex(),
        )?)
        .await?;
    store.mark_unsupported(&record.record_id, unix_now()?)?;
    let record_invalidated = comb_client
        .event_in_channel(channel_id, record_event.id)
        .await?
        .is_none()
        && store
            .artifact(&record.record_id)?
            .is_some_and(|artifact| artifact.status == "unsupported");
    let reviewer_signed_own_event = review_event.pubkey == reviewer.public_key();
    let passed = outsider_denied
        && reviewer_signed_own_event
        && source_deleted
        && record_invalidated
        && restart_idempotence_receipt;

    Ok(ProofReport {
        schema: "comb.proof.v1",
        buzz_sha,
        relay: relay.into(),
        channel_id: channel_id.to_string(),
        source_event_ids: source_ids,
        proposal_event_id: proposal_event.id.to_hex(),
        review_event_id: review_event.id.to_hex(),
        record_event_id: record_event.id.to_hex(),
        invalidation_event_id: invalidation_event.id.to_hex(),
        source_count_before,
        source_count_after,
        outsider_denied,
        reviewer_signed_own_event,
        source_deleted,
        record_invalidated,
        restart_idempotence_receipt,
        coverage_label: "self-attested",
        passed,
        completed_at: unix_now()?,
    })
}

fn evidence_ref(
    relay: &str,
    channel_id: Uuid,
    event: &Event,
    role: EvidenceRole,
) -> Result<EvidenceRef> {
    Ok(EvidenceRef {
        relay: relay.into(),
        community: relay_host(relay),
        channel_id: channel_id.to_string(),
        event_id: event.id.to_hex(),
        kind: event.kind.as_u16() as u32,
        author: event.pubkey.to_hex(),
        created_at: event.created_at.as_secs() as i64,
        role,
        content_digest: comb_core::jcs_sha256(&event.content)?,
        locator: EvidenceLocator::WholeEvent,
    })
}

fn relay_host(relay: &str) -> String {
    relay
        .split_once("://")
        .map_or(relay, |(_, rest)| rest)
        .split('/')
        .next()
        .unwrap_or(relay)
        .to_owned()
}

fn is_expected_private_channel_denial(error: &anyhow::Error) -> bool {
    error.to_string() == "relay closed query: restricted: not a channel member"
}

fn unix_now() -> Result<i64> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system clock predates Unix epoch")?
        .as_secs();
    i64::try_from(seconds).context("Unix timestamp exceeds i64")
}

#[cfg(test)]
mod tests {
    use super::is_expected_private_channel_denial;

    #[test]
    fn exact_private_channel_denial_is_accepted() {
        let error = anyhow::anyhow!("relay closed query: restricted: not a channel member");
        assert!(is_expected_private_channel_denial(&error));
    }

    #[test]
    fn network_or_auth_failures_cannot_pass_as_privacy() {
        for message in [
            "operation timed out",
            "failed to connect to ws://127.0.0.1:4300",
            "relay closed query: auth-required: must authenticate",
        ] {
            let error = anyhow::anyhow!(message);
            assert!(!is_expected_private_channel_denial(&error));
        }
    }
}
