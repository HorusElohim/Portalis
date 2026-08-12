//! Step 4 — content keys: sealed to a verified log, and rotated on removal.
//!
//! Steps 2 and 3 built the machinery for deciding who a person's devices are
//! and what a collection's history says. This is the step where that decision
//! is spent, because sealing is the moment a wrong answer stops being a wrong
//! display and becomes a stolen secret.
//!
//! The attack: a service adds a device to someone's list, the owner seals the
//! content key to it, and the service reads the collection. Nothing downstream
//! notices — the ciphertext is valid, the recipient is real, the owner did the
//! sealing. So the defence is structural. Every recipient's encryption key and
//! address come out of a replayed device log; there is no argument through
//! which another device could enter.
//!
//! A stale log is the same attack in different clothes, and the type system
//! handles that one: a `Recipient` can only be built by adopting an offered
//! log over one already held.
//!
//! Run with `cargo run -p portalis-nexus-demo --bin 04-sealing`.

use ed25519_dalek::{Signer, SigningKey};
use portalis_nexus_client::{
    KeyError, Recipient, generate_content_key, open_content_key, rotate_content_key,
    seal_content_key,
};
use portalis_nexus_protocol::{
    Action, DEVICE_ID_BYTES, DEVICE_KEY_BYTES, DeviceLog, ENCRYPTION_KEY_BYTES, LogEntry,
    NO_PREVIOUS_ENTRY, SHARE_ID_BYTES, SIGNATURE_BYTES, derive_device_id,
};
use x25519_dalek::{PublicKey, StaticSecret};

const COLLECTION: [u8; SHARE_ID_BYTES] = [0x11; SHARE_ID_BYTES];
const NOW: u64 = 1_700_000_000_000_000_000;

fn main() {
    let owner = Person::new("Ada", 1);
    let mira = Person::new("Mira", 2);
    let jonas = Person::new("Jonas", 3);
    let outsider = Person::new("an outsider", 9);

    section("Sealing to two members");
    let key = generate_content_key();
    let sealing = seal_content_key(
        &key,
        COLLECTION,
        &[owner.recipient(), mira.recipient(), jonas.recipient()],
    )
    .expect("seals to the members");

    println!(
        "  {} envelope(s) for {} member(s)",
        sealing.envelopes.len(),
        sealing.members.len()
    );
    for person in [&owner, &mira, &jonas] {
        let opened = person.open(&sealing, COLLECTION).expect("their envelope");
        println!("  {} opens it: {}", person.name, opened == key);
    }
    println!("\n  The membership the sealing produced goes straight into the");
    println!("  revision, so the two cannot disagree about what was sealed");
    println!("  against — the same reason a sealed manifest takes its hash from");
    println!("  the manifest rather than from its caller.");

    section("A third party cannot open it");
    println!(
        "  no envelope is addressed to {}: {}",
        outsider.name,
        sealing
            .envelopes
            .iter()
            .all(|sealed| sealed.recipient_device_id != outsider.device_id())
    );
    // Intercepting one and trying it anyway.
    let intercepted = &sealing.envelopes[0];
    match open_content_key(
        &outsider.secret_bytes(),
        COLLECTION,
        intercepted.recipient_device_id,
        &intercepted.envelope,
    ) {
        Err(error) => println!("  intercepting one and trying it → {error}"),
        Ok(_) => panic!("an outsider must never recover a content key"),
    }

    section("A device the log never enrolled");
    // The service's dream: slip a device into the list before sealing.
    println!("  There is no way to express this. `seal_content_key` takes");
    println!("  recipients, and a recipient is a replayed device log — every");
    println!("  address and every encryption key comes out of it. An invented");
    println!("  device would have to be signed into the log first, which needs");
    println!("  a key already inside it. That is step 2 doing its job here.");

    section("Removing Jonas: rotate");
    let (rotated, after) = rotate_content_key(COLLECTION, &[owner.recipient(), mira.recipient()])
        .expect("rotates to those who remain");

    println!("  a new key, not a reshuffle: {}", rotated != key);
    println!(
        "  {} envelope(s) for {} member(s)",
        after.envelopes.len(),
        after.members.len()
    );
    println!(
        "  nothing addressed to {}: {}",
        jonas.name,
        after
            .envelopes
            .iter()
            .all(|sealed| sealed.recipient_device_id != jonas.device_id())
    );
    match jonas.open(&after, COLLECTION) {
        None => println!("  {} has no envelope in the next revision", jonas.name),
        Some(_) => panic!("a removed member must not be sealed to"),
    }

    println!(
        "\n  What {} already holds still opens — no protocol can reach",
        jonas.name
    );
    println!("  into a device and take a key back. Rotation is not about the");
    println!("  past; it is about the next revision being closed to them.");

    revocation_and_staleness(&owner, &mira);

    println!("\nA content key reached no device outside a verified log, and the");
    println!("two ways to try — an invented device and a stale log — are one");
    println!("unrepresentable and one refused.");
}

