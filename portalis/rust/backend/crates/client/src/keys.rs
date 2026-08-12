//! Sealing a collection's content key to the devices a verified log allows.
//!
//! This is `SPEC.md` §7.6, and it closes the loop steps 2 and 3 opened. A
//! device log says which devices a person has; a revision says who a
//! collection's members are. Neither is worth anything until the moment a key
//! is actually sealed, because that is the moment a mistake becomes a stolen
//! secret rather than a wrong display.
//!
//! The one attack this module exists to prevent: a service adds a device to
//! someone's list, the owner seals the content key to it, and the service
//! reads everything. Nothing downstream can detect it — the ciphertext is
//! valid, the recipient is real, the owner did the sealing. So the defence has
//! to be structural, and it is: there is no parameter here through which a
//! device outside a replayed log can enter. A recipient's encryption key and
//! device id both come from the log itself.
//!
//! A **stale** log is the same attack wearing different clothes. Yesterday's
//! log is genuine, signed, and still contains the device revoked this morning.
//! [`Recipient`] therefore cannot be built from a log directly — only by
//! adopting an offered one over a log already held, which is where
//! [`DeviceLog::adopt`] refuses anything behind or forked from what is known.
//! Getting this wrong is not possible rather than merely discouraged.
//!
//! Removing a member means rotating: a new key, sealed only to those who
//! remain. What a former member already holds cannot be recalled, so the point
//! is that the *next* revision is closed to them.

use portalis_nexus_protocol::{
    CONTENT_KEY_BYTES, ContentKey, DEVICE_ID_BYTES, DEVICE_KEY_BYTES, DeviceLog, DeviceLogError,
    EnvelopeContext, LogEntry, LogHash, Member, SHARE_ID_BYTES, SealError, SealedEnvelope,
    derive_device_id, new_challenge,
};
use thiserror::Error;

/// A recipient whose device log has been brought up to date.
///
/// The only way to make one is [`Self::current`], which adopts an offered log
/// over the one already held. That is deliberate: a log that is stale, forked,
/// or belonging to someone else cannot become a recipient, so sealing to a
/// device that was revoked in the meantime is not an error a caller can make.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipient {
    log: DeviceLog,
}

impl Recipient {
    /// Brings a held log up to date with what a peer or the service offered,
    /// and yields a recipient only if that succeeded.
    ///
    /// # Errors
    ///
    /// Returns [`KeyError::Log`] when the offered entries are invalid, behind
    /// the held log, or disagree with it — each of which is a reason not to
    /// seal anything to this person yet.
    pub fn current(held: &DeviceLog, offered: &[LogEntry]) -> Result<Self, KeyError> {
        Ok(Self {
            log: held.adopt(offered)?,
        })
    }

    /// Who this is.
    #[must_use]
    pub const fn root_key(&self) -> [u8; DEVICE_KEY_BYTES] {
        self.log.root_key()
    }

    /// The log state a revision records for this member, so a contact who
    /// links a device later can see that a re-seal is owed.
    #[must_use]
    pub const fn log_hash(&self) -> LogHash {
        self.log.hash()
    }

    /// The devices a key may be sealed to. Never a device the log revoked.
    #[must_use]
    pub fn authorized(&self) -> Vec<portalis_nexus_protocol::Device> {
        self.log.authorized()
    }
}

/// One sealed copy of a content key, addressed to one device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SealedFor {
    pub member_root_key: [u8; DEVICE_KEY_BYTES],
    pub recipient_device_id: [u8; DEVICE_ID_BYTES],
    pub envelope: SealedEnvelope,
}

/// Everything one sealing produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sealing {
    /// One envelope per authorized device across every recipient.
    pub envelopes: Vec<SealedFor>,
    /// The membership to put in the revision, ascending by root key.
    ///
    /// Returned rather than accepted so the revision cannot claim to have
    /// sealed against a log state the sealing did not use. The same reason a
    /// sealed manifest takes its content hash from the manifest.
    pub members: Vec<Member>,
}

