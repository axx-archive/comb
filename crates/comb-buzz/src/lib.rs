//! Public-protocol adapter for running Comb against an unmodified Buzz relay.
//!
//! The adapter intentionally has no dependency on Buzz's internal crates or
//! database. It authenticates with a normal Nostr key, scopes every query to a
//! single Buzz channel, validates returned signatures, and signs Comb's
//! compatibility events with the publishing actor's own identity.

use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use futures_util::{SinkExt, StreamExt};
use nostr::{
    Alphabet, Event, EventBuilder, Filter, JsonUtil, Keys, Kind, RelayUrl, SingleLetterTag, Tag,
};
use serde::Serialize;
use serde_json::{json, Value};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url as WsUrl;
use uuid::Uuid;

/// Buzz message kind used by the compatibility protocol.
pub const BUZZ_MESSAGE_KIND: u16 = 9;

/// Event roles in Comb's compatibility protocol.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CombEventRole {
    /// A machine-generated, human-reviewable knowledge proposal.
    Proposal,
    /// A review signed by the reviewing human.
    Review,
    /// A ratified record with evidence receipts.
    Record,
    /// A body-free notice that earlier evidence is no longer available.
    Invalidation,
}

impl CombEventRole {
    /// Stable wire name used in the signed `comb` tag.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Proposal => "proposal",
            Self::Review => "review",
            Self::Record => "record",
            Self::Invalidation => "invalidation",
        }
    }
}

/// Options shared by Comb compatibility events.
#[derive(Clone, Debug, Default)]
pub struct CompatibilityTags<'a> {
    /// Signed source event IDs that support the artifact.
    pub source_event_ids: &'a [String],
    /// Earlier proposal or record IDs this event supersedes.
    pub supersedes_event_ids: &'a [String],
    /// Optional proposal or record that this event reviews or invalidates.
    pub target_event_id: Option<&'a str>,
}

/// Sign a channel-local Comb compatibility event.
///
/// The caller supplies the actor key explicitly. This is important for reviews:
/// Comb never signs on behalf of the human reviewer.
pub fn sign_compatibility_event<T: Serialize>(
    keys: &Keys,
    channel_id: Uuid,
    role: CombEventRole,
    stable_id: &str,
    body: &T,
    options: &CompatibilityTags<'_>,
) -> Result<Event> {
    if stable_id.trim().is_empty() {
        bail!("stable Comb ID cannot be empty");
    }

    let mut tags = vec![
        Tag::parse(["h", channel_id.to_string().as_str()])?,
        Tag::parse(["comb", role.as_str(), "v1"])?,
        Tag::parse(["comb-id", stable_id])?,
    ];

    for event_id in options.source_event_ids {
        validate_event_id(event_id)?;
        tags.push(Tag::parse(["e", event_id.as_str(), "", "source"])?);
    }
    for event_id in options.supersedes_event_ids {
        validate_event_id(event_id)?;
        tags.push(Tag::parse(["e", event_id.as_str(), "", "supersedes"])?);
    }
    if let Some(event_id) = options.target_event_id {
        validate_event_id(event_id)?;
        tags.push(Tag::parse(["e", event_id, "", role.as_str()])?);
    }

    let content = match role {
        CombEventRole::Invalidation => String::new(),
        _ => serde_json::to_string(body)?,
    };
    EventBuilder::new(Kind::Custom(BUZZ_MESSAGE_KIND), content)
        .tags(tags)
        .sign_with_keys(keys)
        .context("failed to sign Comb compatibility event")
}

/// Sign a Buzz-compatible channel deletion event for a Comb-authored message.
pub fn sign_delete_event(keys: &Keys, channel_id: Uuid, target_event_id: &str) -> Result<Event> {
    validate_event_id(target_event_id)?;
    let tags = vec![
        Tag::parse(["h", channel_id.to_string().as_str()])?,
        Tag::parse(["e", target_event_id])?,
    ];
    EventBuilder::new(Kind::Custom(5), "")
        .tags(tags)
        .sign_with_keys(keys)
        .context("failed to sign deletion event")
}

