//! Deterministic, I/O-free protocol contracts for Comb.
//!
//! This crate deliberately has no async runtime, network, database, or model
//! dependencies. All persistent identifiers are SHA-256 digests of RFC 8785
//! JSON Canonicalization Scheme bytes.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const PROPOSAL_SCHEMA_V1: &str = "comb.proposal.v1";
pub const SNAPSHOT_SCHEMA_V1: &str = "comb.snapshot.v1";
pub const REVIEW_SCHEMA_V1: &str = "comb.review.v1";
pub const RECORD_SCHEMA_V1: &str = "comb.record.v1";
pub const INVALIDATION_SCHEMA_V1: &str = "comb.invalidation.v1";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CoreError {
    #[error("canonical serialization failed: {0}")]
    Canonical(String),
    #[error("invalid {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
}

pub trait Validate {
    fn validate(&self) -> Result<(), CoreError>;
}

/// Serialize a value using RFC 8785 JCS and hash the canonical bytes.
pub fn jcs_sha256<T: Serialize>(value: &T) -> Result<String, CoreError> {
    let bytes =
        serde_jcs::to_vec(value).map_err(|error| CoreError::Canonical(error.to_string()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}

fn jcs_sha256_without<T: Serialize>(value: &T, key: &str) -> Result<String, CoreError> {
    let mut json =
        serde_json::to_value(value).map_err(|error| CoreError::Canonical(error.to_string()))?;
    let object = json.as_object_mut().ok_or_else(|| CoreError::Invalid {
        field: "digest",
        reason: "content-addressed payload must serialize as an object".to_string(),
    })?;
    object.insert(key.to_string(), Value::String(String::new()));
    jcs_sha256(&json)
}

fn invalid(field: &'static str, reason: impl Into<String>) -> CoreError {
    CoreError::Invalid {
        field,
        reason: reason.into(),
    }
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        return Err(invalid(field, "must not be empty"));
    }
    Ok(())
}

fn require_hex_32(field: &'static str, value: &str) -> Result<(), CoreError> {
    if value.len() != 64 || hex::decode(value).map_or(true, |bytes| bytes.len() != 32) {
        return Err(invalid(
            field,
            "must be 32 bytes encoded as 64 hexadecimal characters",
        ));
    }
    Ok(())
}

fn require_sorted_unique<T: Ord>(field: &'static str, values: &[T]) -> Result<(), CoreError> {
    if values.windows(2).any(|window| window[0] >= window[1]) {
        return Err(invalid(field, "must be strictly sorted and unique"));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceRole {
    Supports,
    Contradicts,
    Context,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EvidenceLocator {
    WholeEvent,
    Utf8ByteSpan { start: u64, end: u64 },
    MediaTimeRange { start_ms: u64, end_ms: u64 },
}

impl Validate for EvidenceLocator {
    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::WholeEvent => Ok(()),
            Self::Utf8ByteSpan { start, end } => {
                if start >= end {
                    return Err(invalid("locator", "UTF-8 byte span must be non-empty"));
                }
                Ok(())
            }
            Self::MediaTimeRange { start_ms, end_ms } => {
                if start_ms >= end_ms {
                    return Err(invalid("locator", "media time range must be non-empty"));
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub relay: String,
    pub community: String,
    pub channel_id: String,
    pub event_id: String,
    pub kind: u32,
    pub author: String,
    pub created_at: i64,
    pub role: EvidenceRole,
    pub content_digest: String,
    pub locator: EvidenceLocator,
}

impl Validate for EvidenceRef {
    fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("relay", &self.relay)?;
        require_non_empty("community", &self.community)?;
        require_non_empty("channelId", &self.channel_id)?;
        require_hex_32("eventId", &self.event_id)?;
        require_hex_32("author", &self.author)?;
        require_hex_32("contentDigest", &self.content_digest)?;
        self.locator.validate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimState {
    Asserted,
    Inferred,
    Unsupported,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Claim {
    pub claim_id: String,
    pub text: String,
    pub state: ClaimState,
    pub valid_from: i64,
    pub valid_until: Option<i64>,
    pub evidence: Vec<EvidenceRef>,
    pub counterevidence: Vec<EvidenceRef>,
    pub supersedes: Vec<String>,
}

impl Claim {
    pub fn normalize(&mut self) {
        self.evidence.sort();
        self.evidence.dedup();
        self.counterevidence.sort();
        self.counterevidence.dedup();
        self.supersedes.sort();
        self.supersedes.dedup();
    }

    pub fn computed_id(&self) -> Result<String, CoreError> {
        jcs_sha256_without(self, "claimId")
    }

    pub fn seal(mut self) -> Result<Self, CoreError> {
        self.normalize();
        self.claim_id = self.computed_id()?;
        self.validate()?;
        Ok(self)
    }

    pub fn all_evidence(&self) -> impl Iterator<Item = &EvidenceRef> {
        self.evidence.iter().chain(self.counterevidence.iter())
    }
}

impl Validate for Claim {
    fn validate(&self) -> Result<(), CoreError> {
        require_hex_32("claimId", &self.claim_id)?;
        require_non_empty("claim.text", &self.text)?;
        if self
            .valid_until
            .is_some_and(|until| until <= self.valid_from)
        {
            return Err(invalid("claim.validUntil", "must be later than validFrom"));
        }
        require_sorted_unique("claim.evidence", &self.evidence)?;
        require_sorted_unique("claim.counterevidence", &self.counterevidence)?;
        require_sorted_unique("claim.supersedes", &self.supersedes)?;
        for evidence in self.all_evidence() {
            evidence.validate()?;
        }
        if self.state == ClaimState::Asserted && self.evidence.is_empty() {
            return Err(invalid(
                "claim.evidence",
                "asserted claims require primary supporting evidence",
            ));
        }
        if self
            .supersedes
            .iter()
            .any(|claim_id| claim_id == &self.claim_id)
        {
            return Err(invalid(
                "claim.supersedes",
                "a claim cannot supersede itself",
            ));
        }
        if self.computed_id()? != self.claim_id {
            return Err(invalid(
                "claimId",
                "does not match canonical payload digest",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageStatus {
    Complete,
    Partial,
    Stale,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageGapReason {
    QueryCapOverflow,
    SameSecondOverflow,
    CountChangedDuringCapture,
    SubscriptionDisconnect,
    SourceDeleted,
    SourceUnavailable,
    SignatureInvalid,
    MissingTranscript,
    PrivateDmNotGranted,
    AdapterError,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageGap {
    pub reason: CoverageGapReason,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageLane {
    pub channel_id: String,
    pub kinds: Vec<u32>,
    pub count_before: u64,
    pub retrieved: u64,
    pub count_after: u64,
    pub signature_failures: u64,
    pub unresolved_sources: u64,
}

impl CoverageLane {
    pub fn normalize(&mut self) {
        self.kinds.sort_unstable();
        self.kinds.dedup();
    }

    pub fn is_exact(&self) -> bool {
        self.count_before == self.count_after
            && self.count_before == self.retrieved
            && self.signature_failures == 0
            && self.unresolved_sources == 0
    }
}

impl Validate for CoverageLane {
    fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("coverage.lane.channelId", &self.channel_id)?;
        if self.kinds.is_empty() {
            return Err(invalid("coverage.lane.kinds", "must not be empty"));
        }
        require_sorted_unique("coverage.lane.kinds", &self.kinds)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TemporalWindow {
    /// Inclusive start, in Unix seconds.
    pub since: i64,
    /// Exclusive end, in Unix seconds.
    pub until: i64,
}

impl Validate for TemporalWindow {
    fn validate(&self) -> Result<(), CoreError> {
        if self.since >= self.until {
            return Err(invalid("window", "must be a non-empty half-open interval"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverageRecord {
    pub status: CoverageStatus,
    pub requested_window: TemporalWindow,
    pub capture_started_at: i64,
    pub capture_completed_at: i64,
    pub lanes: Vec<CoverageLane>,
    pub gaps: Vec<CoverageGap>,
}

impl CoverageRecord {
    pub fn normalize(&mut self) {
        for lane in &mut self.lanes {
            lane.normalize();
        }
        self.lanes.sort();
        self.lanes.dedup();
        self.gaps.sort();
        self.gaps.dedup();
    }

    pub fn satisfies_complete_contract(&self) -> bool {
        self.requested_window.since < self.requested_window.until
            && self.requested_window.until <= self.capture_started_at
            && self.capture_started_at <= self.capture_completed_at
            && !self.lanes.is_empty()
            && self.gaps.is_empty()
            && self.lanes.iter().all(CoverageLane::is_exact)
    }
}

impl Validate for CoverageRecord {
    fn validate(&self) -> Result<(), CoreError> {
        self.requested_window.validate()?;
        if self.capture_completed_at < self.capture_started_at {
            return Err(invalid(
                "coverage.captureCompletedAt",
                "must not precede captureStartedAt",
            ));
        }
        require_sorted_unique("coverage.lanes", &self.lanes)?;
        require_sorted_unique("coverage.gaps", &self.gaps)?;
        for lane in &self.lanes {
            lane.validate()?;
        }
        if self.status == CoverageStatus::Complete && !self.satisfies_complete_contract() {
            return Err(invalid(
                "coverage.status",
                "complete coverage requires a bounded settled window, stable exact counts, valid signatures, resolved sources, and no gaps",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetrievalSnapshot {
    pub schema: String,
    pub snapshot_id: String,
    pub community: String,
    pub channel_id: String,
    pub source_event_ids: Vec<String>,
    pub coverage: CoverageRecord,
}

impl RetrievalSnapshot {
    pub fn normalize(&mut self) {
        self.source_event_ids.sort();
        self.source_event_ids.dedup();
        self.coverage.normalize();
    }

    pub fn computed_id(&self) -> Result<String, CoreError> {
        jcs_sha256_without(self, "snapshotId")
    }

    pub fn seal(mut self) -> Result<Self, CoreError> {
        self.normalize();
        self.snapshot_id = self.computed_id()?;
        self.validate()?;
        Ok(self)
    }
}

impl Validate for RetrievalSnapshot {
    fn validate(&self) -> Result<(), CoreError> {
        if self.schema != SNAPSHOT_SCHEMA_V1 {
            return Err(invalid("snapshot.schema", "unsupported schema"));
        }
        require_hex_32("snapshotId", &self.snapshot_id)?;
        require_non_empty("snapshot.community", &self.community)?;
        require_non_empty("snapshot.channelId", &self.channel_id)?;
        require_sorted_unique("snapshot.sourceEventIds", &self.source_event_ids)?;
        for event_id in &self.source_event_ids {
            require_hex_32("snapshot.sourceEventId", event_id)?;
        }
        self.coverage.validate()?;
        if self
            .coverage
            .lanes
            .iter()
            .any(|lane| lane.channel_id != self.channel_id)
        {
            return Err(invalid(
                "snapshot.coverage",
                "all coverage lanes must match the snapshot channel",
            ));
        }
        let mut covered_kinds = BTreeSet::new();
        for lane in &self.coverage.lanes {
            for kind in &lane.kinds {
                if !covered_kinds.insert(*kind) {
                    return Err(invalid(
                        "snapshot.coverage",
                        "coverage lane kind sets must not overlap",
                    ));
                }
            }
        }
        if self.coverage.status == CoverageStatus::Complete {
            let retrieved = self
                .coverage
                .lanes
                .iter()
                .try_fold(0_u64, |total, lane| total.checked_add(lane.retrieved))
                .ok_or_else(|| invalid("snapshot.coverage", "retrieved count overflow"))?;
            if retrieved != self.source_event_ids.len() as u64 {
                return Err(invalid(
                    "snapshot.sourceEventIds",
                    "complete coverage requires the source manifest size to match retrieved inventory",
                ));
            }
        }
        if self.computed_id()? != self.snapshot_id {
            return Err(invalid(
                "snapshotId",
                "does not match canonical payload digest",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratorInfo {
    pub name: String,
    pub version: String,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
}

impl Validate for GeneratorInfo {
    fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("generator.name", &self.name)?;
        require_non_empty("generator.version", &self.version)?;
        if self
            .model
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(invalid("generator.model", "must not be blank when present"));
        }
        if self
            .prompt_version
            .as_ref()
            .is_some_and(|value| value.trim().is_empty())
        {
            return Err(invalid(
                "generator.promptVersion",
                "must not be blank when present",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Proposal {
    pub schema: String,
    pub proposal_id: String,
    pub channel_id: String,
    pub created_at: i64,
    pub generator: GeneratorInfo,
    pub policy_id: String,
    pub snapshot: RetrievalSnapshot,
    pub claims: Vec<Claim>,
}

impl Proposal {
    pub fn normalize(&mut self) {
        self.snapshot.normalize();
        for claim in &mut self.claims {
            claim.normalize();
        }
        self.claims
            .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        self.claims
            .dedup_by(|left, right| left.claim_id == right.claim_id);
    }

    pub fn computed_id(&self) -> Result<String, CoreError> {
        jcs_sha256_without(self, "proposalId")
    }

    pub fn digest(&self) -> Result<String, CoreError> {
        jcs_sha256(self)
    }

    pub fn seal(mut self) -> Result<Self, CoreError> {
        self.normalize();
        self.proposal_id = self.computed_id()?;
        self.validate()?;
        Ok(self)
    }

    pub fn evidence_event_ids(&self) -> BTreeSet<&str> {
        self.claims
            .iter()
            .flat_map(Claim::all_evidence)
            .map(|evidence| evidence.event_id.as_str())
            .collect()
    }
}

impl Validate for Proposal {
    fn validate(&self) -> Result<(), CoreError> {
        if self.schema != PROPOSAL_SCHEMA_V1 {
            return Err(invalid("proposal.schema", "unsupported schema"));
        }
        require_hex_32("proposalId", &self.proposal_id)?;
        require_non_empty("proposal.channelId", &self.channel_id)?;
        require_non_empty("proposal.policyId", &self.policy_id)?;
        self.generator.validate()?;
        self.snapshot.validate()?;
        if self.snapshot.channel_id != self.channel_id {
            return Err(invalid(
                "proposal.snapshot",
                "snapshot and proposal channels must match",
            ));
        }
        if self.claims.is_empty() {
            return Err(invalid("proposal.claims", "must not be empty"));
        }
        if self
            .claims
            .windows(2)
            .any(|window| window[0].claim_id >= window[1].claim_id)
        {
            return Err(invalid(
                "proposal.claims",
                "must be sorted by claimId and unique",
            ));
        }
        let snapshot_ids: BTreeSet<&str> = self
            .snapshot
            .source_event_ids
            .iter()
            .map(String::as_str)
            .collect();
        for claim in &self.claims {
            claim.validate()?;
            for evidence in claim.all_evidence() {
                if evidence.channel_id != self.channel_id {
                    return Err(invalid(
                        "proposal.claims.evidence",
                        "cross-channel evidence is not allowed in v1",
                    ));
                }
                if evidence.community != self.snapshot.community {
                    return Err(invalid(
                        "proposal.claims.evidence",
                        "evidence community must match the retrieval snapshot",
                    ));
                }
                if !snapshot_ids.contains(evidence.event_id.as_str()) {
                    return Err(invalid(
                        "proposal.claims.evidence",
                        "every evidence event must be present in the retrieval snapshot",
                    ));
                }
            }
        }
        if self.computed_id()? != self.proposal_id {
            return Err(invalid(
                "proposalId",
                "does not match canonical payload digest",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Accept,
    Reject,
    Revise,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub schema: String,
    pub review_id: String,
    pub proposal_event_id: String,
    pub proposal_digest: String,
    pub decision: ReviewDecision,
    pub accepted_claim_ids: Vec<String>,
    pub rejected_claim_ids: Vec<String>,
    pub policy_id: String,
}

impl Review {
    pub fn normalize(&mut self) {
        self.accepted_claim_ids.sort();
        self.accepted_claim_ids.dedup();
        self.rejected_claim_ids.sort();
        self.rejected_claim_ids.dedup();
    }

    pub fn computed_id(&self) -> Result<String, CoreError> {
        jcs_sha256_without(self, "reviewId")
    }

    pub fn seal(mut self) -> Result<Self, CoreError> {
        self.normalize();
        self.review_id = self.computed_id()?;
        self.validate()?;
        Ok(self)
    }
}

impl Validate for Review {
    fn validate(&self) -> Result<(), CoreError> {
        if self.schema != REVIEW_SCHEMA_V1 {
            return Err(invalid("review.schema", "unsupported schema"));
        }
        require_hex_32("reviewId", &self.review_id)?;
        require_hex_32("review.proposalEventId", &self.proposal_event_id)?;
        require_hex_32("review.proposalDigest", &self.proposal_digest)?;
        require_non_empty("review.policyId", &self.policy_id)?;
        require_sorted_unique("review.acceptedClaimIds", &self.accepted_claim_ids)?;
        require_sorted_unique("review.rejectedClaimIds", &self.rejected_claim_ids)?;
        for claim_id in self
            .accepted_claim_ids
            .iter()
            .chain(self.rejected_claim_ids.iter())
        {
            require_hex_32("review.claimId", claim_id)?;
        }
        if self
            .accepted_claim_ids
            .iter()
            .any(|claim_id| self.rejected_claim_ids.binary_search(claim_id).is_ok())
        {
            return Err(invalid(
                "review.claimIds",
                "a claim cannot be both accepted and rejected",
            ));
        }
        if self.decision == ReviewDecision::Accept && self.accepted_claim_ids.is_empty() {
            return Err(invalid(
                "review.acceptedClaimIds",
                "accept reviews require at least one accepted claim",
            ));
        }
        if self.decision == ReviewDecision::Reject && self.rejected_claim_ids.is_empty() {
            return Err(invalid(
                "review.rejectedClaimIds",
                "reject reviews require at least one rejected claim",
            ));
        }
        if self.computed_id()? != self.review_id {
            return Err(invalid(
                "reviewId",
                "does not match canonical payload digest",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryRecord {
    pub schema: String,
    pub record_id: String,
    pub channel_id: String,
    pub created_at: i64,
    pub proposal_event_id: String,
    pub proposal_digest: String,
    pub review_event_ids: Vec<String>,
    pub claims: Vec<Claim>,
    pub supersedes_record_ids: Vec<String>,
}

impl MemoryRecord {
    pub fn normalize(&mut self) {
        self.review_event_ids.sort();
        self.review_event_ids.dedup();
        for claim in &mut self.claims {
            claim.normalize();
        }
        self.claims
            .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
        self.claims
            .dedup_by(|left, right| left.claim_id == right.claim_id);
        self.supersedes_record_ids.sort();
        self.supersedes_record_ids.dedup();
    }

    pub fn computed_id(&self) -> Result<String, CoreError> {
        jcs_sha256_without(self, "recordId")
    }

    pub fn seal(mut self) -> Result<Self, CoreError> {
        self.normalize();
        self.record_id = self.computed_id()?;
        self.validate()?;
        Ok(self)
    }

    pub fn evidence_event_ids(&self) -> BTreeSet<&str> {
        self.claims
            .iter()
            .flat_map(Claim::all_evidence)
            .map(|evidence| evidence.event_id.as_str())
            .collect()
    }
}

impl Validate for MemoryRecord {
    fn validate(&self) -> Result<(), CoreError> {
        if self.schema != RECORD_SCHEMA_V1 {
            return Err(invalid("record.schema", "unsupported schema"));
        }
        require_hex_32("recordId", &self.record_id)?;
        require_non_empty("record.channelId", &self.channel_id)?;
        require_hex_32("record.proposalEventId", &self.proposal_event_id)?;
        require_hex_32("record.proposalDigest", &self.proposal_digest)?;
        if self.review_event_ids.is_empty() {
            return Err(invalid("record.reviewEventIds", "must not be empty"));
        }
        require_sorted_unique("record.reviewEventIds", &self.review_event_ids)?;
        require_sorted_unique("record.supersedesRecordIds", &self.supersedes_record_ids)?;
        for event_id in &self.review_event_ids {
            require_hex_32("record.reviewEventId", event_id)?;
        }
        for record_id in &self.supersedes_record_ids {
            require_hex_32("record.supersedesRecordId", record_id)?;
        }
        if self.claims.is_empty() {
            return Err(invalid("record.claims", "must not be empty"));
        }
        if self
            .claims
            .windows(2)
            .any(|window| window[0].claim_id >= window[1].claim_id)
        {
            return Err(invalid(
                "record.claims",
                "must be sorted by claimId and unique",
            ));
        }
        for claim in &self.claims {
            claim.validate()?;
            if claim.state != ClaimState::Asserted {
                return Err(invalid(
                    "record.claims",
                    "ratified records contain asserted claims only",
                ));
            }
            if claim
                .all_evidence()
                .any(|evidence| evidence.channel_id != self.channel_id)
            {
                return Err(invalid(
                    "record.claims.evidence",
                    "cross-channel evidence is not allowed in v1",
                ));
            }
        }
        if self.computed_id()? != self.record_id {
            return Err(invalid(
                "recordId",
                "does not match canonical payload digest",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvalidationReason {
    SourceDeleted,
    SourceMissing,
    DigestMismatch,
    AuthorityRevoked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Invalidation {
    pub schema: String,
    pub invalidation_id: String,
    pub target_record_event_id: String,
    pub target_record_id: String,
    pub affected_claim_ids: Vec<String>,
    pub source_event_ids: Vec<String>,
    pub reason: InvalidationReason,
    pub detected_at: i64,
}

impl Invalidation {
    pub fn normalize(&mut self) {
        self.affected_claim_ids.sort();
        self.affected_claim_ids.dedup();
        self.source_event_ids.sort();
        self.source_event_ids.dedup();
    }

    pub fn computed_id(&self) -> Result<String, CoreError> {
        jcs_sha256_without(self, "invalidationId")
    }

    pub fn seal(mut self) -> Result<Self, CoreError> {
        self.normalize();
        self.invalidation_id = self.computed_id()?;
        self.validate()?;
        Ok(self)
    }
}

impl Validate for Invalidation {
    fn validate(&self) -> Result<(), CoreError> {
        if self.schema != INVALIDATION_SCHEMA_V1 {
            return Err(invalid("invalidation.schema", "unsupported schema"));
        }
        require_hex_32("invalidationId", &self.invalidation_id)?;
        require_hex_32(
            "invalidation.targetRecordEventId",
            &self.target_record_event_id,
        )?;
        require_hex_32("invalidation.targetRecordId", &self.target_record_id)?;
        if self.affected_claim_ids.is_empty() || self.source_event_ids.is_empty() {
            return Err(invalid(
                "invalidation",
                "affected claims and source events must not be empty",
            ));
        }
        require_sorted_unique("invalidation.affectedClaimIds", &self.affected_claim_ids)?;
        require_sorted_unique("invalidation.sourceEventIds", &self.source_event_ids)?;
        for claim_id in &self.affected_claim_ids {
            require_hex_32("invalidation.claimId", claim_id)?;
        }
        for event_id in &self.source_event_ids {
            require_hex_32("invalidation.sourceEventId", event_id)?;
        }
        if self.computed_id()? != self.invalidation_id {
            return Err(invalid(
                "invalidationId",
                "does not match canonical payload digest",
            ));
        }
        Ok(())
    }
}

/// Return true when the directed record-supersession graph contains a cycle.
pub fn has_supersession_cycle(graph: &BTreeMap<String, Vec<String>>) -> bool {
    fn visit(
        node: &str,
        graph: &BTreeMap<String, Vec<String>>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visiting.contains(node) {
            return true;
        }
        if visited.contains(node) {
            return false;
        }
        visiting.insert(node.to_string());
        if graph
            .get(node)
            .into_iter()
            .flatten()
            .any(|next| visit(next, graph, visiting, visited))
        {
            return true;
        }
        visiting.remove(node);
        visited.insert(node.to_string());
        false
    }

    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    graph
        .keys()
        .any(|node| visit(node, graph, &mut visiting, &mut visited))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex_id(byte: u8) -> String {
        hex::encode([byte; 32])
    }

    fn evidence(event: u8, channel: &str) -> EvidenceRef {
        EvidenceRef {
            relay: "wss://buzz.example".into(),
            community: "buzz.example".into(),
            channel_id: channel.into(),
            event_id: hex_id(event),
            kind: 9,
            author: hex_id(0xa0),
            created_at: 100,
            role: EvidenceRole::Supports,
            content_digest: hex_id(event.wrapping_add(1)),
            locator: EvidenceLocator::WholeEvent,
        }
    }

    fn complete_coverage(channel: &str) -> CoverageRecord {
        let mut coverage = CoverageRecord {
            status: CoverageStatus::Complete,
            requested_window: TemporalWindow {
                since: 10,
                until: 20,
            },
            capture_started_at: 21,
            capture_completed_at: 22,
            lanes: vec![CoverageLane {
                channel_id: channel.into(),
                kinds: vec![9],
                count_before: 1,
                retrieved: 1,
                count_after: 1,
                signature_failures: 0,
                unresolved_sources: 0,
            }],
            gaps: vec![],
        };
        coverage.normalize();
        coverage
    }

    #[test]
    fn jcs_digest_ignores_json_object_insertion_order() {
        let left: Value = serde_json::from_str(r#"{"b":2,"a":1}"#).expect("json");
        let right: Value = serde_json::from_str(r#"{"a":1,"b":2}"#).expect("json");
        assert_eq!(jcs_sha256(&left).unwrap(), jcs_sha256(&right).unwrap());
    }

    #[test]
    fn sealing_a_claim_normalizes_evidence_and_is_deterministic() {
        let claim = Claim {
            claim_id: String::new(),
            text: "Launch waits for rollback proof.".into(),
            state: ClaimState::Asserted,
            valid_from: 100,
            valid_until: None,
            evidence: vec![evidence(2, "channel-a"), evidence(1, "channel-a")],
            counterevidence: vec![],
            supersedes: vec![],
        }
        .seal()
        .unwrap();
        assert_eq!(claim.evidence[0].event_id, hex_id(1));
        claim.validate().unwrap();
        assert_eq!(claim.claim_id, claim.computed_id().unwrap());
    }

    #[test]
    fn canonical_id_detects_payload_tampering() {
        let mut claim = Claim {
            claim_id: String::new(),
            text: "Launch waits for rollback proof.".into(),
            state: ClaimState::Asserted,
            valid_from: 100,
            valid_until: None,
            evidence: vec![evidence(1, "channel-a")],
            counterevidence: vec![],
            supersedes: vec![],
        }
        .seal()
        .unwrap();
        claim.text = "Launch immediately.".into();
        assert!(claim
            .validate()
            .unwrap_err()
            .to_string()
            .contains("claimId"));
    }

    #[test]
    fn asserted_claim_without_evidence_is_rejected() {
        let error = Claim {
            claim_id: hex_id(1),
            text: "Unsupported assertion".into(),
            state: ClaimState::Asserted,
            valid_from: 0,
            valid_until: None,
            evidence: vec![],
            counterevidence: vec![],
            supersedes: vec![],
        }
        .validate()
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("require primary supporting evidence"));
    }

    #[test]
    fn complete_coverage_requires_stable_exact_inventory() {
        let mut coverage = complete_coverage("channel-a");
        coverage.lanes[0].count_after = 2;
        assert!(!coverage.satisfies_complete_contract());
        assert!(coverage.validate().is_err());

        coverage.status = CoverageStatus::Partial;
        coverage.validate().unwrap();
    }

    #[test]
    fn complete_coverage_rejects_live_unsettled_window() {
        let mut coverage = complete_coverage("channel-a");
        coverage.requested_window.until = coverage.capture_started_at + 1;
        assert!(coverage.validate().is_err());
    }

    #[test]
    fn complete_snapshot_manifest_must_equal_retrieved_inventory() {
        let error = RetrievalSnapshot {
            schema: SNAPSHOT_SCHEMA_V1.into(),
            snapshot_id: String::new(),
            community: "buzz.example".into(),
            channel_id: "channel-a".into(),
            source_event_ids: vec![hex_id(1), hex_id(2)],
            coverage: complete_coverage("channel-a"),
        }
        .seal()
        .unwrap_err();
        assert!(error.to_string().contains("manifest size"));
    }

    #[test]
    fn snapshot_rejects_overlapping_coverage_lanes() {
        let mut coverage = complete_coverage("channel-a");
        coverage.status = CoverageStatus::Partial;
        coverage.lanes.push(coverage.lanes[0].clone());
        coverage.lanes[1].count_before = 0;
        coverage.lanes[1].retrieved = 0;
        coverage.lanes[1].count_after = 0;
        coverage.normalize();
        let error = RetrievalSnapshot {
            schema: SNAPSHOT_SCHEMA_V1.into(),
            snapshot_id: String::new(),
            community: "buzz.example".into(),
            channel_id: "channel-a".into(),
            source_event_ids: vec![hex_id(1)],
            coverage,
        }
        .seal()
        .unwrap_err();
        assert!(error.to_string().contains("must not overlap"));
    }

    #[test]
    fn proposal_rejects_cross_channel_or_invented_evidence() {
        let channel = "channel-a";
        let claim = Claim {
            claim_id: String::new(),
            text: "A decision".into(),
            state: ClaimState::Asserted,
            valid_from: 100,
            valid_until: None,
            evidence: vec![evidence(1, "channel-b")],
            counterevidence: vec![],
            supersedes: vec![],
        }
        .seal()
        .unwrap();
        let snapshot = RetrievalSnapshot {
            schema: SNAPSHOT_SCHEMA_V1.into(),
            snapshot_id: String::new(),
            community: "buzz.example".into(),
            channel_id: channel.into(),
            source_event_ids: vec![hex_id(1)],
            coverage: complete_coverage(channel),
        }
        .seal()
        .unwrap();
        let error = Proposal {
            schema: PROPOSAL_SCHEMA_V1.into(),
            proposal_id: String::new(),
            channel_id: channel.into(),
            created_at: 200,
            generator: GeneratorInfo {
                name: "fixture".into(),
                version: "1".into(),
                model: None,
                prompt_version: None,
            },
            policy_id: "default".into(),
            snapshot,
            claims: vec![claim],
        }
        .seal()
        .unwrap_err();
        assert!(error.to_string().contains("cross-channel evidence"));
    }

    #[test]
    fn proposal_requires_all_evidence_in_snapshot() {
        let channel = "channel-a";
        let claim = Claim {
            claim_id: String::new(),
            text: "A decision".into(),
            state: ClaimState::Asserted,
            valid_from: 100,
            valid_until: None,
            evidence: vec![evidence(2, channel)],
            counterevidence: vec![],
            supersedes: vec![],
        }
        .seal()
        .unwrap();
        let snapshot = RetrievalSnapshot {
            schema: SNAPSHOT_SCHEMA_V1.into(),
            snapshot_id: String::new(),
            community: "buzz.example".into(),
            channel_id: channel.into(),
            source_event_ids: vec![hex_id(1)],
            coverage: complete_coverage(channel),
        }
        .seal()
        .unwrap();
        let error = Proposal {
            schema: PROPOSAL_SCHEMA_V1.into(),
            proposal_id: String::new(),
            channel_id: channel.into(),
            created_at: 200,
            generator: GeneratorInfo {
                name: "fixture".into(),
                version: "1".into(),
                model: None,
                prompt_version: None,
            },
            policy_id: "default".into(),
            snapshot,
            claims: vec![claim],
        }
        .seal()
        .unwrap_err();
        assert!(error.to_string().contains("retrieval snapshot"));
    }

    #[test]
    fn supersession_cycle_detector_rejects_self_and_multi_node_cycles() {
        let mut graph = BTreeMap::new();
        graph.insert("a".into(), vec!["a".into()]);
        assert!(has_supersession_cycle(&graph));

        graph.clear();
        graph.insert("a".into(), vec!["b".into()]);
        graph.insert("b".into(), vec!["c".into()]);
        graph.insert("c".into(), vec!["a".into()]);
        assert!(has_supersession_cycle(&graph));

        graph.get_mut("c").unwrap().clear();
        assert!(!has_supersession_cycle(&graph));
    }
}
