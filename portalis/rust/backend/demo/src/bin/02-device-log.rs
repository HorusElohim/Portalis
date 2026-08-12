//! Step 2 — the device log: enrol, revoke, replay, and six attacks refused.
//!
//! Decision D2 says a person is a signed append-only device log, not an
//! account row held by the service. The reason is one attack. If the service
//! holds the list, it can add a device to it, and an owner about to seal a
//! content key will seal to that device: the key is taken and nothing
//! downstream can tell. A log makes that impossible rather than auditable —
//! extending it needs a key already inside it.
//!
//! So this binary spends most of its output on refusals. The happy path is
//! three lines; what matters is that six different lies each get their own
//! answer, because "invalid log" would let two different bugs look alike.
//!
//! Run with `cargo run -p portalis-nexus-demo --bin 02-device-log`.

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_protocol::{
    Action, DEVICE_KEY_BYTES, DeviceLog, DeviceLogError, ENCRYPTION_KEY_BYTES, LogEntry,
    NO_PREVIOUS, SIGNATURE_BYTES,
};

const LAPTOP: [u8; 32] = [1; 32];
const PHONE: [u8; 32] = [2; 32];
const TABLET: [u8; 32] = [3; 32];
/// A device the owner never enrolled. Everything it tries must fail.
const IMPOSTOR: [u8; 32] = [9; 32];
const NOW: u64 = 1_700_000_000_000_000_000;

fn main() {
    let (laptop, phone, tablet) = (key(LAPTOP), key(PHONE), key(TABLET));

    section("A log, built by its own devices");
    let root = root_entry(&laptop);
    let add_phone = next(&root, Action::Enrol, &phone, PHONE, &laptop);
    let add_tablet = next(&add_phone, Action::Enrol, &tablet, TABLET, &laptop);
    let entries = vec![root, add_phone, add_tablet];

    let log = DeviceLog::replay(&entries).expect("the owner's own devices");
    report(&log);
    println!("  Every entry after the first is signed by a device already");
    println!("  inside the log. That is the whole rule.");

    section("Revoking the tablet");
    let revoke = next(&add_tablet, Action::Revoke, &tablet, TABLET, &phone);
    let revoked_entries = [entries.clone(), vec![revoke]].concat();
    let revoked = DeviceLog::replay(&revoked_entries).expect("a revocation");
    report(&revoked);
    println!("  Signed by the phone, not the laptop: any enrolled device may");
    println!("  revoke another. Authority is the log's, not one device's.");
    println!("  The tablet remains in the history — a rotation needs to know");
    println!("  it once could read, not merely that it cannot now.");

    section("Six attacks");

    // 1. The attack the whole design exists to stop.
    let impostor = key(IMPOSTOR);
    let injected = next(&add_tablet, Action::Enrol, &impostor, IMPOSTOR, &impostor);
    refused(
        "a service invents a device and signs for it",
        DeviceLog::replay(&[entries.clone(), vec![injected]].concat()),
        "the key would have been sealed to a device the owner never had",
    );

    // 2. Genuinely signed, by a device whose authority had ended.
    let after_revocation = next(&revoke, Action::Enrol, &impostor, IMPOSTOR, &tablet);
    refused(
        "a revoked device enrols a new one",
        DeviceLog::replay(&[revoked_entries.clone(), vec![after_revocation]].concat()),
        "a stolen laptop could otherwise re-admit itself under a new key",
    );

    // 3. The signature is real, the author it claims is not the signer.
    let mut forged = next(&add_tablet, Action::Enrol, &impostor, IMPOSTOR, &laptop);
    forged.signature = impostor.sign(&forged.signing_payload()).to_bytes();
    refused(
        "an entry claims the laptop as author but is signed by someone else",
        DeviceLog::replay(&[entries.clone(), vec![forged]].concat()),
        "claiming authorship is not the same as having the key",
    );

    // 4. Every entry valid on its own, the order changed.
    let mut reordered = entries.clone();
    reordered.swap(1, 2);
    refused(
        "the entries are reordered",
        DeviceLog::replay(&reordered),
        "order is what decides who was authorized when",
    );

    // 5. A log that begins again is a log with a second beginning.
    let second_root = root_entry(&phone);
    refused(
        "a second root is appended",
        DeviceLog::replay(&[entries.clone(), vec![second_root]].concat()),
        "two roots would mean two people, or one person overwritten",
    );

    // 6. Nothing forged: an older, genuine log served in place of the current
    //    one. This is how a service undoes a revocation without forging
    //    anything at all, and why holding the highest verified state matters.
    refused(
        "an older log is served in place of the current one",
        revoked.adopt(&entries),
        "the tablet's revocation would be undone by a log that is entirely genuine",
    );

    section("A fork is not an update");
    // Same root, same length, different history: internally valid, which is
    // exactly what makes it dangerous.
    let rival_third = next(&add_phone, Action::Enrol, &impostor, IMPOSTOR, &laptop);
    let rival = vec![root, add_phone, rival_third];
    assert!(
        DeviceLog::replay(&rival).is_ok(),
        "the fork verifies on its own"
    );
    println!("  The forked log is internally valid — it verifies on its own.");
    refused(
        "a log that disagrees about what already happened",
        log.adopt(&rival),
        "a service splitting two contacts' view of the same person",
    );
    println!("  Detected by comparing against the state already verified, not");
    println!("  by checking the log against itself. Nothing about a fork is");
    println!("  wrong in isolation, which is why it is never resolved silently.");

    println!("\nSix attacks, six distinct reasons. No log was extended without");
    println!("a key that was already inside it.");
}

