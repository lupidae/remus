//! Runs the introspection query against a live database.
//!
//! The query text is `remus_core::INTROSPECT_SQL`, the same bytes a user can run
//! in psql, so the connected and the paste-the-JSON paths share one contract.

use remus_core::{INTROSPECT_SQL, Schema};
use tokio_postgres_rustls::MakeRustlsConnect;

use crate::Error;

pub async fn fetch(url: &str) -> Result<Schema, Error> {
    let (client, connection) = tokio_postgres::connect(url, tls_connector()).await?;
    // The connection future drives the wire protocol and must be polled
    // concurrently with the query.
    let driver = tokio::spawn(connection);

    let row = client.query_one(INTROSPECT_SQL, &[]).await?;
    let payload: String = row.try_get(0)?;

    // Dropping the client ends the connection cleanly. A failure at that point
    // cannot affect a payload we already hold, so it is not worth reporting.
    drop(client);
    let _ = driver.await;

    Ok(Schema::from_json(&payload)?)
}

/// Public roots only. Providers with private CAs need `sslmode=disable` on a
/// trusted network or the credential-free `--print-sql` path for now.
fn tls_connector() -> MakeRustlsConnect {
    let roots = rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    MakeRustlsConnect::new(config)
}