/// Why a content key was not sealed or opened.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum KeyError {
    /// The offered log is invalid, stale, or forked. Reported before any
    /// sealing happens, because a log that cannot be trusted names devices
    /// that cannot be sealed to.
    #[error(transparent)]
    Log(#[from] DeviceLogError),
    #[error(transparent)]
    Seal(#[from] SealError),
    /// A recipient whose every device has been revoked. Sealing to nobody
    /// silently would look like success and deliver nothing.
    #[error("this member has no authorized device to seal to")]
    NoAuthorizedDevice,
    /// The same person twice, which would put two entries in a revision's
    /// membership and make its encoding non-canonical.
    #[error("the same member appears twice among the recipients")]
    DuplicateMember,
    /// What came out of the envelope is not a content key.
    #[error("the envelope opened but does not hold a {CONTENT_KEY_BYTES}-byte content key")]
    NotAContentKey { actual: usize },
}

/// A fresh content key for one collection.
///
/// One per collection, and a new one on every rotation: reusing a key across
/// revisions would mean a removed member could still read the next one.
#[must_use]
pub fn generate_content_key() -> ContentKey {
    let mut key = [0_u8; CONTENT_KEY_BYTES];
    key.copy_from_slice(&new_challenge()[..CONTENT_KEY_BYTES]);
    key
}

/// Seals `key` to every device the recipients' logs authorize.
///
/// # Errors
///
/// Returns [`KeyError`] when a recipient appears twice, has no authorized
/// device, or its encryption key is one X25519 refuses.
pub fn seal_content_key(
    key: &ContentKey,
    collection_id: [u8; SHARE_ID_BYTES],
    recipients: &[Recipient],
) -> Result<Sealing, KeyError> {
    let mut members: Vec<Member> = Vec::with_capacity(recipients.len());
    let mut envelopes = Vec::new();

    for recipient in recipients {
        let root_key = recipient.root_key();
        if members.iter().any(|member| member.root_key == root_key) {
            return Err(KeyError::DuplicateMember);
        }

        let devices = recipient.authorized();
        if devices.is_empty() {
            return Err(KeyError::NoAuthorizedDevice);
        }
        for device in devices {
            // Both the address and the key come from the log. Nothing a
            // caller passes can redirect an envelope.
            let recipient_device_id = derive_device_id(&device.signing_key);
            let context = EnvelopeContext {
                share_id: collection_id,
                recipient_device_id,
            };
            envelopes.push(SealedFor {
                member_root_key: root_key,
                recipient_device_id,
                envelope: portalis_nexus_protocol::seal_envelope(
                    &device.encryption_key,
                    &context,
                    key,
                )?,
            });
        }

        members.push(Member {
            root_key,
            device_log_hash: recipient.log_hash(),
        });
    }

    // Ascending, because a revision's membership has one canonical encoding.
    members.sort_unstable_by_key(|member| member.root_key);
    Ok(Sealing { envelopes, members })
}

/// Rotates a collection's content key and seals the new one to whoever remains.
///
/// This is what removing a member means. It cannot reach what they already
/// hold — no protocol can — so what it achieves is that the next revision is
/// closed to them.
///
/// # Errors
///
/// Returns [`KeyError`] for the same reasons [`seal_content_key`] does.
pub fn rotate_content_key(
    collection_id: [u8; SHARE_ID_BYTES],
    remaining: &[Recipient],
) -> Result<(ContentKey, Sealing), KeyError> {
    let key = generate_content_key();
    let sealing = seal_content_key(&key, collection_id, remaining)?;
    Ok((key, sealing))
}

/// Opens a content key sealed to this device.
///
/// # Errors
///
/// Returns [`KeyError`] when the envelope was not sealed to this device and
/// collection, or holds something that is not a content key.
pub fn open_content_key(
    device_secret_key: &[u8; DEVICE_ID_BYTES],
    collection_id: [u8; SHARE_ID_BYTES],
    recipient_device_id: [u8; DEVICE_ID_BYTES],
    envelope: &SealedEnvelope,
) -> Result<ContentKey, KeyError> {
    let context = EnvelopeContext {
        share_id: collection_id,
        recipient_device_id,
    };
    let plaintext = portalis_nexus_protocol::open_envelope(device_secret_key, &context, envelope)?;
    ContentKey::try_from(plaintext.as_slice()).map_err(|_| KeyError::NotAContentKey {
        actual: plaintext.len(),
    })
}

#[cfg(test)]
mod tests {
    //! The gate for this step is that a key never reaches a device outside the
    //! verified log, including when the log offered is stale or forged. Most
    //! of that is enforced by the type: [`Recipient`] cannot be built from an
    //! untrusted log. What follows proves the enforcement is real rather than
    //! decorative, and that a rotation actually closes the next revision.

    use ed25519_dalek::{Signer, SigningKey};
    use portalis_nexus_protocol::{
        Action, ENCRYPTION_KEY_BYTES, NO_PREVIOUS_ENTRY, SIGNATURE_BYTES,
    };
    use x25519_dalek::{PublicKey, StaticSecret};

    use super::*;

    const COLLECTION: [u8; SHARE_ID_BYTES] = [0x11; SHARE_ID_BYTES];
    const NOW: u64 = 1_700_000_000_000_000_000;

    /// One device: an Ed25519 identity and the X25519 pair that receives keys.
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

        fn secret_bytes(&self) -> [u8; DEVICE_ID_BYTES] {
            self.secret.to_bytes()
        }
    }

    fn entry(
        root: &Device,
        sequence: u64,
        previous: LogHash,
        action: Action,
        subject: &Device,
        author: &Device,
    ) -> LogEntry {
        let mut entry = LogEntry {
            root_key: root.signing_key(),
            sequence,
            previous_hash: previous,
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

    fn root_entry(root: &Device) -> LogEntry {
        entry(root, 1, NO_PREVIOUS_ENTRY, Action::Enrol, root, root)
    }

    /// A person with two devices.
    fn two_devices(root: &Device, second: &Device) -> Vec<LogEntry> {
        let first = root_entry(root);
        let enrol = entry(root, 2, first.hash(), Action::Enrol, second, root);
        vec![first, enrol]
    }

    fn recipient(entries: &[LogEntry]) -> Recipient {
        let log = DeviceLog::replay(entries).expect("a valid log");
        Recipient::current(&log, entries).expect("a recipient at its own log")
    }

    #[test]
    fn a_key_is_sealed_to_every_authorized_device_and_opens_on_each() {
        let (laptop, phone) = (Device::new(1), Device::new(2));
        let entries = two_devices(&laptop, &phone);
        let key = generate_content_key();

        let sealing =
            seal_content_key(&key, COLLECTION, &[recipient(&entries)]).expect("seals to the owner");

        assert_eq!(sealing.envelopes.len(), 2, "one per authorized device");
        assert_eq!(sealing.members.len(), 1, "one member, with two devices");
        assert_eq!(sealing.members[0].root_key, laptop.signing_key());

        for device in [&laptop, &phone] {
            let envelope = sealing
                .envelopes
                .iter()
                .find(|sealed| sealed.recipient_device_id == device.device_id())
                .expect("an envelope for this device");
            assert_eq!(
                open_content_key(
                    &device.secret_bytes(),
                    COLLECTION,
                    device.device_id(),
                    &envelope.envelope,
                )
                .expect("opens on the device it was sealed to"),
                key
            );
        }
    }

    /// The whole point: a device the log does not authorize gets nothing, and
    /// there is no argument through which it could.
    #[test]
    fn a_device_outside_the_log_receives_nothing_and_cannot_open_what_it_intercepts() {
        let (laptop, phone, outsider) = (Device::new(1), Device::new(2), Device::new(9));
        let entries = two_devices(&laptop, &phone);
        let key = generate_content_key();

        let sealing = seal_content_key(&key, COLLECTION, &[recipient(&entries)]).expect("seals");

        assert!(
            sealing
                .envelopes
                .iter()
                .all(|sealed| sealed.recipient_device_id != outsider.device_id()),
            "no envelope was addressed to a device the log never enrolled"
        );
        // Intercepting one and trying it is refused by the cryptography, not
        // only by the addressing.
        assert!(matches!(
            open_content_key(
                &outsider.secret_bytes(),
                COLLECTION,
                sealing.envelopes[0].recipient_device_id,
                &sealing.envelopes[0].envelope,
            ),
            Err(KeyError::Seal(_))
        ));
    }

    #[test]
    fn a_revoked_device_is_not_sealed_to() {
        let (laptop, phone) = (Device::new(1), Device::new(2));
        let mut entries = two_devices(&laptop, &phone);
        entries.push(entry(
            &laptop,
            3,
            entries.last().expect("entries").hash(),
            Action::Revoke,
            &phone,
            &laptop,
        ));
        let key = generate_content_key();

        let sealing = seal_content_key(&key, COLLECTION, &[recipient(&entries)]).expect("seals");

        assert_eq!(sealing.envelopes.len(), 1);
        assert_eq!(
            sealing.envelopes[0].recipient_device_id,
            laptop.device_id(),
            "only the device that remains"
        );
    }

    /// A stale log is genuine, signed, and still holds the device revoked this
    /// morning. It cannot become a recipient, so it cannot be sealed to.
    #[test]
    fn a_stale_log_cannot_become_a_recipient() {
        let (laptop, phone) = (Device::new(1), Device::new(2));
        let yesterday = two_devices(&laptop, &phone);
        let mut today = yesterday.clone();
        today.push(entry(
            &laptop,
            3,
            yesterday.last().expect("entries").hash(),
            Action::Revoke,
            &phone,
            &laptop,
        ));
        let current = DeviceLog::replay(&today).expect("today's log");

        assert_eq!(
            Recipient::current(&current, &yesterday),
            Err(KeyError::Log(DeviceLogError::StaleLog {
                held: 3,
                offered: 2
            }))
        );
    }

    #[test]
    fn a_forged_or_forked_log_cannot_become_a_recipient() {
        let (laptop, phone, impostor) = (Device::new(1), Device::new(2), Device::new(9));
        let entries = two_devices(&laptop, &phone);
        let held = DeviceLog::replay(&entries).expect("a valid log");

        // Signed by a device the log never enrolled.
        let mut forged = entries.clone();
        forged.push(entry(
            &laptop,
            3,
            entries.last().expect("entries").hash(),
            Action::Enrol,
            &impostor,
            &impostor,
        ));
        assert_eq!(
            Recipient::current(&held, &forged),
            Err(KeyError::Log(DeviceLogError::UnknownAuthor { sequence: 3 }))
        );

        // Valid on its own, and disagreeing with what is held.
        let rival = vec![
            entries[0],
            entry(
                &laptop,
                2,
                entries[0].hash(),
                Action::Enrol,
                &impostor,
                &laptop,
            ),
        ];
        assert_eq!(
            Recipient::current(&held, &rival),
            Err(KeyError::Log(DeviceLogError::ForkedLog { sequence: 2 }))
        );
    }

    #[test]
    fn rotation_closes_the_next_revision_to_a_removed_member() {
        let (owner, member, removed) = (Device::new(1), Device::new(2), Device::new(3));
        let owner_entries = two_devices(&owner, &Device::new(4));
        let member_entries = vec![root_entry(&member)];
        let removed_entries = vec![root_entry(&removed)];

        let first = generate_content_key();
        let before = seal_content_key(
            &first,
            COLLECTION,
            &[
                recipient(&owner_entries),
                recipient(&member_entries),
                recipient(&removed_entries),
            ],
        )
        .expect("seals to everyone");
        assert_eq!(before.members.len(), 3);

        // The removed member could read the revision they were a member of,
        // and nothing can take that back.
        let their_envelope = before
            .envelopes
            .iter()
            .find(|sealed| sealed.recipient_device_id == removed.device_id())
            .expect("they were sealed to");
        assert_eq!(
            open_content_key(
                &removed.secret_bytes(),
                COLLECTION,
                removed.device_id(),
                &their_envelope.envelope,
            )
            .expect("what they already hold still opens"),
            first
        );

        let (second, after) = rotate_content_key(
            COLLECTION,
            &[recipient(&owner_entries), recipient(&member_entries)],
        )
        .expect("rotates to those who remain");

        assert_ne!(second, first, "a rotation is a new key, not a reshuffle");
        assert_eq!(after.members.len(), 2);
        assert!(
            after
                .envelopes
                .iter()
                .all(|sealed| sealed.recipient_device_id != removed.device_id()),
            "nothing in the next revision is addressed to them"
        );
        assert!(
            !after
                .members
                .iter()
                .any(|m| m.root_key == removed.signing_key()),
            "and the revision does not list them"
        );
    }

    #[test]
    fn membership_is_canonical_and_records_what_was_sealed_against() {
        let (owner, high, low) = (Device::new(1), Device::new(9), Device::new(2));
        let owner_entries = vec![root_entry(&owner)];
        let high_entries = vec![root_entry(&high)];
        let low_entries = vec![root_entry(&low)];
        let key = generate_content_key();

        // Offered out of order on purpose.
        let sealing = seal_content_key(
            &key,
            COLLECTION,
            &[
                recipient(&high_entries),
                recipient(&owner_entries),
                recipient(&low_entries),
            ],
        )
        .expect("seals");

        let ordered: Vec<_> = sealing.members.iter().map(|m| m.root_key).collect();
        let mut expected = ordered.clone();
        expected.sort_unstable();
        assert_eq!(
            ordered, expected,
            "a revision's membership has one encoding"
        );

        // Each member's recorded log hash is the log actually sealed against,
        // which is what lets a contact see a re-seal is owed.
        for entries in [&owner_entries, &high_entries, &low_entries] {
            let log = DeviceLog::replay(entries).expect("a valid log");
            assert!(
                sealing
                    .members
                    .iter()
                    .any(|m| m.root_key == log.root_key() && m.device_log_hash == log.hash())
            );
        }
    }

    #[test]
    fn sealing_refuses_a_member_twice_and_a_member_with_no_device() {
        let (owner, phone) = (Device::new(1), Device::new(2));
        let entries = two_devices(&owner, &phone);
        let key = generate_content_key();

        assert_eq!(
            seal_content_key(
                &key,
                COLLECTION,
                &[recipient(&entries), recipient(&entries)]
            ),
            Err(KeyError::DuplicateMember)
        );

        // Every device revoked, including the root: nobody left to seal to,
        // and reporting nothing sealed as success would deliver silence.
        let mut abandoned = entries.clone();
        abandoned.push(entry(
            &owner,
            3,
            entries.last().expect("entries").hash(),
            Action::Revoke,
            &phone,
            &owner,
        ));
        abandoned.push(entry(
            &owner,
            4,
            abandoned.last().expect("entries").hash(),
            Action::Revoke,
            &owner,
            &owner,
        ));
        assert_eq!(
            seal_content_key(&key, COLLECTION, &[recipient(&abandoned)]),
            Err(KeyError::NoAuthorizedDevice)
        );
    }

    #[test]
    fn an_envelope_is_bound_to_its_collection_and_its_device() {
        let (laptop, phone) = (Device::new(1), Device::new(2));
        let entries = two_devices(&laptop, &phone);
        let key = generate_content_key();
        let sealing = seal_content_key(&key, COLLECTION, &[recipient(&entries)]).expect("seals");
        let mine = sealing
            .envelopes
            .iter()
            .find(|sealed| sealed.recipient_device_id == laptop.device_id())
            .expect("an envelope for the laptop");

        assert!(matches!(
            open_content_key(
                &laptop.secret_bytes(),
                [0x99; SHARE_ID_BYTES],
                laptop.device_id(),
                &mine.envelope,
            ),
            Err(KeyError::Seal(_))
        ));
        assert!(
            matches!(
                open_content_key(
                    &laptop.secret_bytes(),
                    COLLECTION,
                    phone.device_id(),
                    &mine.envelope,
                ),
                Err(KeyError::Seal(_))
            ),
            "the device id is authenticated, so an envelope cannot be re-addressed"
        );
    }

    #[test]
    fn an_envelope_holding_something_other_than_a_key_is_refused() {
        let laptop = Device::new(1);
        let context = EnvelopeContext {
            share_id: COLLECTION,
            recipient_device_id: laptop.device_id(),
        };
        let envelope =
            portalis_nexus_protocol::seal_envelope(&laptop.encryption_key(), &context, b"short")
                .expect("seals");

        assert_eq!(
            open_content_key(
                &laptop.secret_bytes(),
                COLLECTION,
                laptop.device_id(),
                &envelope,
            ),
            Err(KeyError::NotAContentKey { actual: 5 })
        );
    }

    #[test]
    fn every_generated_key_is_a_different_key() {
        let first = generate_content_key();

        assert_ne!(first, generate_content_key());
        assert_eq!(first.len(), CONTENT_KEY_BYTES);
    }
}
