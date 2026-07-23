//! Deterministic projection and ratification engine for Comb.
//!
//! The engine folds already verified Buzz event envelopes. It does not perform
//! I/O or trust arrival order to resolve conflicting authority decisions.

use std::collections::{BTreeMap, BTreeSet};

use comb_core::{
    has_supersession_cycle, jcs_sha256, ClaimState, CoreError, Invalidation, InvalidationReason,
    MemoryRecord, Proposal, Review, ReviewDecision, Validate,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EngineError {
    #[error(transparent)]
    Core(#[from] CoreError),
    #[error("invalid event envelope: {0}")]
    InvalidEnvelope(String),
    #[error("event id collision for {0}")]
    EventIdCollision(String),
    #[error("unknown ratification policy {0}")]
    UnknownPolicy(String),
    #[error("proposal not found: {0}")]
    ProposalNotFound(String),
    #[error("record not found: {0}")]
    RecordNotFound(String),
    #[error("review not found: {0}")]
    ReviewNotFound(String),
    #[error("proposal is not ratified: {0:?}")]
    ProposalNotRatified(RatificationStatus),
    #[error("payload reference mismatch: {0}")]
    ReferenceMismatch(String),
    #[error("source evidence is unavailable: {0}")]
    SourceUnavailable(String),
    #[error("record supersession cycle detected")]
    SupersessionCycle,
    #[error("record supersedes an unknown record: {0}")]
    UnknownSupersededRecord(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope<T> {
    pub event_id: String,
    pub author: String,
    pub created_at: i64,
    pub payload: T,
}

impl<T: Serialize> EventEnvelope<T> {
    fn validate_envelope(&self) -> Result<(), EngineError> {
        require_hex_32("eventId", &self.event_id)?;
        require_hex_32("author", &self.author)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "event", rename_all = "snake_case")]
#[allow(
    clippy::large_enum_variant,
    reason = "fold inputs are processed one at a time; boxing every public payload would add API friction"
)]
pub enum EngineEvent {
    Proposal(EventEnvelope<Proposal>),
    Review(EventEnvelope<Review>),
    Record(EventEnvelope<MemoryRecord>),
    Invalidation(EventEnvelope<Invalidation>),
    SourceDeleted(SourceDeletion),
}

impl EngineEvent {
    fn event_id(&self) -> &str {
        match self {
            Self::Proposal(event) => &event.event_id,
            Self::Review(event) => &event.event_id,
            Self::Record(event) => &event.event_id,
            Self::Invalidation(event) => &event.event_id,
            Self::SourceDeleted(event) => &event.deletion_event_id,
        }
    }

    fn digest(&self) -> Result<String, EngineError> {
        Ok(jcs_sha256(self)?)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceDeletion {
    pub deletion_event_id: String,
    pub source_event_id: String,
    pub author: String,
    pub observed_at: i64,
}

impl SourceDeletion {
    fn validate(&self) -> Result<(), EngineError> {
        require_hex_32("deletionEventId", &self.deletion_event_id)?;
        require_hex_32("sourceEventId", &self.source_event_id)?;
        require_hex_32("author", &self.author)
    }
}

fn require_hex_32(field: &str, value: &str) -> Result<(), EngineError> {
    if value.len() != 64 || hex::decode(value).map_or(true, |bytes| bytes.len() != 32) {
        return Err(EngineError::InvalidEnvelope(format!(
            "{field} must be 32 bytes encoded as 64 hexadecimal characters"
        )));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RatificationPolicy {
    pub policy_id: String,
    pub ratifiers: BTreeSet<String>,
    pub approval_quorum: usize,
    pub rejection_threshold: usize,
}

impl RatificationPolicy {
    pub fn validate(&self) -> Result<(), EngineError> {
        if self.policy_id.trim().is_empty() {
            return Err(EngineError::InvalidEnvelope(
                "policyId must not be empty".to_string(),
            ));
        }
        if self.ratifiers.is_empty() {
            return Err(EngineError::InvalidEnvelope(
                "ratifiers must not be empty".to_string(),
            ));
        }
        for ratifier in &self.ratifiers {
            require_hex_32("ratifier", ratifier)?;
        }
        if self.approval_quorum == 0 || self.approval_quorum > self.ratifiers.len() {
            return Err(EngineError::InvalidEnvelope(
                "approvalQuorum must be between one and the ratifier count".to_string(),
            ));
        }
        if self.rejection_threshold == 0 || self.rejection_threshold > self.ratifiers.len() {
            return Err(EngineError::InvalidEnvelope(
                "rejectionThreshold must be between one and the ratifier count".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RatificationStatus {
    Pending,
    Ratified,
    Rejected,
    RevisionRequested,
    Contested,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatificationEvaluation {
    pub status: RatificationStatus,
    pub accepted_review_event_ids: Vec<String>,
    pub rejected_review_event_ids: Vec<String>,
    pub revision_review_event_ids: Vec<String>,
    pub unauthorized_review_event_ids: Vec<String>,
    pub conflicting_reviewers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingInvalidation {
    pub target_record_event_id: String,
    pub target_record_id: String,
    pub affected_claim_ids: Vec<String>,
    pub source_event_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyOutcome {
    Applied,
    Duplicate,
    SourceDeleted {
        pending_invalidations: Vec<PendingInvalidation>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecordAvailability {
    Available,
    ExplicitlyInvalidated,
    SourceUnavailable(Vec<String>),
    Superseded,
}

#[derive(Debug, Default)]
pub struct Projection {
    policies: BTreeMap<String, RatificationPolicy>,
    proposals: BTreeMap<String, EventEnvelope<Proposal>>,
    reviews: BTreeMap<String, EventEnvelope<Review>>,
    records: BTreeMap<String, EventEnvelope<MemoryRecord>>,
    invalidations: BTreeMap<String, EventEnvelope<Invalidation>>,
    deleted_sources: BTreeSet<String>,
    event_digests: BTreeMap<String, String>,
}

impl Projection {
    pub fn new(
        policies: impl IntoIterator<Item = RatificationPolicy>,
    ) -> Result<Self, EngineError> {
        let mut projection = Self::default();
        for policy in policies {
            projection.add_policy(policy)?;
        }
        Ok(projection)
    }

    pub fn add_policy(&mut self, policy: RatificationPolicy) -> Result<(), EngineError> {
        policy.validate()?;
        if let Some(existing) = self.policies.get(&policy.policy_id) {
            if existing != &policy {
                return Err(EngineError::InvalidEnvelope(format!(
                    "policy {} is immutable",
                    policy.policy_id
                )));
            }
            return Ok(());
        }
        self.policies.insert(policy.policy_id.clone(), policy);
        Ok(())
    }

    pub fn proposal(&self, event_id: &str) -> Option<&EventEnvelope<Proposal>> {
        self.proposals.get(event_id)
    }

    pub fn review(&self, event_id: &str) -> Option<&EventEnvelope<Review>> {
        self.reviews.get(event_id)
    }

    pub fn record(&self, event_id: &str) -> Option<&EventEnvelope<MemoryRecord>> {
        self.records.get(event_id)
    }

    pub fn apply(&mut self, event: EngineEvent) -> Result<ApplyOutcome, EngineError> {
        let event_id = event.event_id().to_string();
        let digest = event.digest()?;
        if let Some(existing) = self.event_digests.get(&event_id) {
            return if existing == &digest {
                Ok(ApplyOutcome::Duplicate)
            } else {
                Err(EngineError::EventIdCollision(event_id))
            };
        }

        let outcome = match event {
            EngineEvent::Proposal(event) => {
                self.apply_proposal(event)?;
                ApplyOutcome::Applied
            }
            EngineEvent::Review(event) => {
                self.apply_review(event)?;
                ApplyOutcome::Applied
            }
            EngineEvent::Record(event) => {
                self.apply_record(event)?;
                ApplyOutcome::Applied
            }
            EngineEvent::Invalidation(event) => {
                self.apply_invalidation(event)?;
                ApplyOutcome::Applied
            }
            EngineEvent::SourceDeleted(deletion) => {
                deletion.validate()?;
                self.deleted_sources.insert(deletion.source_event_id);
                ApplyOutcome::SourceDeleted {
                    pending_invalidations: self.pending_invalidations(),
                }
            }
        };
        self.event_digests.insert(event_id, digest);
        Ok(outcome)
    }

    fn apply_proposal(&mut self, event: EventEnvelope<Proposal>) -> Result<(), EngineError> {
        event.validate_envelope()?;
        event.payload.validate()?;
        if !self.policies.contains_key(&event.payload.policy_id) {
            return Err(EngineError::UnknownPolicy(event.payload.policy_id.clone()));
        }
        for source in event.payload.evidence_event_ids() {
            if self.deleted_sources.contains(source) {
                return Err(EngineError::SourceUnavailable(source.to_string()));
            }
        }
        self.proposals.insert(event.event_id.clone(), event);
        Ok(())
    }

    fn apply_review(&mut self, event: EventEnvelope<Review>) -> Result<(), EngineError> {
        event.validate_envelope()?;
        event.payload.validate()?;
        let proposal = self
            .proposals
            .get(&event.payload.proposal_event_id)
            .ok_or_else(|| {
                EngineError::ProposalNotFound(event.payload.proposal_event_id.clone())
            })?;
        if proposal.payload.digest()? != event.payload.proposal_digest {
            return Err(EngineError::ReferenceMismatch(
                "review proposal digest does not match the referenced proposal".to_string(),
            ));
        }
        if proposal.payload.policy_id != event.payload.policy_id {
            return Err(EngineError::ReferenceMismatch(
                "review policy does not match the proposal policy".to_string(),
            ));
        }
        let asserted: BTreeSet<&str> = proposal
            .payload
            .claims
            .iter()
            .filter(|claim| claim.state == ClaimState::Asserted)
            .map(|claim| claim.claim_id.as_str())
            .collect();
        let reviewed: BTreeSet<&str> = event
            .payload
            .accepted_claim_ids
            .iter()
            .chain(event.payload.rejected_claim_ids.iter())
            .map(String::as_str)
            .collect();
        if !reviewed.is_subset(&asserted) {
            return Err(EngineError::ReferenceMismatch(
                "review references claims outside the proposal's asserted claim set".to_string(),
            ));
        }
        if event.payload.decision == ReviewDecision::Accept
            && event
                .payload
                .accepted_claim_ids
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != asserted
        {
            return Err(EngineError::ReferenceMismatch(
                "accept reviews must accept every asserted proposal claim".to_string(),
            ));
        }
        self.reviews.insert(event.event_id.clone(), event);
        Ok(())
    }

    fn apply_record(&mut self, event: EventEnvelope<MemoryRecord>) -> Result<(), EngineError> {
        event.validate_envelope()?;
        event.payload.validate()?;
        let proposal = self
            .proposals
            .get(&event.payload.proposal_event_id)
            .ok_or_else(|| {
                EngineError::ProposalNotFound(event.payload.proposal_event_id.clone())
            })?;
        if proposal.payload.digest()? != event.payload.proposal_digest {
            return Err(EngineError::ReferenceMismatch(
                "record proposal digest does not match the referenced proposal".to_string(),
            ));
        }
        if proposal.payload.channel_id != event.payload.channel_id {
            return Err(EngineError::ReferenceMismatch(
                "record and proposal channels must match".to_string(),
            ));
        }
        if proposal.author != event.author {
            return Err(EngineError::ReferenceMismatch(
                "record must be signed by the proposal author".to_string(),
            ));
        }
        let evaluation = self.ratification(&event.payload.proposal_event_id)?;
        if evaluation.status != RatificationStatus::Ratified {
            return Err(EngineError::ProposalNotRatified(evaluation.status));
        }
        let accepted_reviews: BTreeSet<&str> = evaluation
            .accepted_review_event_ids
            .iter()
            .map(String::as_str)
            .collect();
        if !event
            .payload
            .review_event_ids
            .iter()
            .all(|review_id| accepted_reviews.contains(review_id.as_str()))
        {
            return Err(EngineError::ReferenceMismatch(
                "record references a review that did not count toward ratification".to_string(),
            ));
        }
        if event.payload.review_event_ids.len()
            < self
                .policies
                .get(&proposal.payload.policy_id)
                .expect("policy existence checked when proposal was applied")
                .approval_quorum
        {
            return Err(EngineError::ReferenceMismatch(
                "record does not carry enough accepted review events for quorum".to_string(),
            ));
        }
        let proposal_claims: BTreeSet<&str> = proposal
            .payload
            .claims
            .iter()
            .filter(|claim| claim.state == ClaimState::Asserted)
            .map(|claim| claim.claim_id.as_str())
            .collect();
        let record_claims: BTreeSet<&str> = event
            .payload
            .claims
            .iter()
            .map(|claim| claim.claim_id.as_str())
            .collect();
        if proposal_claims != record_claims {
            return Err(EngineError::ReferenceMismatch(
                "record claims must exactly equal the proposal's asserted claims".to_string(),
            ));
        }
        for source in event.payload.evidence_event_ids() {
            if self.deleted_sources.contains(source) {
                return Err(EngineError::SourceUnavailable(source.to_string()));
            }
        }
        for superseded in &event.payload.supersedes_record_ids {
            let Some(existing) = self
                .records
                .values()
                .find(|candidate| candidate.payload.record_id == *superseded)
            else {
                return Err(EngineError::UnknownSupersededRecord(superseded.clone()));
            };
            if existing.payload.channel_id != event.payload.channel_id {
                return Err(EngineError::ReferenceMismatch(
                    "a record may supersede records only in the same channel".to_string(),
                ));
            }
        }

        let mut graph = self.supersession_graph();
        graph.insert(
            event.payload.record_id.clone(),
            event.payload.supersedes_record_ids.clone(),
        );
        if has_supersession_cycle(&graph) {
            return Err(EngineError::SupersessionCycle);
        }
        self.records.insert(event.event_id.clone(), event);
        Ok(())
    }

    fn apply_invalidation(
        &mut self,
        event: EventEnvelope<Invalidation>,
    ) -> Result<(), EngineError> {
        event.validate_envelope()?;
        event.payload.validate()?;
        let record = self
            .records
            .get(&event.payload.target_record_event_id)
            .ok_or_else(|| {
                EngineError::RecordNotFound(event.payload.target_record_event_id.clone())
            })?;
        if record.payload.record_id != event.payload.target_record_id {
            return Err(EngineError::ReferenceMismatch(
                "invalidation target record digest does not match".to_string(),
            ));
        }
        if record.author != event.author {
            return Err(EngineError::ReferenceMismatch(
                "invalidation must be signed by the record author".to_string(),
            ));
        }
        let record_claims: BTreeSet<&str> = record
            .payload
            .claims
            .iter()
            .map(|claim| claim.claim_id.as_str())
            .collect();
        if !event
            .payload
            .affected_claim_ids
            .iter()
            .all(|claim_id| record_claims.contains(claim_id.as_str()))
        {
            return Err(EngineError::ReferenceMismatch(
                "invalidation references claims outside the record".to_string(),
            ));
        }
        let sources = record.payload.evidence_event_ids();
        if !event
            .payload
            .source_event_ids
            .iter()
            .all(|event_id| sources.contains(event_id.as_str()))
        {
            return Err(EngineError::ReferenceMismatch(
                "invalidation references sources outside the record".to_string(),
            ));
        }
        if event.payload.reason == InvalidationReason::SourceDeleted
            && !event
                .payload
                .source_event_ids
                .iter()
                .all(|event_id| self.deleted_sources.contains(event_id))
        {
            return Err(EngineError::ReferenceMismatch(
                "source-deleted invalidation requires observed source deletion".to_string(),
            ));
        }
        self.invalidations.insert(event.event_id.clone(), event);
        Ok(())
    }

    pub fn ratification(
        &self,
        proposal_event_id: &str,
    ) -> Result<RatificationEvaluation, EngineError> {
        let proposal = self
            .proposals
            .get(proposal_event_id)
            .ok_or_else(|| EngineError::ProposalNotFound(proposal_event_id.to_string()))?;
        if proposal
            .payload
            .evidence_event_ids()
            .iter()
            .any(|source| self.deleted_sources.contains(*source))
        {
            return Ok(RatificationEvaluation {
                status: RatificationStatus::Unavailable,
                accepted_review_event_ids: vec![],
                rejected_review_event_ids: vec![],
                revision_review_event_ids: vec![],
                unauthorized_review_event_ids: vec![],
                conflicting_reviewers: vec![],
            });
        }
        let policy = self
            .policies
            .get(&proposal.payload.policy_id)
            .ok_or_else(|| EngineError::UnknownPolicy(proposal.payload.policy_id.clone()))?;

        let mut by_reviewer: BTreeMap<&str, Vec<(&str, ReviewDecision)>> = BTreeMap::new();
        let mut unauthorized = Vec::new();
        for (event_id, review) in &self.reviews {
            if review.payload.proposal_event_id != proposal_event_id {
                continue;
            }
            if !policy.ratifiers.contains(&review.author) {
                unauthorized.push(event_id.clone());
                continue;
            }
            by_reviewer
                .entry(&review.author)
                .or_default()
                .push((event_id, review.payload.decision));
        }

        let mut accepted = Vec::new();
        let mut rejected = Vec::new();
        let mut revise = Vec::new();
        let mut conflicting_reviewers = Vec::new();
        for (reviewer, reviews) in by_reviewer {
            let decisions: BTreeSet<ReviewDecision> =
                reviews.iter().map(|(_, decision)| *decision).collect();
            if decisions.len() > 1 {
                conflicting_reviewers.push(reviewer.to_string());
                continue;
            }
            // Multiple identical signed reviews by one reviewer count once.
            let event_id = reviews
                .iter()
                .map(|(event_id, _)| *event_id)
                .min()
                .expect("reviewer group is non-empty")
                .to_string();
            match decisions.iter().next().expect("decision set is non-empty") {
                ReviewDecision::Accept => accepted.push(event_id),
                ReviewDecision::Reject => rejected.push(event_id),
                ReviewDecision::Revise => revise.push(event_id),
            }
        }
        accepted.sort();
        rejected.sort();
        revise.sort();
        unauthorized.sort();
        conflicting_reviewers.sort();

        let status = if !conflicting_reviewers.is_empty()
            || (!accepted.is_empty() && (!rejected.is_empty() || !revise.is_empty()))
            || (!rejected.is_empty() && !revise.is_empty())
        {
            RatificationStatus::Contested
        } else if accepted.len() >= policy.approval_quorum {
            RatificationStatus::Ratified
        } else if rejected.len() >= policy.rejection_threshold {
            RatificationStatus::Rejected
        } else if !revise.is_empty() {
            RatificationStatus::RevisionRequested
        } else {
            RatificationStatus::Pending
        };

        Ok(RatificationEvaluation {
            status,
            accepted_review_event_ids: accepted,
            rejected_review_event_ids: rejected,
            revision_review_event_ids: revise,
            unauthorized_review_event_ids: unauthorized,
            conflicting_reviewers,
        })
    }

    pub fn record_availability(
        &self,
        record_event_id: &str,
    ) -> Result<RecordAvailability, EngineError> {
        let record = self
            .records
            .get(record_event_id)
            .ok_or_else(|| EngineError::RecordNotFound(record_event_id.to_string()))?;
        if self
            .invalidations
            .values()
            .any(|event| event.payload.target_record_event_id == record_event_id)
        {
            return Ok(RecordAvailability::ExplicitlyInvalidated);
        }
        let deleted: Vec<String> = record
            .payload
            .evidence_event_ids()
            .into_iter()
            .filter(|source| self.deleted_sources.contains(*source))
            .map(str::to_string)
            .collect();
        if !deleted.is_empty() {
            return Ok(RecordAvailability::SourceUnavailable(deleted));
        }
        if self.records.iter().any(|(other_event_id, other)| {
            other_event_id != record_event_id
                && self
                    .base_availability(other_event_id)
                    .is_some_and(|available| available)
                && other
                    .payload
                    .supersedes_record_ids
                    .contains(&record.payload.record_id)
        }) {
            return Ok(RecordAvailability::Superseded);
        }
        Ok(RecordAvailability::Available)
    }

    fn base_availability(&self, record_event_id: &str) -> Option<bool> {
        let record = self.records.get(record_event_id)?;
        let invalidated = self
            .invalidations
            .values()
            .any(|event| event.payload.target_record_event_id == record_event_id);
        let deleted = record
            .payload
            .evidence_event_ids()
            .iter()
            .any(|source| self.deleted_sources.contains(*source));
        Some(!invalidated && !deleted)
    }

    pub fn current_records(&self) -> Vec<&EventEnvelope<MemoryRecord>> {
        self.records
            .iter()
            .filter_map(|(event_id, record)| {
                (self.record_availability(event_id).ok()? == RecordAvailability::Available)
                    .then_some(record)
            })
            .collect()
    }

    pub fn pending_invalidations(&self) -> Vec<PendingInvalidation> {
        let mut pending = Vec::new();
        for (record_event_id, record) in &self.records {
            let deleted_sources: BTreeSet<&str> = record
                .payload
                .evidence_event_ids()
                .into_iter()
                .filter(|source| self.deleted_sources.contains(*source))
                .collect();
            if deleted_sources.is_empty()
                || self.invalidations.values().any(|invalidation| {
                    invalidation.payload.target_record_event_id == *record_event_id
                })
            {
                continue;
            }
            let affected_claim_ids: Vec<String> = record
                .payload
                .claims
                .iter()
                .filter(|claim| {
                    claim
                        .all_evidence()
                        .any(|evidence| deleted_sources.contains(evidence.event_id.as_str()))
                })
                .map(|claim| claim.claim_id.clone())
                .collect();
            pending.push(PendingInvalidation {
                target_record_event_id: record_event_id.clone(),
                target_record_id: record.payload.record_id.clone(),
                affected_claim_ids,
                source_event_ids: deleted_sources.into_iter().map(str::to_string).collect(),
            });
        }
        pending.sort_by(|left, right| {
            left.target_record_event_id
                .cmp(&right.target_record_event_id)
        });
        pending
    }

    pub fn supersession_graph(&self) -> BTreeMap<String, Vec<String>> {
        self.records
            .values()
            .map(|record| {
                (
                    record.payload.record_id.clone(),
                    record.payload.supersedes_record_ids.clone(),
                )
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use comb_core::{
        Claim, CoverageLane, CoverageRecord, CoverageStatus, EvidenceLocator, EvidenceRef,
        EvidenceRole, GeneratorInfo, RetrievalSnapshot, TemporalWindow, INVALIDATION_SCHEMA_V1,
        PROPOSAL_SCHEMA_V1, RECORD_SCHEMA_V1, REVIEW_SCHEMA_V1, SNAPSHOT_SCHEMA_V1,
    };

    use super::*;

    const CHANNEL: &str = "channel-a";

    fn id(byte: u8) -> String {
        hex::encode([byte; 32])
    }

    fn evidence(source: u8) -> EvidenceRef {
        EvidenceRef {
            relay: "wss://buzz.example".into(),
            community: "buzz.example".into(),
            channel_id: CHANNEL.into(),
            event_id: id(source),
            kind: 9,
            author: id(0xa0),
            created_at: 100,
            role: EvidenceRole::Supports,
            content_digest: id(source.wrapping_add(1)),
            locator: EvidenceLocator::WholeEvent,
        }
    }

    fn claim(source: u8, text: &str) -> Claim {
        Claim {
            claim_id: String::new(),
            text: text.into(),
            state: ClaimState::Asserted,
            valid_from: 100,
            valid_until: None,
            evidence: vec![evidence(source)],
            counterevidence: vec![],
            supersedes: vec![],
        }
        .seal()
        .unwrap()
    }

    fn proposal(claims: Vec<Claim>) -> Proposal {
        let mut source_event_ids: Vec<String> = claims
            .iter()
            .flat_map(Claim::all_evidence)
            .map(|evidence| evidence.event_id.clone())
            .collect();
        source_event_ids.sort();
        source_event_ids.dedup();
        let count = source_event_ids.len() as u64;
        let mut coverage = CoverageRecord {
            status: CoverageStatus::Complete,
            requested_window: TemporalWindow {
                since: 10,
                until: 20,
            },
            capture_started_at: 21,
            capture_completed_at: 22,
            lanes: vec![CoverageLane {
                channel_id: CHANNEL.into(),
                kinds: vec![9],
                count_before: count,
                retrieved: count,
                count_after: count,
                signature_failures: 0,
                unresolved_sources: 0,
            }],
            gaps: vec![],
        };
        coverage.normalize();
        let snapshot = RetrievalSnapshot {
            schema: SNAPSHOT_SCHEMA_V1.into(),
            snapshot_id: String::new(),
            community: "buzz.example".into(),
            channel_id: CHANNEL.into(),
            source_event_ids,
            coverage,
        }
        .seal()
        .unwrap();
        Proposal {
            schema: PROPOSAL_SCHEMA_V1.into(),
            proposal_id: String::new(),
            channel_id: CHANNEL.into(),
            created_at: 200,
            generator: GeneratorInfo {
                name: "fixture".into(),
                version: "1".into(),
                model: None,
                prompt_version: None,
            },
            policy_id: "default".into(),
            snapshot,
            claims,
        }
        .seal()
        .unwrap()
    }

    fn policy(ratifiers: &[u8], quorum: usize) -> RatificationPolicy {
        RatificationPolicy {
            policy_id: "default".into(),
            ratifiers: ratifiers.iter().map(|byte| id(*byte)).collect(),
            approval_quorum: quorum,
            rejection_threshold: 1,
        }
    }

    fn proposal_event(proposal: Proposal) -> EventEnvelope<Proposal> {
        EventEnvelope {
            event_id: id(0x10),
            author: id(0xc0),
            created_at: 200,
            payload: proposal,
        }
    }

    fn review_event(
        event_byte: u8,
        reviewer: u8,
        proposal_event: &EventEnvelope<Proposal>,
        decision: ReviewDecision,
    ) -> EventEnvelope<Review> {
        let claim_ids: Vec<String> = proposal_event
            .payload
            .claims
            .iter()
            .filter(|claim| claim.state == ClaimState::Asserted)
            .map(|claim| claim.claim_id.clone())
            .collect();
        let (accepted, rejected) = match decision {
            ReviewDecision::Accept => (claim_ids, vec![]),
            ReviewDecision::Reject => (vec![], claim_ids),
            ReviewDecision::Revise => (vec![], vec![]),
        };
        EventEnvelope {
            event_id: id(event_byte),
            author: id(reviewer),
            created_at: 210,
            payload: Review {
                schema: REVIEW_SCHEMA_V1.into(),
                review_id: String::new(),
                proposal_event_id: proposal_event.event_id.clone(),
                proposal_digest: proposal_event.payload.digest().unwrap(),
                decision,
                accepted_claim_ids: accepted,
                rejected_claim_ids: rejected,
                policy_id: "default".into(),
            }
            .seal()
            .unwrap(),
        }
    }

    fn record_event(
        event_byte: u8,
        proposal: &EventEnvelope<Proposal>,
        reviews: &[&EventEnvelope<Review>],
        supersedes: Vec<String>,
    ) -> EventEnvelope<MemoryRecord> {
        EventEnvelope {
            event_id: id(event_byte),
            author: proposal.author.clone(),
            created_at: 220,
            payload: MemoryRecord {
                schema: RECORD_SCHEMA_V1.into(),
                record_id: String::new(),
                channel_id: proposal.payload.channel_id.clone(),
                created_at: 220,
                proposal_event_id: proposal.event_id.clone(),
                proposal_digest: proposal.payload.digest().unwrap(),
                review_event_ids: reviews
                    .iter()
                    .map(|review| review.event_id.clone())
                    .collect(),
                claims: proposal
                    .payload
                    .claims
                    .iter()
                    .filter(|claim| claim.state == ClaimState::Asserted)
                    .cloned()
                    .collect(),
                supersedes_record_ids: supersedes,
            }
            .seal()
            .unwrap(),
        }
    }

    #[test]
    fn quorum_ratifies_and_unauthorized_reviews_are_visible_but_ignored() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let review_a = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        let review_b = review_event(0x21, 0x42, &proposal, ReviewDecision::Accept);
        let outsider = review_event(0x22, 0x99, &proposal, ReviewDecision::Reject);
        let mut projection = Projection::new([policy(&[0x41, 0x42], 2)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::Review(review_a.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::Review(review_b.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::Review(outsider.clone()))
            .unwrap();

        let evaluation = projection.ratification(&proposal.event_id).unwrap();
        assert_eq!(evaluation.status, RatificationStatus::Ratified);
        assert_eq!(evaluation.accepted_review_event_ids.len(), 2);
        assert_eq!(
            evaluation.unauthorized_review_event_ids,
            vec![outsider.event_id]
        );
    }

    #[test]
    fn accept_and_reject_make_proposal_contested_without_ordering() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let accept = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        let reject = review_event(0x21, 0x42, &proposal, ReviewDecision::Reject);
        let mut projection = Projection::new([policy(&[0x41, 0x42], 1)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        projection.apply(EngineEvent::Review(reject)).unwrap();
        projection.apply(EngineEvent::Review(accept)).unwrap();
        assert_eq!(
            projection.ratification(&proposal.event_id).unwrap().status,
            RatificationStatus::Contested
        );
    }

    #[test]
    fn one_reviewer_cannot_submit_conflicting_decisions() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let accept = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        let reject = review_event(0x21, 0x41, &proposal, ReviewDecision::Reject);
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        projection.apply(EngineEvent::Review(accept)).unwrap();
        projection.apply(EngineEvent::Review(reject)).unwrap();
        let evaluation = projection.ratification(&proposal.event_id).unwrap();
        assert_eq!(evaluation.status, RatificationStatus::Contested);
        assert_eq!(evaluation.conflicting_reviewers, vec![id(0x41)]);
    }

    #[test]
    fn record_requires_ratification_and_exact_claim_set() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let review = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        let record = record_event(0x30, &proposal, &[&review], vec![]);
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        assert!(matches!(
            projection.apply(EngineEvent::Record(record.clone())),
            Err(EngineError::ProposalNotRatified(
                RatificationStatus::Pending
            ))
        ));
        projection
            .apply(EngineEvent::Review(review.clone()))
            .unwrap();
        projection.apply(EngineEvent::Record(record)).unwrap();
        assert_eq!(projection.current_records().len(), 1);
    }

    #[test]
    fn review_with_wrong_proposal_digest_is_rejected() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let mut review = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        review.payload.proposal_digest = id(0xee);
        review.payload.review_id = String::new();
        review.payload = review.payload.seal().unwrap();
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        assert!(matches!(
            projection.apply(EngineEvent::Review(review)),
            Err(EngineError::ReferenceMismatch(_))
        ));
    }

    #[test]
    fn deletion_observed_before_proposal_prevents_publication() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        projection
            .apply(EngineEvent::SourceDeleted(SourceDeletion {
                deletion_event_id: id(0x40),
                source_event_id: id(1),
                author: id(0xa0),
                observed_at: 199,
            }))
            .unwrap();
        assert!(matches!(
            projection.apply(EngineEvent::Proposal(proposal)),
            Err(EngineError::SourceUnavailable(_))
        ));
    }

    #[test]
    fn source_deletion_fails_closed_and_yields_pending_invalidation() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let review = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        let record = record_event(0x30, &proposal, &[&review], vec![]);
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::Review(review.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::Record(record.clone()))
            .unwrap();

        let outcome = projection
            .apply(EngineEvent::SourceDeleted(SourceDeletion {
                deletion_event_id: id(0x40),
                source_event_id: id(1),
                author: id(0xa0),
                observed_at: 230,
            }))
            .unwrap();
        let ApplyOutcome::SourceDeleted {
            pending_invalidations,
        } = outcome
        else {
            panic!("expected deletion outcome");
        };
        assert_eq!(pending_invalidations.len(), 1);
        assert_eq!(
            pending_invalidations[0].target_record_event_id,
            record.event_id
        );
        assert_eq!(
            projection.record_availability(&record.event_id).unwrap(),
            RecordAvailability::SourceUnavailable(vec![id(1)])
        );
        assert_eq!(
            projection.ratification(&proposal.event_id).unwrap().status,
            RatificationStatus::Unavailable
        );
        assert!(projection.current_records().is_empty());
    }

    #[test]
    fn explicit_invalidation_must_be_record_author_and_source_bound() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let review = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        let record = record_event(0x30, &proposal, &[&review], vec![]);
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::Review(review.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::Record(record.clone()))
            .unwrap();
        projection
            .apply(EngineEvent::SourceDeleted(SourceDeletion {
                deletion_event_id: id(0x40),
                source_event_id: id(1),
                author: id(0xa0),
                observed_at: 230,
            }))
            .unwrap();
        let invalidation = Invalidation {
            schema: INVALIDATION_SCHEMA_V1.into(),
            invalidation_id: String::new(),
            target_record_event_id: record.event_id.clone(),
            target_record_id: record.payload.record_id.clone(),
            affected_claim_ids: vec![record.payload.claims[0].claim_id.clone()],
            source_event_ids: vec![id(1)],
            reason: InvalidationReason::SourceDeleted,
            detected_at: 231,
        }
        .seal()
        .unwrap();
        let envelope = EventEnvelope {
            event_id: id(0x41),
            author: proposal.author.clone(),
            created_at: 231,
            payload: invalidation,
        };
        projection
            .apply(EngineEvent::Invalidation(envelope))
            .unwrap();
        assert_eq!(
            projection.record_availability(&record.event_id).unwrap(),
            RecordAvailability::ExplicitlyInvalidated
        );
        assert!(projection.pending_invalidations().is_empty());
    }

    #[test]
    fn supersession_hides_old_record_but_invalid_new_record_restores_it() {
        let proposal_a = proposal_event(proposal(vec![claim(1, "Launch Friday")]));
        let review_a = review_event(0x20, 0x41, &proposal_a, ReviewDecision::Accept);
        let record_a = record_event(0x30, &proposal_a, &[&review_a], vec![]);
        let mut proposal_b = proposal_event(proposal(vec![claim(2, "Delay launch")]));
        proposal_b.event_id = id(0x11);
        let review_b = review_event(0x21, 0x41, &proposal_b, ReviewDecision::Accept);
        let record_b = record_event(
            0x31,
            &proposal_b,
            &[&review_b],
            vec![record_a.payload.record_id.clone()],
        );
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        for event in [
            EngineEvent::Proposal(proposal_a.clone()),
            EngineEvent::Review(review_a),
            EngineEvent::Record(record_a.clone()),
            EngineEvent::Proposal(proposal_b.clone()),
            EngineEvent::Review(review_b),
            EngineEvent::Record(record_b.clone()),
        ] {
            projection.apply(event).unwrap();
        }
        assert_eq!(
            projection.record_availability(&record_a.event_id).unwrap(),
            RecordAvailability::Superseded
        );
        projection
            .apply(EngineEvent::SourceDeleted(SourceDeletion {
                deletion_event_id: id(0x42),
                source_event_id: id(2),
                author: id(0xa0),
                observed_at: 240,
            }))
            .unwrap();
        assert_eq!(
            projection.record_availability(&record_a.event_id).unwrap(),
            RecordAvailability::Available
        );
    }

    #[test]
    fn unknown_superseded_record_is_rejected_and_graph_cycle_guard_is_active() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let review = review_event(0x20, 0x41, &proposal, ReviewDecision::Accept);
        let record = record_event(0x30, &proposal, &[&review], vec![id(0xfe)]);
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        projection
            .apply(EngineEvent::Proposal(proposal.clone()))
            .unwrap();
        projection.apply(EngineEvent::Review(review)).unwrap();
        assert!(matches!(
            projection.apply(EngineEvent::Record(record)),
            Err(EngineError::UnknownSupersededRecord(_))
        ));

        let mut graph = BTreeMap::new();
        graph.insert(id(1), vec![id(2)]);
        graph.insert(id(2), vec![id(1)]);
        assert!(has_supersession_cycle(&graph));
    }

    #[test]
    fn apply_is_idempotent_and_detects_event_id_collision() {
        let proposal = proposal_event(proposal(vec![claim(1, "Delay launch")]));
        let mut projection = Projection::new([policy(&[0x41], 1)]).unwrap();
        assert_eq!(
            projection
                .apply(EngineEvent::Proposal(proposal.clone()))
                .unwrap(),
            ApplyOutcome::Applied
        );
        assert_eq!(
            projection
                .apply(EngineEvent::Proposal(proposal.clone()))
                .unwrap(),
            ApplyOutcome::Duplicate
        );
        let mut changed = proposal;
        changed.created_at += 1;
        assert!(matches!(
            projection.apply(EngineEvent::Proposal(changed)),
            Err(EngineError::EventIdCollision(_))
        ));
    }
}