/// Sign a Buzz NIP-29 channel creation event for the deterministic demo.
pub fn sign_create_channel(
    keys: &Keys,
    channel_id: Uuid,
    name: &str,
    private: bool,
) -> Result<Event> {
    if name.trim().is_empty() {
        bail!("channel name cannot be empty");
    }
    let visibility = if private { "private" } else { "open" };
    let tags = vec![
        Tag::parse(["h", channel_id.to_string().as_str()])?,
        Tag::parse(["name", name])?,
        Tag::parse(["visibility", visibility])?,
        Tag::parse(["channel_type", "stream"])?,
        Tag::parse([
            "about",
            "Disposable That's Cool workspace for the Comb compatibility proof",
        ])?,
    ];
    EventBuilder::new(Kind::Custom(9007), "")
        .tags(tags)
        .sign_with_keys(keys)
        .context("failed to sign channel creation event")
}

/// Sign a Buzz NIP-29 member-add event.
pub fn sign_add_member(
    owner_keys: &Keys,
    channel_id: Uuid,
    target_pubkey: &str,
    role: Option<&str>,
) -> Result<Event> {
    nostr::PublicKey::from_hex(target_pubkey)
        .with_context(|| format!("invalid member public key: {target_pubkey}"))?;
    let mut tags = vec![
        Tag::parse(["h", channel_id.to_string().as_str()])?,
        Tag::parse(["p", target_pubkey])?,
    ];
    if let Some(role) = role {
        if !matches!(role, "owner" | "admin" | "member" | "bot") {
            bail!("unsupported Buzz member role: {role}");
        }
        tags.push(Tag::parse(["role", role])?);
    }
    EventBuilder::new(Kind::Custom(9000), "")
        .tags(tags)
        .sign_with_keys(owner_keys)
        .context("failed to sign member-add event")
}

/// Sign a minimal Nostr profile so a disposable proof identity is legible in Buzz.
pub fn sign_profile(keys: &Keys, display_name: &str, about: &str) -> Result<Event> {
    if display_name.trim().is_empty() {
        bail!("profile display name cannot be empty");
    }
    let content = serde_json::to_string(&json!({
        "display_name": display_name,
        "name": display_name.to_ascii_lowercase().replace(' ', "-"),
        "about": about,
    }))?;
    EventBuilder::new(Kind::Custom(0), content)
        .sign_with_keys(keys)
        .context("failed to sign profile event")
}

/// Sign an ordinary Buzz channel message used as primary evidence.
pub fn sign_channel_message(keys: &Keys, channel_id: Uuid, content: &str) -> Result<Event> {
    if content.trim().is_empty() {
        bail!("channel message cannot be empty");
    }
    EventBuilder::new(Kind::Custom(BUZZ_MESSAGE_KIND), content)
        .tag(Tag::parse(["h", channel_id.to_string().as_str()])?)
        .sign_with_keys(keys)
        .context("failed to sign channel message")
}

fn validate_event_id(event_id: &str) -> Result<()> {
    nostr::EventId::from_hex(event_id)
        .with_context(|| format!("invalid Nostr event ID: {event_id}"))?;
    Ok(())
}

/// Authenticated Buzz WebSocket client.
#[derive(Clone)]
pub struct BuzzClient {
    relay_url: String,
    keys: Keys,
    timeout: Duration,
}

impl BuzzClient {
    /// Create a client for a Buzz identity.
    pub fn new(relay_url: impl Into<String>, keys: Keys) -> Self {
        Self {
            relay_url: relay_url.into(),
            keys,
            timeout: Duration::from_secs(8),
        }
    }

    /// Override the default eight-second operation timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Public key used to authenticate and publish.
    #[must_use]
    pub fn public_key_hex(&self) -> String {
        self.keys.public_key().to_hex()
    }

    /// Publish a pre-signed event and wait for an affirmative relay `OK`.
    pub async fn publish(&self, event: &Event) -> Result<()> {
        event
            .verify()
            .context("refusing to publish an invalid signature")?;
        let mut ws = self.connect_authenticated().await?;
        send_json(&mut ws, json!(["EVENT", event])).await?;
        wait_for_ok(&mut ws, &event.id.to_hex(), self.timeout).await
    }

    /// Query one channel and verify every returned event signature.
    pub async fn query_channel(
        &self,
        channel_id: Uuid,
        kinds: &[u16],
        limit: usize,
    ) -> Result<Vec<Event>> {
        let mut filter = Filter::new().custom_tag(
            SingleLetterTag::lowercase(Alphabet::H),
            channel_id.to_string(),
        );
        for kind in kinds {
            filter = filter.kind(Kind::Custom(*kind));
        }
        if limit > 0 {
            filter = filter.limit(limit);
        }
        self.query(filter).await
    }