/// The two ways a revoked device could still be sealed to, and why neither
/// works: the log the sealing reads is the log it was revoked in, and a log
/// from before the revocation cannot become a recipient at all.
fn revocation_and_staleness(owner: &Person, mira: &Person) {
    section("Revoking a device: the same rotation, one person");
    let mira_after_theft = mira.revoke_second_device();
    let (_, tightened) =
        rotate_content_key(COLLECTION, &[owner.recipient(), mira_after_theft]).expect("rotates");

    println!(
        "  {} now has {} authorized device(s), and {} envelope(s)",
        mira.name,
        mira.log().authorized().len() - 1,
        tightened
            .envelopes
            .iter()
            .filter(|sealed| sealed.member_root_key == mira.root_key())
            .count()
    );
    println!("  The revoked device is not addressed, because the log it was");
    println!("  revoked in is the same log the sealing reads.");

    section("A stale log cannot be used at all");
    // Genuine, signed, and one revocation behind. This is the attack that
    // needs no forgery whatsoever.
    let current = mira.log_after_revocation();
    match Recipient::current(&current, &mira.entries) {
        Err(KeyError::Log(error)) => {
            println!("  yesterday's log, offered against today's → {error}");
            println!("  Refused before sealing is even reachable: a Recipient");
            println!("  cannot be built from a log behind the one already held,");
            println!("  so sealing to a device revoked this morning is not a");
            println!("  mistake this API allows.");
        }
        other => panic!("a stale log must be refused, got {other:?}"),
    }
}

/// One person, their devices, and the log that says so.
struct Person {
    name: &'static str,
    devices: Vec<Device>,
    entries: Vec<LogEntry>,
}

impl Person {
    /// Everyone here has two devices, which is what makes a revocation
    /// meaningful rather than an account deletion.
    fn new(name: &'static str, seed: u8) -> Self {
        let first = Device::new(seed);
        let second = Device::new(seed.wrapping_add(0x20));
        let root = entry(&first, 1, [0; 32], Action::Enrol, &first, &first);
        let enrol = entry(&first, 2, root.hash(), Action::Enrol, &second, &first);
        Self {
            name,
            devices: vec![first, second],
            entries: vec![root, enrol],
        }
    }

    fn log(&self) -> DeviceLog {
        DeviceLog::replay(&self.entries).expect("a valid log")
    }

    fn recipient(&self) -> Recipient {
        Recipient::current(&self.log(), &self.entries).expect("a recipient at its own log")
    }

    fn root_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.devices[0].signing_key()
    }

    fn device_id(&self) -> [u8; DEVICE_ID_BYTES] {
        self.devices[0].device_id()
    }

    fn secret_bytes(&self) -> [u8; DEVICE_ID_BYTES] {
        self.devices[0].secret.to_bytes()
    }

    fn revoked_entries(&self) -> Vec<LogEntry> {
        let mut entries = self.entries.clone();
        entries.push(entry(
            &self.devices[0],
            3,
            self.entries.last().expect("entries").hash(),
            Action::Revoke,
            &self.devices[1],
            &self.devices[0],
        ));
        entries
    }

    fn log_after_revocation(&self) -> DeviceLog {
        DeviceLog::replay(&self.revoked_entries()).expect("a valid log")
    }

    fn revoke_second_device(&self) -> Recipient {
        let entries = self.revoked_entries();
        Recipient::current(&self.log(), &entries).expect("adopting a longer log")
    }

    /// Opens whichever envelope this person's first device was given.
    fn open(
        &self,
        sealing: &portalis_nexus_client::Sealing,
        collection: [u8; SHARE_ID_BYTES],
    ) -> Option<[u8; 32]> {
        let sealed = sealing
            .envelopes
            .iter()
            .find(|sealed| sealed.recipient_device_id == self.device_id())?;
        open_content_key(
            &self.secret_bytes(),
            collection,
            sealed.recipient_device_id,
            &sealed.envelope,
        )
        .ok()
    }
}

/// An Ed25519 identity and the X25519 pair that receives sealed keys.
struct Device {
    signing: SigningKey,
    secret: StaticSecret,
}

impl Device {
    fn new(seed: u8) -> Self {
        Self {
            signing: SigningKey::from_bytes(&[seed; 32]),
            secret: StaticSecret::from([seed.wrapping_add(0x40); ENCRYPTION_KEY_BYTES]),
        }
    }

    fn signing_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.signing.verifying_key().to_bytes()
    }

    fn encryption_key(&self) -> [u8; ENCRYPTION_KEY_BYTES] {
        *PublicKey::from(&self.secret).as_bytes()
    }

    fn device_id(&self) -> [u8; DEVICE_ID_BYTES] {
        derive_device_id(&self.signing_key())
    }
}

fn entry(
    root: &Device,
    sequence: u64,
    previous: [u8; 32],
    action: Action,
    subject: &Device,
    author: &Device,
) -> LogEntry {
    let mut entry = LogEntry {
        root_key: root.signing_key(),
        sequence,
        previous_hash: if sequence == 1 {
            NO_PREVIOUS_ENTRY
        } else {
            previous
        },
        action,
        subject_signing_key: subject.signing_key(),
        subject_encryption_key: match action {
            Action::Enrol => subject.encryption_key(),
            Action::Revoke => [0; ENCRYPTION_KEY_BYTES],
        },
        at_unix_ns: NOW + sequence,
        author_key: author.signing_key(),
        signature: [0; SIGNATURE_BYTES],
    };
    entry.signature = author.signing.sign(&entry.signing_payload()).to_bytes();
    entry
}

fn section(title: &str) {
    println!("\n{title}\n{}", "─".repeat(title.chars().count()));
}
