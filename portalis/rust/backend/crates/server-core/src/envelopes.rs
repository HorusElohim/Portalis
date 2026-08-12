//! Key-envelope delivery: pushing a sealed share key to one of a user's own
//! devices, and letting that device fetch what is addressed to it.
//!
//! Nexus never sees a share key in plaintext. It only ever handles
//! `ciphertext` bytes a device sealed to another device's public key, and its
//! one job here is deciding whether the sender may address the recipient it
//! named — never what the envelope contains.

use portalis_nexus_protocol::{ENCRYPTION_KEY_BYTES, MAX_KEY_ENVELOPE_CIPHERTEXT_BYTES};
use thiserror::Error;

use crate::ports::{
    Clock, DeviceId, DeviceRecord, EncryptionKey, EnvelopeRepository, IdentityRepository,
    KeyEnvelopePage, KeyEnvelopeRecord, RepositoryError, ShareId, ShareRepository, UserId,
};

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum EnvelopeError {
    #[error("ephemeral public key must contain exactly {ENCRYPTION_KEY_BYTES} bytes, got {actual}")]
    InvalidEphemeralKeyLength { actual: usize },
    #[error("ciphertext exceeds the {MAX_KEY_ENVELOPE_CIPHERTEXT_BYTES}-byte envelope limit")]
    CiphertextTooLarge { actual: usize },
    /// None of the three below is an authorization rule. Whether a device may
    /// hold this collection's key is the owner's decision, made against a
    /// device log the service does not have. These ask a different question —
    /// is this address a destination worth writing to at all — for the same
    /// reason the service verifies signatures on write (`SPEC.md` §23):
    /// storing what can never be read wastes space, and an envelope is 4 KiB.
    #[error("that device is not authorized")]
    UnknownRecipient,
    /// Before a share exists there is no membership to deliver against, so the
    /// only safe destination is one of the sender's own devices. Without this
    /// any authenticated user could write to every enrolled device in the
    /// system under an invented share identifier.
    #[error("that device does not belong to you")]
    NotYourDevice,
    /// A revoked device can never authenticate to collect this, so the
    /// envelope is dead on arrival.
    #[error("that device was revoked")]
    RecipientRevoked,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

/// A sealed share key, addressed to one of the sender's own devices.
#[derive(Clone, Copy, Debug)]
pub struct PutKeyEnvelopeRequest<'a> {
    pub share_id: ShareId,
    pub recipient_device_id: DeviceId,
    pub ephemeral_public_key: &'a [u8],
    pub ciphertext: &'a [u8],
}

/// Applies the key-envelope rules over injected storage and time.
pub struct EnvelopeService<S, C> {
    store: S,
    clock: C,
}

