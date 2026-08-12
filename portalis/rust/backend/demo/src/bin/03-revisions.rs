//! Step 3 — the revision chain: three revisions, a rollback, and a fork.
//!
//! Decision D3 says a collection is a chain of signed revisions rather than a
//! row the service keeps for you. The difference only shows under attack, and
//! the attacks here are the uncomfortable ones: nothing is forged in either.
//!
//! An **older revision, served as the current one**, is entirely genuine — the
//! owner really did sign it, last week, before removing someone. A **fork** is
//! two revisions that both verify perfectly and disagree about what happened.
//! Neither is detectable by looking at the object in front of you, which is
//! why the service cannot be the one to decide and why a reader holds the
//! highest revision it has verified.
//!
//! That held state is the whole trick. Take it away and a rollback is
//! indistinguishable from an update.
//!
//! Run with `cargo run -p portalis-nexus-demo --bin 03-revisions`.

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_client::{ChainError, ChainStore, MemoryChainStore, verify_revision};
use portalis_nexus_protocol::{
    Action, DEVICE_KEY_BYTES, DeviceLog, ENCRYPTION_KEY_BYTES, LogEntry, Member, NO_PREVIOUS_ENTRY,
    NO_PREVIOUS_REVISION, Revision, RevisionHash, SHARE_ID_BYTES, SIGNATURE_BYTES,
};

const COLLECTION: [u8; SHARE_ID_BYTES] = [0x11; SHARE_ID_BYTES];
const LAPTOP: [u8; 32] = [1; 32];
const PHONE: [u8; 32] = [2; 32];
/// A device the owner never enrolled.
const IMPOSTOR: [u8; 32] = [9; 32];
const NOW: u64 = 1_700_000_000_000_000_000;

#[tokio::main]
async fn main() {
    let (laptop, phone) = (key(LAPTOP), key(PHONE));
    let (log, log_entries) = owner_log(&laptop, &phone);
    let store = MemoryChainStore::default();

    section("Three revisions");
    let first = revision(&laptop, 1, NO_PREVIOUS_REVISION, [0x22; 32], &[2, 3]);
    let second = revision(&laptop, 2, first.hash(), [0x33; 32], &[2, 3]);
    // The third removes a member, which is the change a rollback would undo.
    let third = revision(&phone, 3, second.hash(), [0x44; 32], &[2]);

    for candidate in [&first, &second, &third] {
        let accepted = verify_revision(candidate, &log, &store, Some(candidate.manifest_hash), &[])
            .await
            .expect("a revision from the owner's own device");
        println!(
            "  revision {} · {} member(s) · {}",
            accepted.state.number,
            candidate.members.len(),
            short(&accepted.state.revision_hash)
        );
    }
    println!("  The third is signed by the phone, not the laptop: any");
    println!("  unrevoked owner device may publish. Authority comes from the");
    println!("  device log, which is why that log is step 2 and this is step 3.");

    section("A rollback");
    // Nothing forged. The owner signed this, and it is simply old.
    refused(
        "the second revision, served again after the third",
        verify_revision(&second, &log, &store, None, &[]).await,
        "the member removed in revision 3 would silently regain access",
    );
    println!("  Detectable only against the held state. The revision itself is");
    println!("  flawless — it was flawless when it was current.");

    section("A fork");
    let rival = revision(&laptop, 3, second.hash(), [0x99; 32], &[2, 3]);
    assert!(rival.verify(), "the fork verifies on its own");
    println!("  A second revision 3, genuinely signed, naming the same parent.");
    println!("  It verifies on its own — both branches do.");
    match verify_revision(&rival, &log, &store, None, &[]).await {
        Err(ChainError::Fork {
            number,
            kept,
            refused,
        }) => {
            println!("\n  refused: two revisions numbered {number}");
            println!("    kept    {} — the first seen", short(&kept));
            println!("    refused {} — surfaced, not discarded", short(&refused));
            println!("    without it: two members would hold different histories");
            println!("    of the same collection and neither would know");
        }
        other => panic!("a fork must be reported as one, got {other:?}"),
    }
    println!("\n  A fork is never resolved silently. It means a compromised");
    println!("  owner device or a service splitting members' views, and");
    println!("  picking a winner is not a decision code can make correctly.");

    section("What the held state still says");
    let held = store
        .highest(COLLECTION)
        .await
        .expect("a healthy store")
        .expect("three revisions were accepted");
    println!(
        "  revision {} · {} — untouched by either attack",
        held.number,
        short(&held.revision_hash)
    );

    other_refusals(&laptop, &phone, &log, log_entries, third.hash(), &store).await;

    println!("\nEight refusals, each with its own reason. Nothing was forged in");
    println!("the two that matter most.");
}

