use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use backend::collections::{self, CollectionInfo};
use backend::settings;
use backend::torrent::SourceFile;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
const WAIT_TIMEOUT: Duration = Duration::from_secs(60);

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mode = args.next().context("missing peer mode")?;
    match mode.as_str() {
        "owner" => {
            let fixture = PathBuf::from(args.next().context("missing fixture path")?);
            let download_dir = PathBuf::from(args.next().context("missing download directory")?);
            let listen_port = parse_port(args.next())?;
            configure_engine(download_dir, listen_port).await?;
            run_owner(fixture).await
        }
        "joiner" => {
            let invite = args.next().context("missing invite code")?;
            let expected = args.next().context("missing expected fixture contents")?;
            let download_dir = PathBuf::from(args.next().context("missing download directory")?);
            let listen_port = parse_port(args.next())?;
            configure_engine(download_dir, listen_port).await?;
            run_joiner(invite, expected).await
        }
        _ => anyhow::bail!("unknown peer mode {mode:?}"),
    }
}

fn parse_port(value: Option<String>) -> Result<u16> {
    value
        .context("missing BitTorrent listen port")?
        .parse::<u16>()
        .context("invalid BitTorrent listen port")
}

async fn configure_engine(download_dir: PathBuf, listen_port: u16) -> Result<()> {
    let mut engine = settings::default_engine_settings();
    engine.download_dir = Some(download_dir.to_string_lossy().into_owned());
    engine.listen_port_start = listen_port;
    engine.listen_port_end = listen_port + 1;
    engine.enable_upnp_port_forwarding = false;
    engine.disable_dht = true;
    engine.persist_session = false;
    engine.fastresume = false;
    engine.peer_connect_timeout_secs = Some(2);
    engine.peer_read_write_timeout_secs = Some(5);
    settings::set_engine_settings(engine).await?;
    collections::start_engine().await
}

async fn run_owner(fixture: PathBuf) -> Result<()> {
    let length = std::fs::metadata(&fixture)
        .with_context(|| format!("reading fixture metadata for {fixture:?}"))?
        .len();
    let created = collections::create_collection_with_media(
        "two-instance-local-test".into(),
        vec![SourceFile {
            name: "fixture.txt".into(),
            path: fixture.to_string_lossy().into_owned(),
            length_bytes: Some(length),
        }],
    )
    .await?;
    let ready = wait_for_collection(&created.id, |collection| {
        collection.ingestion.is_none() && collection.media.iter().any(|media| media.fetched)
    })
    .await?;
    let invite = ready
        .invite_code
        .context("owner did not produce an encoded invite")?;
    let info_hash = ready
        .media
        .first()
        .map(|media| media.info_hash.clone())
        .context("owner collection has no media")?;
    let sync_address = collections::sync_address().await?;
    println!("OWNER_READY {invite} {sync_address} {info_hash}");
    keep_alive().await
}

async fn run_joiner(invite: String, expected: String) -> Result<()> {
    let joined = collections::join_collection(invite, "local-joiner".into()).await?;
    println!("JOINED {}", joined.id);
    let fetched = wait_for_collection(&joined.id, |collection| {
        collection.pending_media == 0
            && collection
                .media
                .iter()
                .any(|media| media.fetched && media.absolute_path.is_some())
    })
    .await?;
    let media = fetched
        .media
        .iter()
        .find(|media| media.fetched && media.absolute_path.is_some())
        .context("joiner has no completed media path")?;
    let path = PathBuf::from(media.absolute_path.as_ref().unwrap());
    let contents = std::fs::read_to_string(&path)
        .with_context(|| format!("reading fetched fixture at {path:?}"))?;
    anyhow::ensure!(
        contents == expected,
        "fetched fixture contents do not match"
    );
    println!("FETCHED {} {}", media.info_hash, path.display());
    keep_alive().await
}

async fn wait_for_collection<F>(collection_id: &str, predicate: F) -> Result<CollectionInfo>
where
    F: Fn(&CollectionInfo) -> bool,
{
    let deadline = tokio::time::Instant::now() + WAIT_TIMEOUT;
    loop {
        let collections = collections::list_collections().await?;
        if let Some(collection) = collections.iter().find(|item| item.id == collection_id) {
            if predicate(collection) {
                return Ok(collection.clone());
            }
        }
        if tokio::time::Instant::now() >= deadline {
            anyhow::bail!("timed out waiting for collection {collection_id}");
        }
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}

async fn keep_alive() -> Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(3600)).await;
    }
}