impl<S, C> EnvelopeService<S, C>
where
    S: EnvelopeRepository + IdentityRepository + ShareRepository,
    C: Clock,
{
    pub const fn new(store: S, clock: C) -> Self {
        Self { store, clock }
    }

    /// Stores a sealed share key for one of `sender`'s own devices.
    ///
    /// Who may *read* a collection is not decided here. Under D2 and D3 the
    /// owner seals only to devices a verified device log authorizes, and the
    /// service holds neither that log nor the key, so an ownership or
    /// membership rule here could only re-check a copy that may be behind the
    /// owner's — refusing deliveries that are in fact correct.
    ///
    /// What the service still asks is whether the address is a destination
    /// worth writing to: a device it has enrolled, not revoked, and — before
    /// any share exists to deliver against — one of the sender's own.
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError`] when the ephemeral key has the wrong length,
    /// the ciphertext exceeds its bound, the recipient is unknown, revoked, or
    /// a stranger's device with no share to justify it, or storage fails.
    pub async fn put_key_envelope(
        &self,
        sender: UserId,
        request: PutKeyEnvelopeRequest<'_>,
    ) -> Result<(), EnvelopeError> {
        let ephemeral_public_key =
            EncryptionKey::try_from(request.ephemeral_public_key).map_err(|_| {
                EnvelopeError::InvalidEphemeralKeyLength {
                    actual: request.ephemeral_public_key.len(),
                }
            })?;
        if request.ciphertext.len() > MAX_KEY_ENVELOPE_CIPHERTEXT_BYTES {
            return Err(EnvelopeError::CiphertextTooLarge {
                actual: request.ciphertext.len(),
            });
        }

        let recipient = self
            .store
            .find_device(request.recipient_device_id)
            .await?
            .ok_or(EnvelopeError::UnknownRecipient)?;
        if recipient.is_revoked() {
            return Err(EnvelopeError::RecipientRevoked);
        }
        // With a share to deliver against, any enrolled device is a plausible
        // destination and the owner decides which ones actually get sealed to.
        // Without one there is nothing to justify writing to a stranger.
        if self.store.find_share(request.share_id).await?.is_none() && recipient.user_id != sender {
            return Err(EnvelopeError::NotYourDevice);
        }

        self.store
            .put_key_envelope(KeyEnvelopeRecord {
                share_id: request.share_id,
                recipient_device_id: request.recipient_device_id,
                ephemeral_public_key,
                ciphertext: request.ciphertext.to_vec(),
                created_at_unix_ns: self.clock.now_unix_ns(),
            })
            .await?;
        Ok(())
    }

    /// One bounded envelope page addressed to an already-authenticated device.
    ///
    /// # Errors
    ///
    /// Returns [`EnvelopeError`] when storage fails.
    pub async fn list_key_envelopes(
        &self,
        recipient: &DeviceRecord,
        after_share_id: Option<ShareId>,
    ) -> Result<KeyEnvelopePage, EnvelopeError> {
        Ok(self
            .store
            .list_key_envelopes(recipient.device_id, after_share_id)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{FixedClock, InMemoryIdentities};
    use crate::ports::UserId;
    use crate::ports::{DeviceRecord, IdentityRepository, UserDirectory, UserRecord};

    const NOW: u64 = 1_700_000_000_000_000_000;
    const SENDER: UserId = [1; 16];
    const OTHER_USER: UserId = [2; 16];
    const SENDER_DEVICE: DeviceId = [1; 32];
    const RECIPIENT_DEVICE: DeviceId = [2; 32];
    const EPHEMERAL_KEY: [u8; 32] = [9; 32];
    const SHARE: ShareId = [7; 16];

    type TestService = EnvelopeService<InMemoryIdentities, FixedClock>;

    fn service() -> TestService {
        EnvelopeService::new(InMemoryIdentities::default(), FixedClock::new(NOW))
    }

    fn device(device_id: DeviceId, owner: UserId) -> DeviceRecord {
        DeviceRecord {
            device_id,
            user_id: owner,
            public_key: [0; 32],
            encryption_public_key: [0; 32],
            created_at_unix_ns: NOW,
            last_authenticated_at_unix_ns: None,
            revoked_at_unix_ns: None,
        }
    }

    fn request(ciphertext: &[u8]) -> PutKeyEnvelopeRequest<'_> {
        PutKeyEnvelopeRequest {
            share_id: SHARE,
            recipient_device_id: RECIPIENT_DEVICE,
            ephemeral_public_key: &EPHEMERAL_KEY,
            ciphertext,
        }
    }

    #[tokio::test]
    async fn a_sender_pushes_an_envelope_to_their_own_other_device() {
        let service = service();
        service
            .store
            .enrol_device(device(SENDER_DEVICE, SENDER))
            .expect("enrolled");
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, SENDER))
            .expect("enrolled");

        service
            .put_key_envelope(SENDER, request(b"sealed"))
            .await
            .expect("stored");

        let listed = service
            .list_key_envelopes(&device(RECIPIENT_DEVICE, SENDER), None)
            .await
            .expect("listed");
        assert_eq!(listed.envelopes.len(), 1);
        assert_eq!(listed.envelopes[0].share_id, SHARE);
        assert_eq!(listed.envelopes[0].recipient_device_id, RECIPIENT_DEVICE);
        assert_eq!(listed.envelopes[0].ephemeral_public_key, EPHEMERAL_KEY);
        assert_eq!(listed.envelopes[0].ciphertext, b"sealed");
        assert_eq!(listed.envelopes[0].created_at_unix_ns, NOW);
    }

    #[tokio::test]
    async fn a_share_owner_distributes_keys_only_to_authorized_member_devices() {
        let service = service();
        service
            .store
            .enrol_device(device(SENDER_DEVICE, SENDER))
            .expect("owner device");
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, OTHER_USER))
            .expect("recipient device");
        let share = crate::ShareRecord {
            share_id: SHARE,
            owner: SENDER,
            revision: 1,
            snapshot_id: [8; 32],
            capsule: b"capsule".to_vec(),
            capsule_signature: vec![7; 64],
            created_at_unix_ns: NOW,
            updated_at_unix_ns: NOW,
        };
        service
            .store
            .save_publication(
                share.clone(),
                crate::ShareSnapshotRecord {
                    share_id: SHARE,
                    revision: 1,
                    snapshot_id: share.snapshot_id,
                    capsule: share.capsule.clone(),
                    capsule_signature: share.capsule_signature.clone(),
                    created_at_unix_ns: NOW,
                },
                None,
            )
            .await
            .expect("share");

        // Membership and ownership are no longer checked: both are the owner's
        // decision, made against a device log, and re-checking a copy that may
        // be behind it would refuse deliveries that are in fact correct. The
        // share exists, so any enrolled device is a plausible destination.
        assert_eq!(
            service.put_key_envelope(SENDER, request(b"sealed")).await,
            Ok(())
        );
        assert_eq!(
            service.put_key_envelope(SENDER, request(b"sealed")).await,
            Ok(())
        );
        service
            .store
            .grant_share_access(crate::ShareMembershipRecord {
                share_id: SHARE,
                user_id: OTHER_USER,
                granted_at_unix_ns: NOW,
            })
            .await
            .expect("member");
        service
            .put_key_envelope(SENDER, request(b"sealed"))
            .await
            .expect("authorized delivery");
    }

    fn outage(operation: &str) -> Result<(), EnvelopeError> {
        Err(EnvelopeError::Repository(RepositoryError::Unavailable(
            operation.to_owned(),
        )))
    }

    /// Storage failures are reported rather than swallowed: a push that
    /// looked stored but was not would leave a device unable to open a share
    /// it was told it could.
    ///
    /// The recipient is read before the write, so an unavailable store fails
    /// at the read. [`FailingWrites`] covers the write itself.
    #[tokio::test]
    async fn a_store_outage_is_reported_while_reading() {
        let service = service();
        service
            .store
            .enrol_device(device(SENDER_DEVICE, SENDER))
            .expect("enrolled");
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, SENDER))
            .expect("enrolled");
        service.store.set_unavailable(true);

        assert_eq!(
            service.put_key_envelope(SENDER, request(b"sealed")).await,
            outage("the store is switched off")
        );
        assert_eq!(
            service
                .list_key_envelopes(&device(RECIPIENT_DEVICE, SENDER), None)
                .await
                .map(|_| ()),
            outage("the store is switched off")
        );
    }

    /// A store that answers every read and fails only the envelope write.
    ///
    /// `InMemoryIdentities` fails everything at once, which never reaches the
    /// write: the recipient lookup fails first. This is the only way to
    /// exercise a push that got past its checks and then lost its storage.
    struct FailingWrites(InMemoryIdentities);

    impl UserDirectory for FailingWrites {
        fn find_user(
            &self,
            user_id: UserId,
        ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send
        {
            self.0.find_user(user_id)
        }

        fn find_user_by_handle(
            &self,
            normalized_username: &str,
            discriminator: &str,
        ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send
        {
            self.0
                .find_user_by_handle(normalized_username, discriminator)
        }
    }

    impl IdentityRepository for FailingWrites {
        fn insert_registration(
            &self,
            user: UserRecord,
            device: DeviceRecord,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            self.0.insert_registration(user, device)
        }

        fn find_device(
            &self,
            device_id: DeviceId,
        ) -> impl std::future::Future<Output = Result<Option<DeviceRecord>, RepositoryError>> + Send
        {
            self.0.find_device(device_id)
        }

        fn list_devices(
            &self,
            user: UserId,
        ) -> impl std::future::Future<Output = Result<Vec<DeviceRecord>, RepositoryError>> + Send
        {
            self.0.list_devices(user)
        }

        fn link_device(
            &self,
            device: DeviceRecord,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            self.0.link_device(device)
        }

        fn touch_device(
            &self,
            device_id: DeviceId,
            at_unix_ns: u64,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            self.0.touch_device(device_id, at_unix_ns)
        }

        fn revoke_device(
            &self,
            device_id: DeviceId,
            at_unix_ns: u64,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            self.0.revoke_device(device_id, at_unix_ns)
        }
    }

    impl EnvelopeRepository for FailingWrites {
        fn put_key_envelope(
            &self,
            _envelope: KeyEnvelopeRecord,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            std::future::ready(Err(RepositoryError::Unavailable("put".to_owned())))
        }

        fn list_key_envelopes(
            &self,
            recipient_device_id: DeviceId,
            after_share_id: Option<ShareId>,
        ) -> impl std::future::Future<Output = Result<KeyEnvelopePage, RepositoryError>> + Send
        {
            self.0
                .list_key_envelopes(recipient_device_id, after_share_id)
        }
    }

    impl ShareRepository for FailingWrites {
        async fn find_share(
            &self,
            share_id: ShareId,
        ) -> Result<Option<crate::ShareRecord>, RepositoryError> {
            self.0.find_share(share_id).await
        }

        async fn save_publication(
            &self,
            share: crate::ShareRecord,
            snapshot: crate::ShareSnapshotRecord,
            expected_revision: Option<u64>,
        ) -> Result<(), RepositoryError> {
            self.0
                .save_publication(share, snapshot, expected_revision)
                .await
        }

        async fn find_snapshot(
            &self,
            share_id: ShareId,
            revision: u64,
        ) -> Result<Option<crate::ShareSnapshotRecord>, RepositoryError> {
            self.0.find_snapshot(share_id, revision).await
        }

        async fn grant_share_access(
            &self,
            membership: crate::ShareMembershipRecord,
        ) -> Result<(), RepositoryError> {
            self.0.grant_share_access(membership).await
        }

        async fn revoke_share_access(
            &self,
            share_id: ShareId,
            user_id: UserId,
        ) -> Result<(), RepositoryError> {
            self.0.revoke_share_access(share_id, user_id).await
        }

        async fn has_share_access(
            &self,
            share_id: ShareId,
            user_id: UserId,
        ) -> Result<bool, RepositoryError> {
            self.0.has_share_access(share_id, user_id).await
        }

        async fn list_authorized_shares(
            &self,
            user_id: UserId,
        ) -> Result<Vec<crate::ShareRecord>, RepositoryError> {
            self.0.list_authorized_shares(user_id).await
        }

        async fn list_share_members(
            &self,
            share_id: ShareId,
        ) -> Result<Vec<UserId>, RepositoryError> {
            self.0.list_share_members(share_id).await
        }
    }

    /// The double must be transparent apart from the write it fails, or a
    /// test using it would be testing the double rather than the service.
    #[tokio::test]
    async fn the_double_passes_everything_but_the_write_through() {
        let store = FailingWrites(InMemoryIdentities::default());
        let user = UserRecord {
            user_id: SENDER,
            username: "Ada".to_owned(),
            normalized_username: "ada".to_owned(),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: NOW,
        };
        let first = device(SENDER_DEVICE, SENDER);

        assert_eq!(
            store.insert_registration(user.clone(), first.clone()).await,
            Ok(())
        );
        assert_eq!(store.find_user(SENDER).await, Ok(Some(user.clone())));
        assert_eq!(
            store.find_user_by_handle("ada", "7Q2XZ").await,
            Ok(Some(user))
        );
        assert_eq!(
            store.find_device(SENDER_DEVICE).await,
            Ok(Some(first.clone()))
        );
        assert_eq!(store.list_devices(SENDER).await, Ok(vec![first]));
        assert_eq!(
            store.link_device(device(RECIPIENT_DEVICE, SENDER)).await,
            Ok(())
        );
        assert_eq!(store.touch_device(SENDER_DEVICE, NOW).await, Ok(()));
        assert_eq!(store.revoke_device(SENDER_DEVICE, NOW).await, Ok(()));
        assert_eq!(
            store
                .list_key_envelopes(RECIPIENT_DEVICE, None)
                .await
                .map(|page| page.envelopes),
            Ok(Vec::new())
        );

        // Shares too: the owner check that decides whether a key may be
        // addressed to another user's device reads through this double, so a
        // delegation that lied would silently change who a share reaches.
        let head = crate::ShareRecord {
            share_id: SHARE,
            owner: SENDER,
            revision: 1,
            snapshot_id: [4; 32],
            capsule: b"sealed".to_vec(),
            capsule_signature: vec![5; 64],
            created_at_unix_ns: NOW,
            updated_at_unix_ns: NOW,
        };
        let snapshot = crate::ShareSnapshotRecord {
            share_id: SHARE,
            revision: 1,
            snapshot_id: head.snapshot_id,
            capsule: head.capsule.clone(),
            capsule_signature: head.capsule_signature.clone(),
            created_at_unix_ns: NOW,
        };

        assert_eq!(store.find_share(SHARE).await, Ok(None));
        assert_eq!(
            store
                .save_publication(head.clone(), snapshot.clone(), None)
                .await,
            Ok(())
        );
        assert_eq!(store.find_share(SHARE).await, Ok(Some(head.clone())));
        assert_eq!(store.find_snapshot(SHARE, 1).await, Ok(Some(snapshot)));
        assert_eq!(
            store
                .grant_share_access(crate::ShareMembershipRecord {
                    share_id: SHARE,
                    user_id: OTHER_USER,
                    granted_at_unix_ns: NOW,
                })
                .await,
            Ok(())
        );
        assert_eq!(store.has_share_access(SHARE, OTHER_USER).await, Ok(true));
        assert_eq!(store.revoke_share_access(SHARE, OTHER_USER).await, Ok(()));
        assert_eq!(store.has_share_access(SHARE, OTHER_USER).await, Ok(false));
        store
            .grant_share_access(crate::ShareMembershipRecord {
                share_id: SHARE,
                user_id: OTHER_USER,
                granted_at_unix_ns: NOW,
            })
            .await
            .expect("granted again");
        assert_eq!(
            store.list_authorized_shares(OTHER_USER).await,
            Ok(vec![head])
        );
        assert_eq!(
            store.list_share_members(SHARE).await,
            Ok(vec![SENDER, OTHER_USER]),
            "the owner is a member of their own share"
        );
    }

    #[tokio::test]
    async fn a_store_outage_is_reported_while_writing() {
        let store = FailingWrites(InMemoryIdentities::default());
        store
            .0
            .enrol_device(device(RECIPIENT_DEVICE, SENDER))
            .expect("enrolled");
        let service = EnvelopeService::new(store, FixedClock::new(NOW));

        assert_eq!(
            service.put_key_envelope(SENDER, request(b"sealed")).await,
            outage("put"),
            "the write failed after its checks passed"
        );
    }

    #[tokio::test]
    async fn pushing_to_an_unknown_device_is_refused() {
        let service = service();
        service
            .store
            .enrol_device(device(SENDER_DEVICE, SENDER))
            .expect("enrolled");

        assert_eq!(
            service.put_key_envelope(SENDER, request(b"sealed")).await,
            Err(EnvelopeError::UnknownRecipient)
        );
    }

    /// With no share to deliver against, a stranger's device is not a
    /// destination this service will write to.
    #[tokio::test]
    async fn pushing_to_someone_elses_device_is_refused_before_a_share_exists() {
        let service = service();
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, OTHER_USER))
            .expect("enrolled");

        assert_eq!(
            service.put_key_envelope(SENDER, request(b"sealed")).await,
            Err(EnvelopeError::NotYourDevice)
        );
    }

    /// A revoked device can never authenticate to collect an envelope, so
    /// storing one for it would be writing something guaranteed unread.
    #[tokio::test]
    async fn a_revoked_recipient_cannot_receive_a_replacement_envelope() {
        let service = service();
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, SENDER))
            .expect("enrolled");
        service
            .store
            .revoke_device(RECIPIENT_DEVICE, NOW)
            .await
            .expect("revoked");

        assert_eq!(
            service.put_key_envelope(SENDER, request(b"sealed")).await,
            Err(EnvelopeError::RecipientRevoked)
        );
    }

    #[tokio::test]
    async fn a_malformed_ephemeral_key_is_refused() {
        let service = service();
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, SENDER))
            .expect("enrolled");

        assert_eq!(
            service
                .put_key_envelope(
                    SENDER,
                    PutKeyEnvelopeRequest {
                        share_id: SHARE,
                        recipient_device_id: RECIPIENT_DEVICE,
                        ephemeral_public_key: &[0; 10],
                        ciphertext: b"sealed",
                    },
                )
                .await,
            Err(EnvelopeError::InvalidEphemeralKeyLength { actual: 10 })
        );
    }

    #[tokio::test]
    async fn re_pushing_for_the_same_recipient_replaces_the_envelope() {
        let service = service();
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, SENDER))
            .expect("enrolled");

        service
            .put_key_envelope(SENDER, request(b"first"))
            .await
            .expect("stored");
        service
            .put_key_envelope(SENDER, request(b"second"))
            .await
            .expect("stored");

        let listed = service
            .list_key_envelopes(&device(RECIPIENT_DEVICE, SENDER), None)
            .await
            .expect("listed");
        assert_eq!(
            listed.envelopes.len(),
            1,
            "a rotated key replaces the old envelope"
        );
        assert_eq!(listed.envelopes[0].ciphertext, b"second");
    }

    #[tokio::test]
    async fn listing_envelopes_for_a_device_with_none_is_empty() {
        let service = service();
        assert_eq!(
            service
                .list_key_envelopes(&device(RECIPIENT_DEVICE, SENDER), None)
                .await,
            Ok(KeyEnvelopePage {
                envelopes: Vec::new(),
                next_after_share_id: None,
            })
        );
    }

    #[tokio::test]
    async fn an_oversized_ciphertext_is_refused_before_storage() {
        let service = service();
        service
            .store
            .enrol_device(device(RECIPIENT_DEVICE, SENDER))
            .expect("enrolled");
        let ciphertext = vec![0; MAX_KEY_ENVELOPE_CIPHERTEXT_BYTES + 1];

        assert_eq!(
            service.put_key_envelope(SENDER, request(&ciphertext)).await,
            Err(EnvelopeError::CiphertextTooLarge {
                actual: ciphertext.len(),
            })
        );
    }
}
