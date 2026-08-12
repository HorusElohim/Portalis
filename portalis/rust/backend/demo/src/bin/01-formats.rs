//! Step 1 — the canonical byte layouts, printed and pinned.
//!
//! These formats are a contract between clients, and nothing on the server
//! side can detect a peer that builds them differently: the service holds a
//! sealed manifest opaquely by design. So the only thing standing between two
//! implementations that disagree is a vector, and this is where the vectors
//! live.
//!
//! Every value below is fixed. Ed25519 signs deterministically and the sealed
//! manifest derives its nonce, so an implementation on another platform that
//! produces different bytes for these inputs is wrong, and this binary says
//! so rather than leaving it to be discovered over the wire.
//!
//! Run with `cargo run -p portalis-nexus-demo --bin 01-formats`.

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_protocol::{
    CONTENT_KEY_BYTES, ContentKey, EntryContext, INFO_HASH_BYTES, Manifest, ManifestContext,
    ManifestEntry, SHARE_ID_BYTES, open_entry, open_manifest, seal_entry, seal_manifest,
};

/// The collection every vector below belongs to.
const COLLECTION: [u8; SHARE_ID_BYTES] = [0x11; SHARE_ID_BYTES];
const REVISION: u64 = 7;
/// Never a real key. A vector's key is public by definition.
const CONTENT_KEY: ContentKey = [0x22; CONTENT_KEY_BYTES];
const AUTHOR_SEED: [u8; 32] = [0x33; 32];

/// The vectors themselves, as digests of the canonical bytes. A digest rather
/// than three hundred hex characters, because the whole point is a single
/// value another implementation can compare against — and any difference at
/// all, in any field, changes it.
const MANIFEST_HASH: &str = "1c861aa044645f9164619c447ee309f4baefbb25479cc9127b0fa42ff0ff9d2e";
const MANIFEST_BYTES: usize = 331;
const SEALED_DIGEST: &str = "47c068aa47f43518c90e38fc5fe883392669332f911c0e9c00f72624653b981a";
const SEALED_BYTES: usize = 360;

fn main() {
    let author = SigningKey::from_bytes(&AUTHOR_SEED);
    let manifest = manifest(&author);
    let hash = manifests(&manifest);
    entries(hash);

    println!("\nEvery vector above held.");
}

/// The manifest, its hash, and the sealed form the service stores.
fn manifests(manifest: &Manifest) -> portalis_nexus_protocol::ManifestHash {
    section("Manifest — canonical plaintext");
    let encoded = manifest.encode();
    print_bytes(&encoded);
    pin(
        "manifest bytes",
        &encoded,
        MANIFEST_BYTES,
        MANIFEST_HASH,
        "a domain prefix, a little-endian count, then two entries by info hash",
    );

    section("Manifest — content hash");
    let hash = manifest.hash();
    println!("  blake3 = {}", hex(&hash));
    assert_eq!(hex(&hash), MANIFEST_HASH, "the content hash is the vector");
    println!("  This is the name a revision points at, and what a peer checks");
    println!("  a fetched manifest against before believing a byte of it.");

    section("Sealed manifest — what the service stores and cannot read");
    let sealed = seal_manifest(&CONTENT_KEY, COLLECTION, REVISION, manifest);
    print_bytes(&sealed);
    println!(
        "  version {} · nonce derived from collection, revision and hash",
        sealed[0]
    );

    // The derived nonce is the point: a publisher whose acknowledgement was
    // lost re-seals to identical bytes, so a retry is recognisable as one
    // rather than arriving as a second, different revision.
    assert_eq!(
        sealed,
        seal_manifest(&CONTENT_KEY, COLLECTION, REVISION, manifest),
        "sealing is deterministic, so a retry is recognisable"
    );
    println!("  Re-sealing the same revision reproduces these bytes exactly.");
    pin(
        "sealed manifest",
        &sealed,
        SEALED_BYTES,
        SEALED_DIGEST,
        "a version byte, a derived nonce, then ciphertext and tag",
    );

    let context = ManifestContext {
        collection_id: COLLECTION,
        revision: REVISION,
        manifest_hash: hash,
    };
    let opened = open_manifest(&CONTENT_KEY, &context, &sealed).expect("opens under its context");
    assert_eq!(opened.encode(), encoded);
    println!("  Opened under its own context, and the plaintext matches.");

    section("Sealed manifest — refusals");
    let wrong_revision = ManifestContext {
        revision: REVISION + 1,
        ..context
    };
    refused(
        "a later revision",
        open_manifest(&CONTENT_KEY, &wrong_revision, &sealed).err(),
    );
    let wrong_collection = ManifestContext {
        collection_id: [0x44; SHARE_ID_BYTES],
        ..context
    };
    refused(
        "another collection",
        open_manifest(&CONTENT_KEY, &wrong_collection, &sealed).err(),
    );
    refused(
        "a different content key",
        open_manifest(&[0x55; CONTENT_KEY_BYTES], &context, &sealed).err(),
    );
    let mut tampered = sealed.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 1;
    refused(
        "one flipped bit",
        open_manifest(&CONTENT_KEY, &context, &tampered).err(),
    );

    hash
}

