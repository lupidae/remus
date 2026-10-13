//! `remus push`: the introspected schema, sent to romulus as a new version of
//! one database. The token names the database, so there is nothing else to say.

use remus_core::Schema;
use serde::Deserialize;

use crate::{Error, cli::PushArgs};

#[derive(Debug, Deserialize)]
pub struct Pushed {
    pub number: i32,
    /// False when the schema is identical to the latest version: romulus keeps
    /// one copy, so a CI job can push on every merge.
    pub created: bool,
    pub url: String,
}

#[derive(Deserialize)]
struct Refusal {
    error: String,
}

/// JSON, not SQL: it is the lossless format, and romulus stores it as is
/// instead of compiling anything.
pub async fn push(schema: &Schema, args: &PushArgs) -> Result<Pushed, Error> {
    let body = remus_core::emit::json(&schema.clone().canonical())?;
    let endpoint = format!("{}/api/push", args.to.trim_end_matches('/'));
    let authorization = format!("Bearer {}", args.token);
    let message = args.message.clone().unwrap_or_default();

    let (status, text) = tokio::task::spawn_blocking(move || {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .http_status_as_error(false)
            .build()
            .into();
        let mut response = agent
            .post(&endpoint)
            .header("Authorization", &authorization)
            .header("Content-Type", "application/json")
            .query("message", &message)
            .send(body)?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string()?;
        Ok::<_, ureq::Error>((status, text))
    })
    .await
    .map_err(|_| Error::PushInterrupted)?
    .map_err(Error::Push)?;

    match status {
        200 | 201 => serde_json::from_str(&text).map_err(|_| Error::PushAnswer),
        401 => Err(Error::PushRefused(
            "unknown token: create one in the database's settings in romulus".to_owned(),
        )),
        _ => Err(Error::PushRefused(
            serde_json::from_str::<Refusal>(&text)
                .map(|r| r.error)
                .unwrap_or_else(|_| format!("romulus answered {status}")),
        )),
    }
}