    /// Execute a signed, authenticated subscription until `EOSE`.
    pub async fn query(&self, filter: Filter) -> Result<Vec<Event>> {
        let mut ws = self.connect_authenticated().await?;
        let subscription_id = format!("comb-query-{}", Uuid::new_v4());
        send_json(&mut ws, json!(["REQ", subscription_id, filter])).await?;

        let mut events = Vec::new();
        loop {
            let text = next_text(&mut ws, self.timeout).await?;
            let value: Value = serde_json::from_str(&text)?;
            match value.get(0).and_then(Value::as_str) {
                Some("EVENT") => {
                    let event_value = value
                        .get(2)
                        .ok_or_else(|| anyhow!("EVENT response missing event payload"))?;
                    let event = Event::from_json(event_value.to_string())?;
                    event
                        .verify()
                        .context("relay returned an invalid signature")?;
                    events.push(event);
                }
                Some("EOSE") => return Ok(events),
                Some("CLOSED") => {
                    let reason = value.get(2).and_then(Value::as_str).unwrap_or("unknown");
                    bail!("relay closed query: {reason}");
                }
                Some("NOTICE") => {
                    let reason = value.get(1).and_then(Value::as_str).unwrap_or("unknown");
                    bail!("relay notice during query: {reason}");
                }
                _ => {}
            }
        }
    }

    /// Resolve one event inside a channel, returning `None` when unavailable.
    pub async fn event_in_channel(
        &self,
        channel_id: Uuid,
        event_id: nostr::EventId,
    ) -> Result<Option<Event>> {
        let filter = Filter::new()
            .id(event_id)
            .custom_tag(
                SingleLetterTag::lowercase(Alphabet::H),
                channel_id.to_string(),
            )
            .limit(1);
        Ok(self.query(filter).await?.into_iter().next())
    }

    /// Execute a NIP-45 count for one channel.
    pub async fn count_channel(&self, channel_id: Uuid, kinds: &[u16]) -> Result<u64> {
        let mut filter = Filter::new().custom_tag(
            SingleLetterTag::lowercase(Alphabet::H),
            channel_id.to_string(),
        );
        for kind in kinds {
            filter = filter.kind(Kind::Custom(*kind));
        }
        let mut ws = self.connect_authenticated().await?;
        let subscription_id = format!("comb-count-{}", Uuid::new_v4());
        send_json(&mut ws, json!(["COUNT", subscription_id, filter])).await?;

        loop {
            let text = next_text(&mut ws, self.timeout).await?;
            let value: Value = serde_json::from_str(&text)?;
            match value.get(0).and_then(Value::as_str) {
                Some("COUNT") if value.get(1).and_then(Value::as_str) == Some(&subscription_id) => {
                    return value
                        .get(2)
                        .and_then(|payload| payload.get("count"))
                        .and_then(Value::as_u64)
                        .ok_or_else(|| anyhow!("COUNT response missing numeric count"));
                }
                Some("CLOSED") => {
                    let reason = value.get(2).and_then(Value::as_str).unwrap_or("unknown");
                    bail!("relay closed count: {reason}");
                }
                Some("NOTICE") => {
                    let reason = value.get(1).and_then(Value::as_str).unwrap_or("unknown");
                    bail!("relay notice during count: {reason}");
                }
                _ => {}
            }
        }
    }

    async fn connect_authenticated(&self) -> Result<Ws> {
        let parsed = WsUrl::parse(&self.relay_url)
            .with_context(|| format!("invalid relay URL: {}", self.relay_url))?;
        let (mut ws, _) = connect_async(parsed.as_str())
            .await
            .with_context(|| format!("failed to connect to {}", self.relay_url))?;

        let challenge = wait_for_auth_challenge(&mut ws, self.timeout).await?;
        let relay_url = RelayUrl::parse(&self.relay_url)?;
        let auth_event = EventBuilder::auth(challenge, relay_url)
            .sign_with_keys(&self.keys)
            .context("failed to sign Buzz authentication event")?;
        let auth_id = auth_event.id.to_hex();
        send_json(&mut ws, json!(["AUTH", auth_event])).await?;
        wait_for_ok(&mut ws, &auth_id, self.timeout).await?;
        Ok(ws)
    }
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn wait_for_auth_challenge(ws: &mut Ws, timeout: Duration) -> Result<String> {
    loop {
        let text = next_text(ws, timeout).await?;
        let value: Value = serde_json::from_str(&text)?;
        if value.get(0).and_then(Value::as_str) == Some("AUTH") {
            return value
                .get(1)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| anyhow!("AUTH message missing challenge"));
        }
    }
}