fn report(log: &DeviceLog) {
    println!(
        "  sequence {} · hash {}",
        log.sequence(),
        short(&log.hash())
    );
    for device in log.history() {
        let state = match device.revoked_at_unix_ns {
            None => "authorized".to_owned(),
            Some(at) => format!("revoked at {}", at - NOW),
        };
        println!("    {} — {state}", short(&device.signing_key));
    }
}

fn refused(what: &str, outcome: Result<DeviceLog, DeviceLogError>, why_it_matters: &str) {
    let error = outcome.expect_err("this attack must be refused");
    println!("\n  {what}");
    println!("    refused: {error}");
    println!("    without it: {why_it_matters}");
}

fn key(seed: [u8; 32]) -> SigningKey {
    SigningKey::from_bytes(&seed)
}

fn public(signer: &SigningKey) -> [u8; DEVICE_KEY_BYTES] {
    signer.verifying_key().to_bytes()
}

/// A device's X25519 key. Its value is arbitrary here; that it survives
/// replay, so an owner can seal to it, is not.
fn encryption_key(seed: [u8; 32]) -> [u8; ENCRYPTION_KEY_BYTES] {
    [seed[0].wrapping_add(0x40); ENCRYPTION_KEY_BYTES]
}

fn root_entry(root: &SigningKey) -> LogEntry {
    sign(
        LogEntry {
            root_key: public(root),
            sequence: 1,
            previous_hash: NO_PREVIOUS,
            action: Action::Enrol,
            subject_signing_key: public(root),
            subject_encryption_key: encryption_key(root.to_bytes()),
            at_unix_ns: NOW,
            author_key: public(root),
            signature: [0; SIGNATURE_BYTES],
        },
        root,
    )
}

fn next(
    previous: &LogEntry,
    action: Action,
    subject: &SigningKey,
    subject_seed: [u8; 32],
    author: &SigningKey,
) -> LogEntry {
    sign(
        LogEntry {
            root_key: previous.root_key,
            sequence: previous.sequence + 1,
            previous_hash: previous.hash(),
            action,
            subject_signing_key: public(subject),
            subject_encryption_key: match action {
                Action::Enrol => encryption_key(subject_seed),
                Action::Revoke => [0; ENCRYPTION_KEY_BYTES],
            },
            at_unix_ns: previous.at_unix_ns + 1,
            author_key: public(author),
            signature: [0; SIGNATURE_BYTES],
        },
        author,
    )
}

fn sign(mut entry: LogEntry, author: &SigningKey) -> LogEntry {
    entry.signature = author.sign(&entry.signing_payload()).to_bytes();
    entry
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