/// One entry's `.torrent`, sealed under the same key.
fn entries(_manifest_hash: portalis_nexus_protocol::ManifestHash) {
    section("Entry payload — one entry's .torrent");
    // The plaintext is the descriptor and nothing else. It used to repeat the
    // collection name and info hash the manifest entry already carries, which
    // was two more chances for the two to disagree.
    let torrent = b"d8:announce0:4:infod4:name3:onee".to_vec();
    let entry_context = EntryContext {
        collection_id: COLLECTION,
        info_hash: [0x01; 20],
    };
    let payload = seal_entry(&CONTENT_KEY, &entry_context, &torrent).expect("seals");
    println!("  plaintext = the raw .torrent, {} bytes", torrent.len());
    println!(
        "  sealed    = {} bytes, version {}",
        payload.len(),
        payload[0]
    );
    println!("  The nonce is random here: an entry has no revision to derive");
    println!("  one from, and no retry needs to reproduce it.");

    let opened = open_entry(&CONTENT_KEY, &entry_context, &payload).expect("opens");
    assert_eq!(opened, torrent);
    println!("  Round trips to the same descriptor.");

    section("Entry payload — refusals");
    let other_entry = EntryContext {
        info_hash: [0x02; 20],
        ..entry_context
    };
    refused(
        "another entry in the same collection",
        open_entry(&CONTENT_KEY, &other_entry, &payload).err(),
    );
    let other_collection = EntryContext {
        collection_id: [0x44; SHARE_ID_BYTES],
        ..entry_context
    };
    refused(
        "the same entry in another collection",
        open_entry(&CONTENT_KEY, &other_collection, &payload).err(),
    );
}

/// Two entries, ascending by info hash, each signed by the same author.
fn manifest(author: &SigningKey) -> Manifest {
    let entries = vec![
        entry(author, [0x01; INFO_HASH_BYTES], "one.jpg", None),
        entry(author, [0x02; INFO_HASH_BYTES], "two.jpg", Some([0x66; 32])),
    ];
    Manifest::new(entries).expect("a canonical manifest")
}

fn entry(
    author: &SigningKey,
    info_hash: [u8; INFO_HASH_BYTES],
    name: &str,
    thumbnail_hash: Option<[u8; 32]>,
) -> ManifestEntry {
    let mut entry = ManifestEntry {
        info_hash,
        name: name.to_owned(),
        thumbnail_hash,
        author_public_key: author.verifying_key().to_bytes(),
        added_at_unix_ns: 1_700_000_000_000_000_000,
        signature: [0; 64],
    };
    entry.signature = author.sign(&entry.signing_payload()).to_bytes();
    entry
}

/// Holds a vector, and says what it is so a mismatch on another platform
/// points somewhere rather than at an opaque blob.
fn pin(what: &str, bytes: &[u8], expected_len: usize, expected_digest: &str, shape: &str) {
    println!("  {what}: {} bytes — {shape}", bytes.len());
    let digest = hex(blake3::hash(bytes).as_bytes());
    println!("  digest = {digest}");
    assert_eq!(
        bytes.len(),
        expected_len,
        "{what}: the length changed, so a field was added, removed or resized"
    );
    assert_eq!(
        digest, expected_digest,
        "{what}: the bytes changed. If that was deliberate, the format changed \
         and the version byte owes an increment; if not, something upstream \
         moved underneath this format"
    );
}

fn refused(what: &str, error: Option<impl std::fmt::Display>) {
    let error = error.expect("this must not open");
    println!("  {what} → refused: {error}");
}

fn print_bytes(bytes: &[u8]) {
    for (index, chunk) in bytes.chunks(32).enumerate() {
        println!("  {:04x}  {}", index * 32, hex(chunk));
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn section(title: &str) {
    println!("\n{title}\n{}", "─".repeat(title.chars().count()));
}
