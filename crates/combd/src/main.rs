//! Long-running, channel-scoped Comb observer.
//!
//! Proposal extraction is intentionally policy-specific and currently exposed
//! through the deterministic CLI proof. The daemon establishes the production
//! safety boundary: one explicit identity, one explicit channel, authenticated
//! counts/reads, signature validation, and fail-closed shutdown on lost access.

use std::{env, time::Duration};

use anyhow::{Context, Result};
use comb_buzz::BuzzClient;
use nostr::Keys;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let relay = required_env("BUZZ_RELAY_URL")?;
    let channel_id = required_env("BUZZ_CHANNEL_ID")?
        .parse::<Uuid>()
        .context("BUZZ_CHANNEL_ID must be a UUID")?;
    let keys = Keys::parse(&required_env("BUZZ_PRIVATE_KEY")?)
        .context("BUZZ_PRIVATE_KEY must be a hex or nsec private key")?;
    let interval = env::var("COMB_POLL_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|seconds| *seconds >= 5)
        .unwrap_or(30);
    let client = BuzzClient::new(relay.clone(), keys);

    eprintln!(
        "{}",
        serde_json::json!({
            "event": "combd.started",
            "relay": relay,
            "channelId": channel_id,
            "publicKey": client.public_key_hex(),
            "mode": "channel-observer",
        })
    );

    let mut timer = tokio::time::interval(Duration::from_secs(interval));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                eprintln!("{}", serde_json::json!({"event": "combd.stopped"}));
                return Ok(());
            }
            _ = timer.tick() => {
                let count = client.count_channel(channel_id, &[9]).await.with_context(|| {
                    format!(
                        "lost authorized access to channel {channel_id}; refusing to continue"
                    )
                })?;
                eprintln!(
                    "{}",
                    serde_json::json!({
                        "event": "combd.channel_observed",
                        "channelId": channel_id,
                        "signedMessageCount": count,
                        "coverage": "not-captured",
                    })
                );
            }
        }
    }
}

fn required_env(name: &str) -> Result<String> {
    env::var(name).with_context(|| format!("{name} is required"))
}