async fn wait_for_ok(ws: &mut Ws, event_id: &str, timeout: Duration) -> Result<()> {
    loop {
        let text = next_text(ws, timeout).await?;
        let value: Value = serde_json::from_str(&text)?;
        if value.get(0).and_then(Value::as_str) != Some("OK")
            || value.get(1).and_then(Value::as_str) != Some(event_id)
        {
            continue;
        }
        if value.get(2).and_then(Value::as_bool) == Some(true) {
            return Ok(());
        }
        let reason = value.get(3).and_then(Value::as_str).unwrap_or("unknown");
        bail!("relay rejected event {event_id}: {reason}");
    }
}

async fn next_text(ws: &mut Ws, timeout: Duration) -> Result<String> {
    loop {
        let message = tokio::time::timeout(timeout, ws.next())
            .await
            .context("timed out waiting for relay message")?
            .ok_or_else(|| anyhow!("relay closed the WebSocket"))??;
        match message {
            Message::Text(text) => return Ok(text.to_string()),
            Message::Ping(bytes) => ws.send(Message::Pong(bytes)).await?,
            Message::Close(frame) => bail!("relay closed connection: {frame:?}"),
            _ => {}
        }
    }
}

async fn send_json(ws: &mut Ws, value: Value) -> Result<()> {
    ws.send(Message::Text(value.to_string().into())).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Serialize)]
    struct Body<'a> {
        title: &'a str,
    }

    fn tag_values(event: &Event) -> Vec<Vec<String>> {
        event
            .tags
            .iter()
            .map(|tag| tag.as_slice().to_vec())
            .collect()
    }

    #[test]
    fn proposal_is_signed_and_channel_scoped() -> Result<()> {
        let keys = Keys::generate();
        let channel = Uuid::new_v4();
        let source = EventBuilder::text_note("source").sign_with_keys(&keys)?;
        let event = sign_compatibility_event(
            &keys,
            channel,
            CombEventRole::Proposal,
            "knowledge.launch-date.v1",
            &Body { title: "Launch" },
            &CompatibilityTags {
                source_event_ids: &[source.id.to_hex()],
                ..CompatibilityTags::default()
            },
        )?;
        event.verify()?;
        let tags = tag_values(&event);
        assert!(tags.contains(&vec!["h".into(), channel.to_string()]));
        assert!(tags.contains(&vec!["comb".into(), "proposal".into(), "v1".into()]));
        assert!(tags.contains(&vec![
            "e".into(),
            source.id.to_hex(),
            "".into(),
            "source".into()
        ]));
        Ok(())
    }

    #[test]
    fn reviewer_signs_their_own_review() -> Result<()> {
        let comb = Keys::generate();
        let reviewer = Keys::generate();
        let proposal = EventBuilder::text_note("proposal").sign_with_keys(&comb)?;
        let review = sign_compatibility_event(
            &reviewer,
            Uuid::new_v4(),
            CombEventRole::Review,
            "review.launch-date.alex",
            &serde_json::json!({ "decision": "approve" }),
            &CompatibilityTags {
                target_event_id: Some(&proposal.id.to_hex()),
                ..CompatibilityTags::default()
            },
        )?;
        assert_eq!(review.pubkey, reviewer.public_key());
        assert_ne!(review.pubkey, comb.public_key());
        Ok(())
    }

    #[test]
    fn invalidation_body_is_empty_and_only_carries_receipt_ids() -> Result<()> {
        let keys = Keys::generate();
        let record = EventBuilder::text_note("record").sign_with_keys(&keys)?;
        let invalidation = sign_compatibility_event(
            &keys,
            Uuid::new_v4(),
            CombEventRole::Invalidation,
            "invalidation.launch-date.v1",
            &serde_json::json!({ "must": "not leak" }),
            &CompatibilityTags {
                target_event_id: Some(&record.id.to_hex()),
                ..CompatibilityTags::default()
            },
        )?;
        assert!(invalidation.content.is_empty());
        assert!(!invalidation.as_json().contains("must"));
        Ok(())
    }

    #[test]
    fn malformed_evidence_id_is_rejected_before_signing() {
        let result = sign_compatibility_event(
            &Keys::generate(),
            Uuid::new_v4(),
            CombEventRole::Proposal,
            "knowledge.test.v1",
            &serde_json::json!({}),
            &CompatibilityTags {
                source_event_ids: &["not-an-event".into()],
                ..CompatibilityTags::default()
            },
        );
        assert!(result.is_err());
    }
}