/// Everything the chain refuses that is not a rollback or a fork. Grouped
/// because each is a one-liner: the interesting attacks are the two above.
async fn other_refusals(
    laptop: &SigningKey,
    phone: &SigningKey,
    log: &DeviceLog,
    log_entries: Vec<LogEntry>,
    parent: RevisionHash,
    store: &MemoryChainStore,
) {
    section("The rest of the chain's refusals");
    let impostor = key(IMPOSTOR);
    let mut forged = revision(laptop, 4, parent, [0x55; 32], &[2]);
    forged.signature = impostor.sign(&forged.signing_payload()).to_bytes();
    refused(
        "a revision signed by someone other than its author",
        verify_revision(&forged, log, store, None, &[]).await,
        "anyone could publish into anyone's collection",
    );

    let mut outsider = revision(laptop, 4, parent, [0x55; 32], &[2]);
    outsider.author_key = public(&impostor);
    let outsider = sign(outsider, &impostor);
    refused(
        "a revision from a device the owner never enrolled",
        verify_revision(&outsider, log, store, None, &[]).await,
        "the service could publish on the owner's behalf",
    );

    let revoked_log = revoke_the_phone(laptop, phone, log_entries);
    let by_revoked = revision(phone, 4, parent, [0x55; 32], &[2]);
    refused(
        "a revision from a device revoked after it was enrolled",
        verify_revision(&by_revoked, &revoked_log, store, None, &[]).await,
        "a stolen device would keep publishing after being removed",
    );

    let skipped = revision(laptop, 6, parent, [0x55; 32], &[2]);
    refused(
        "a revision that skips a number",
        verify_revision(&skipped, log, store, None, &[]).await,
        "a reader would never learn what happened in between",
    );

    let relinked = revision(laptop, 4, [7; 32], [0x55; 32], &[2]);
    refused(
        "a revision naming a parent that is not the one held",
        verify_revision(&relinked, log, store, None, &[]).await,
        "history could be rewritten without redoing everything after it",
    );

    let honest = revision(laptop, 4, parent, [0x55; 32], &[2]);
    refused(
        "a revision whose manifest is not the one fetched",
        verify_revision(&honest, log, store, Some([0xaa; 32]), &[]).await,
        "the signed list and the delivered list could differ",
    );

    section("Not a refusal");
    // A member who linked a device since the seal. The revision is fine; the
    // key needs sealing again.
    let accepted = verify_revision(
        &honest,
        log,
        store,
        Some(honest.manifest_hash),
        &[([2; DEVICE_KEY_BYTES], [0xff; 32])],
    )
    .await
    .expect("a valid revision");
    println!(
        "  revision {} accepted, and {} member(s) owed a re-seal",
        accepted.state.number,
        accepted.reseal_owed.len()
    );
    println!("  Their device log moved after the owner sealed to them, so a new");
    println!("  device of theirs opens nothing yet. That is a job to do, not a");
    println!("  lie to refuse — and knowing which is the difference between a");
    println!("  clear state and a mystery.");
}

fn refused<T: std::fmt::Debug>(what: &str, outcome: Result<T, ChainError>, why: &str) {
    let error = outcome.expect_err("this must be refused");
    println!("\n  {what}");
    println!("    refused: {error}");
    println!("    without it: {why}");
}

/// An owner with two devices: the laptop that started the log, and a phone.
fn owner_log(laptop: &SigningKey, phone: &SigningKey) -> (DeviceLog, Vec<LogEntry>) {
    let root = sign_entry(
        LogEntry {
            root_key: public(laptop),
            sequence: 1,
            previous_hash: NO_PREVIOUS_ENTRY,
            action: Action::Enrol,
            subject_signing_key: public(laptop),
            subject_encryption_key: [0x40; ENCRYPTION_KEY_BYTES],
            at_unix_ns: NOW,
            author_key: public(laptop),
            signature: [0; SIGNATURE_BYTES],
        },
        laptop,
    );
    let enrol = sign_entry(
        LogEntry {
            root_key: public(laptop),
            sequence: 2,
            previous_hash: root.hash(),
            action: Action::Enrol,
            subject_signing_key: public(phone),
            subject_encryption_key: [0x41; ENCRYPTION_KEY_BYTES],
            at_unix_ns: NOW + 1,
            author_key: public(laptop),
            signature: [0; SIGNATURE_BYTES],
        },
        laptop,
    );
    let entries = vec![root, enrol];
    (
        DeviceLog::replay(&entries).expect("the owner's own devices"),
        entries,
    )
}

fn revoke_the_phone(
    laptop: &SigningKey,
    phone: &SigningKey,
    mut entries: Vec<LogEntry>,
) -> DeviceLog {
    let revoke = sign_entry(
        LogEntry {
            root_key: public(laptop),
            sequence: 3,
            previous_hash: entries.last().expect("entries").hash(),
            action: Action::Revoke,
            subject_signing_key: public(phone),
            subject_encryption_key: [0; ENCRYPTION_KEY_BYTES],
            at_unix_ns: NOW + 2,
            author_key: public(laptop),
            signature: [0; SIGNATURE_BYTES],
        },
        laptop,
    );
    entries.push(revoke);
    DeviceLog::replay(&entries).expect("a revocation")
}

fn revision(
    author: &SigningKey,
    number: u64,
    previous: RevisionHash,
    manifest_hash: [u8; 32],
    members: &[u8],
) -> Revision {
    sign(
        Revision {
            collection_id: COLLECTION,
            number,
            previous_hash: previous,
            manifest_hash,
            owner_root_key: public(&key(LAPTOP)),
            at_unix_ns: NOW + number,
            members: members
                .iter()
                .map(|&root| Member {
                    root_key: [root; DEVICE_KEY_BYTES],
                    device_log_hash: [root.wrapping_add(0x80); 32],
                })
                .collect(),
            author_key: public(author),
            signature: [0; SIGNATURE_BYTES],
        },
        author,
    )
}

fn sign(mut revision: Revision, author: &SigningKey) -> Revision {
    revision.signature = author.sign(&revision.signing_payload()).to_bytes();
    revision
}

fn sign_entry(mut entry: LogEntry, author: &SigningKey) -> LogEntry {
    entry.signature = author.sign(&entry.signing_payload()).to_bytes();
    entry
}

fn key(seed: [u8; 32]) -> SigningKey {
    SigningKey::from_bytes(&seed)
}

fn public(signer: &SigningKey) -> [u8; DEVICE_KEY_BYTES] {
    signer.verifying_key().to_bytes()
}

fn short(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().take(6).fold(String::new(), |mut out, byte| {
        let _ = write!(out, "{byte:02x}");
        out
    })
}

fn section(title: &str) {
    println!("\n{title}\n{}", "─".repeat(title.chars().count()));
}
