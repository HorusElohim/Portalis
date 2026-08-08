//! Registration and device authentication.

use portalis_nexus_protocol::{
    DISCRIMINATOR_CHARS, SessionBinding, SignatureError, UUID_V7_ENTROPY_BYTES,
    authentication_payload, derive_device_id, registration_payload, user_id_from, verify_signature,
};
use thiserror::Error;

use crate::handle::{
    HandleError, discriminator_from_entropy, normalize_username, validate_username,
};
use crate::ports::{
    Clock, DeviceKey, DeviceRecord, DeviceRepository, RandomSource, RepositoryError, UserId,
    UserRecord, UserRepository,
};

/// How many random discriminators a registration tries before giving up.
///
/// Allocation always retries a fresh random value against the unique index; it
/// never scans for the next free one, which would leak how many users share a
/// username and serialise every registration on the same scan.
pub const HANDLE_ALLOCATION_ATTEMPTS: usize = 8;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    #[error(transparent)]
    Handle(#[from] HandleError),
    #[error(transparent)]
    Signature(#[from] SignatureError),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error("no free discriminator after {HANDLE_ALLOCATION_ATTEMPTS} attempts")]
    UsernameUnavailable,
    #[error("this device is already registered")]
    DeviceAlreadyRegistered,
    #[error("this device is not authorized")]
    UnknownDevice,
    #[error("this device was revoked")]
    DeviceRevoked,
    #[error("the device is authorized but its user is missing")]
    MissingUser,
}

/// Who a verified connection belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Identity {
    pub user: UserRecord,
    pub device: DeviceRecord,
}

/// A signed request to claim a new username.
#[derive(Clone, Copy, Debug)]
pub struct RegistrationRequest<'a> {
    pub binding: SessionBinding<'a>,
    pub requested_username: &'a str,
    pub device_public_key: &'a [u8],
    pub signature: &'a [u8],
}

/// A signed request to prove ownership of an enrolled device.
#[derive(Clone, Copy, Debug)]
pub struct AuthenticationRequest<'a> {
    pub binding: SessionBinding<'a>,
    pub device_public_key: &'a [u8],
    pub signature: &'a [u8],
}

/// Applies the identity rules over injected storage, time, and randomness.
pub struct IdentityService<U, D, C, R> {
    users: U,
    devices: D,
    clock: C,
    random: R,
}

impl<U, D, C, R> IdentityService<U, D, C, R>
where
    U: UserRepository,
    D: DeviceRepository,
    C: Clock,
    R: RandomSource,
{
    pub const fn new(users: U, devices: D, clock: C, random: R) -> Self {
        Self {
            users,
            devices,
            clock,
            random,
        }
    }

    /// Claims a username and enrols the signing device as its first.
    ///
    /// The signature is verified before anything is written, so an unsigned
    /// request cannot consume a discriminator or reveal whether a name exists.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError`] when the username breaks the handle rules,
    /// the signature does not cover this request, the device is already
    /// enrolled, no discriminator is free, or storage fails.
    pub async fn register(
        &self,
        request: RegistrationRequest<'_>,
    ) -> Result<Identity, IdentityError> {
        validate_username(request.requested_username)?;
        let payload = registration_payload(
            &request.binding,
            request.requested_username,
            request.device_public_key,
        );
        let public_key = Self::verify(request.device_public_key, &payload, request.signature)?;

        let device_id = derive_device_id(&public_key);
        if self.devices.find_device(device_id).await?.is_some() {
            return Err(IdentityError::DeviceAlreadyRegistered);
        }

        let now = self.clock.now_unix_ms();
        let user_id = self.new_user_id(now);
        let user = self
            .allocate_handle(request.requested_username, user_id, now)
            .await?;

        // The user and its first device must land together. Until the adapter
        // wraps both writes in a transaction, a failure here leaves a user
        // with no device, which registration below reports rather than hides.
        let device = DeviceRecord {
            device_id,
            user_id,
            public_key,
            created_at_unix_ms: now,
            last_authenticated_at_unix_ms: Some(now),
            revoked_at_unix_ms: None,
        };
        self.devices.insert_device(device.clone()).await?;

        Ok(Identity { user, device })
    }

    /// Verifies that a connection holds the key of an authorized device.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError`] when the signature does not cover this
    /// request, the device is unknown or revoked, its user is missing, or
    /// storage fails.
    pub async fn authenticate(
        &self,
        request: AuthenticationRequest<'_>,
    ) -> Result<Identity, IdentityError> {
        let payload = authentication_payload(&request.binding, request.device_public_key);
        let public_key = Self::verify(request.device_public_key, &payload, request.signature)?;

        let device_id = derive_device_id(&public_key);
        let mut device = self
            .devices
            .find_device(device_id)
            .await?
            .ok_or(IdentityError::UnknownDevice)?;
        if device.is_revoked() {
            return Err(IdentityError::DeviceRevoked);
        }

        let user = self
            .users
            .find_user(device.user_id)
            .await?
            .ok_or(IdentityError::MissingUser)?;

        let now = self.clock.now_unix_ms();
        self.devices.touch_device(device_id, now).await?;
        device.last_authenticated_at_unix_ms = Some(now);

        Ok(Identity { user, device })
    }

    /// Revokes a device, ending its ability to authenticate.
    ///
    /// # Errors
    ///
    /// Returns [`IdentityError`] when the device is unknown or storage fails.
    pub async fn revoke_device(&self, device_id: [u8; 32]) -> Result<(), IdentityError> {
        if self.devices.find_device(device_id).await?.is_none() {
            return Err(IdentityError::UnknownDevice);
        }
        let now = self.clock.now_unix_ms();
        self.devices.revoke_device(device_id, now).await?;
        Ok(())
    }

    /// Verifies a signature and returns the key in its fixed-size form.
    fn verify(
        device_public_key: &[u8],
        payload: &[u8],
        signature: &[u8],
    ) -> Result<DeviceKey, IdentityError> {
        verify_signature(device_public_key, payload, signature)?;
        // Verification already proved the length, so this cannot fail.
        let key = DeviceKey::try_from(device_public_key)
            .expect("a verified signature implies a fixed-size key");
        Ok(key)
    }

    fn new_user_id(&self, now_unix_ms: u64) -> UserId {
        let mut entropy = [0_u8; UUID_V7_ENTROPY_BYTES];
        self.random.fill(&mut entropy);
        user_id_from(now_unix_ms, &entropy)
    }

    /// Retries random discriminators against the unique index.
    async fn allocate_handle(
        &self,
        username: &str,
        user_id: UserId,
        now_unix_ms: u64,
    ) -> Result<UserRecord, IdentityError> {
        for _ in 0..HANDLE_ALLOCATION_ATTEMPTS {
            let mut entropy = [0_u8; DISCRIMINATOR_CHARS];
            self.random.fill(&mut entropy);
            // The username was validated on entry and the discriminator is
            // generated from the alphabet, so the pair is a valid handle by
            // construction and needs no re-validation here.
            let user = UserRecord {
                user_id,
                username: username.to_owned(),
                normalized_username: normalize_username(username),
                discriminator: discriminator_from_entropy(&entropy),
                created_at_unix_ms: now_unix_ms,
            };
            match self.users.insert_user(user.clone()).await {
                Ok(()) => return Ok(user),
                Err(RepositoryError::HandleTaken) => {}
                Err(error) => return Err(error.into()),
            }
        }
        Err(IdentityError::UsernameUnavailable)
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};
    use portalis_nexus_protocol::CURRENT_PROTOCOL_VERSION;

    use super::*;
    use crate::memory::{FixedClock, InMemoryDevices, InMemoryUsers, ScriptedRandom};

    const NOW: u64 = 1_700_000_000_000;
    const AUTHORITY: &str = "nexus.portalis.test";

    /// One service type across every test. `IdentityService` is generic, so a
    /// second instantiation would be measured as its own set of coverage
    /// regions; the fault-injecting stores stand in for the plain ones with
    /// `Fault::None`.
    type TestService = IdentityService<FaultyUsers, FaultyDevices, FixedClock, ScriptedRandom>;

    /// Which store operation should fail, so the service's degraded paths are
    /// exercised rather than assumed.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
    enum Fault {
        #[default]
        None,
        Find,
        Insert,
        Touch,
        Revoke,
    }

    impl Fault {
        /// Returns the failure for `operation`, or `None` to pass through.
        fn hits(self, operation: Fault) -> Option<RepositoryError> {
            (self == operation).then(|| RepositoryError::Unavailable(self.label().to_owned()))
        }

        fn label(self) -> &'static str {
            match self {
                Self::None => "none",
                Self::Find => "find",
                Self::Insert => "insert",
                Self::Touch => "touch",
                Self::Revoke => "revoke",
            }
        }
    }

    #[derive(Default)]
    struct FaultyUsers {
        inner: InMemoryUsers,
        fault: Fault,
    }

    impl UserRepository for FaultyUsers {
        fn insert_user(
            &self,
            user: UserRecord,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            let failure = self.fault.hits(Fault::Insert);
            let inner = self.inner.insert_user(user);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }

        fn find_user(
            &self,
            user_id: UserId,
        ) -> impl std::future::Future<Output = Result<Option<UserRecord>, RepositoryError>> + Send
        {
            let failure = self.fault.hits(Fault::Find);
            let inner = self.inner.find_user(user_id);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }
    }

    #[derive(Default)]
    struct FaultyDevices {
        inner: InMemoryDevices,
        fault: Fault,
    }

    impl DeviceRepository for FaultyDevices {
        fn insert_device(
            &self,
            device: DeviceRecord,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            let failure = self.fault.hits(Fault::Insert);
            let inner = self.inner.insert_device(device);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }

        fn find_device(
            &self,
            device_id: crate::ports::DeviceId,
        ) -> impl std::future::Future<Output = Result<Option<DeviceRecord>, RepositoryError>> + Send
        {
            let failure = self.fault.hits(Fault::Find);
            let inner = self.inner.find_device(device_id);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }

        fn touch_device(
            &self,
            device_id: crate::ports::DeviceId,
            at_unix_ms: u64,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            let failure = self.fault.hits(Fault::Touch);
            let inner = self.inner.touch_device(device_id, at_unix_ms);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }

        fn revoke_device(
            &self,
            device_id: crate::ports::DeviceId,
            at_unix_ms: u64,
        ) -> impl std::future::Future<Output = Result<(), RepositoryError>> + Send {
            let failure = self.fault.hits(Fault::Revoke);
            let inner = self.inner.revoke_device(device_id, at_unix_ms);
            async move {
                match failure {
                    Some(error) => Err(error),
                    None => inner.await,
                }
            }
        }
    }

    fn unavailable(operation: &str) -> IdentityError {
        IdentityError::Repository(RepositoryError::Unavailable(operation.to_owned()))
    }

    fn service(random: &[u8]) -> TestService {
        service_with(FaultyUsers::default(), FaultyDevices::default(), random)
    }

    fn service_with(users: FaultyUsers, devices: FaultyDevices, random: &[u8]) -> TestService {
        IdentityService::new(
            users,
            devices,
            FixedClock::new(NOW),
            ScriptedRandom::new(random),
        )
    }

    fn key(seed: u8) -> SigningKey {
        SigningKey::from_bytes(&[seed; 32])
    }

    fn binding(challenge: &[u8; 32]) -> SessionBinding<'_> {
        SessionBinding {
            protocol_version: CURRENT_PROTOCOL_VERSION,
            server_authority: AUTHORITY,
            connection_id: &[4; 16],
            challenge,
            server_time_unix_ms: NOW,
        }
    }

    /// Signs a well-formed registration for `username` with `signer`.
    fn registration<'a>(
        signer: &SigningKey,
        username: &'a str,
        challenge: &'a [u8; 32],
        public_key: &'a mut [u8; 32],
        signature: &'a mut [u8; 64],
    ) -> RegistrationRequest<'a> {
        *public_key = signer.verifying_key().to_bytes();
        let payload = registration_payload(&binding(challenge), username, public_key);
        *signature = signer.sign(&payload).to_bytes();
        RegistrationRequest {
            binding: binding(challenge),
            requested_username: username,
            device_public_key: public_key,
            signature,
        }
    }

    /// Signs a well-formed authentication for `signer`.
    fn authentication<'a>(
        signer: &SigningKey,
        challenge: &'a [u8; 32],
        public_key: &'a mut [u8; 32],
        signature: &'a mut [u8; 64],
    ) -> AuthenticationRequest<'a> {
        *public_key = signer.verifying_key().to_bytes();
        let payload = authentication_payload(&binding(challenge), public_key);
        *signature = signer.sign(&payload).to_bytes();
        AuthenticationRequest {
            binding: binding(challenge),
            device_public_key: public_key,
            signature,
        }
    }

    #[tokio::test]
    async fn registers_a_user_and_enrols_its_first_device() {
        let service = service(&[9]);
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);

        let identity = service
            .register(registration(
                &signer,
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("registration succeeds");

        assert_eq!(identity.user.username, "Ada");
        assert_eq!(identity.user.normalized_username, "ada");
        assert_eq!(identity.user.discriminator.len(), DISCRIMINATOR_CHARS);
        assert_eq!(identity.user.created_at_unix_ms, NOW);
        assert_eq!(identity.device.device_id, derive_device_id(&public));
        assert_eq!(identity.device.user_id, identity.user.user_id);
        assert_eq!(identity.device.public_key, public);
        assert!(!identity.device.is_revoked());
    }

    #[tokio::test]
    async fn an_enrolled_device_authenticates_and_records_the_time() {
        let service = service(&[9]);
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);
        let registered = service
            .register(registration(
                &signer,
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("registration succeeds");
        service.clock.advance(5_000);

        let identity = service
            .authenticate(authentication(
                &signer,
                &[2; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("authentication succeeds");

        assert_eq!(identity.user, registered.user);
        assert_eq!(identity.device.device_id, registered.device.device_id);
        assert_eq!(
            identity.device.last_authenticated_at_unix_ms,
            Some(NOW + 5_000)
        );
    }

    #[tokio::test]
    async fn rejects_a_signature_from_the_wrong_key() {
        let service = service(&[9]);
        let signer = key(7);
        let impostor = key(8);
        let (mut public, mut signature) = ([0; 32], [0; 64]);
        service
            .register(registration(
                &signer,
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("registration succeeds");

        // The impostor claims the enrolled key but signs with its own.
        let enrolled = signer.verifying_key().to_bytes();
        let payload = authentication_payload(&binding(&[2; 32]), &enrolled);
        let forged = impostor.sign(&payload).to_bytes();

        assert_eq!(
            service
                .authenticate(AuthenticationRequest {
                    binding: binding(&[2; 32]),
                    device_public_key: &enrolled,
                    signature: &forged,
                })
                .await,
            Err(IdentityError::Signature(SignatureError::Rejected))
        );
    }

    #[tokio::test]
    async fn rejects_a_signature_replayed_from_another_challenge() {
        let service = service(&[9]);
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);
        service
            .register(registration(
                &signer,
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("registration succeeds");
        // A signature captured against one challenge.
        let captured = authentication(&signer, &[2; 32], &mut public, &mut signature);
        let stolen_signature = *captured.signature.first_chunk::<64>().expect("64 bytes");

        // Replayed onto a connection issued a different challenge.
        let public_key = signer.verifying_key().to_bytes();
        assert_eq!(
            service
                .authenticate(AuthenticationRequest {
                    binding: binding(&[3; 32]),
                    device_public_key: &public_key,
                    signature: &stolen_signature,
                })
                .await,
            Err(IdentityError::Signature(SignatureError::Rejected))
        );
    }

    #[tokio::test]
    async fn rejects_an_unknown_device() {
        let service = service(&[9]);
        let stranger = key(11);
        let (mut public, mut signature) = ([0; 32], [0; 64]);

        assert_eq!(
            service
                .authenticate(authentication(
                    &stranger,
                    &[2; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(IdentityError::UnknownDevice)
        );
    }

    #[tokio::test]
    async fn a_revoked_device_can_no_longer_authenticate() {
        let service = service(&[9]);
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);
        let identity = service
            .register(registration(
                &signer,
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("registration succeeds");

        service
            .revoke_device(identity.device.device_id)
            .await
            .expect("revocation succeeds");

        assert_eq!(
            service
                .authenticate(authentication(
                    &signer,
                    &[2; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(IdentityError::DeviceRevoked)
        );
        assert_eq!(
            service.revoke_device([0; 32]).await,
            Err(IdentityError::UnknownDevice)
        );
    }

    #[tokio::test]
    async fn refuses_to_register_the_same_device_twice() {
        let service = service(&[9]);
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);
        service
            .register(registration(
                &signer,
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("registration succeeds");

        assert_eq!(
            service
                .register(registration(
                    &signer,
                    "Grace",
                    &[1; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(IdentityError::DeviceAlreadyRegistered)
        );
    }

    #[tokio::test]
    async fn rejects_a_username_that_breaks_the_handle_rules() {
        let service = service(&[9]);
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);

        assert_eq!(
            service
                .register(registration(
                    &signer,
                    "ad",
                    &[1; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(IdentityError::Handle(HandleError::UsernameTooShort {
                actual: 2
            }))
        );
        assert!(
            service.users.inner.is_empty() && service.devices.inner.is_empty(),
            "a rejected registration must write nothing"
        );
    }

    #[tokio::test]
    async fn shares_a_username_by_allocating_a_different_discriminator() {
        // Randomness that yields a fresh discriminator on the second draw.
        let service = service(&[1, 1, 1, 1, 1, 2]);
        let (mut public, mut signature) = ([0; 32], [0; 64]);
        let first = service
            .register(registration(
                &key(7),
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("first registration succeeds");

        let second = service
            .register(registration(
                &key(8),
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("second registration succeeds");

        assert_eq!(
            first.user.normalized_username,
            second.user.normalized_username
        );
        assert_ne!(first.user.discriminator, second.user.discriminator);
        assert_ne!(first.user.user_id, second.user.user_id);
    }

    #[tokio::test]
    async fn gives_up_when_no_discriminator_is_free() {
        // Constant randomness always proposes the same taken discriminator.
        let service = service(&[1]);
        let (mut public, mut signature) = ([0; 32], [0; 64]);
        service
            .register(registration(
                &key(7),
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("first registration succeeds");

        assert_eq!(
            service
                .register(registration(
                    &key(8),
                    "Ada",
                    &[1; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(IdentityError::UsernameUnavailable)
        );
    }

    #[tokio::test]
    async fn rejects_a_registration_that_is_not_signed_for_this_request() {
        let service = service(&[9]);
        let signer = key(7);
        let public = signer.verifying_key().to_bytes();
        // Signed for a different username than the one requested.
        let payload = registration_payload(&binding(&[1; 32]), "Grace", &public);
        let signature = signer.sign(&payload).to_bytes();

        assert_eq!(
            service
                .register(RegistrationRequest {
                    binding: binding(&[1; 32]),
                    requested_username: "Ada",
                    device_public_key: &public,
                    signature: &signature,
                })
                .await,
            Err(IdentityError::Signature(SignatureError::Rejected))
        );
        assert!(
            service.users.inner.is_empty(),
            "nothing is written for a bad signature"
        );
    }

    #[tokio::test]
    async fn authenticating_a_device_whose_user_vanished_is_reported() {
        let devices = FaultyDevices::default();
        let signer = key(7);
        let public = signer.verifying_key().to_bytes();
        devices
            .inner
            .insert_device(DeviceRecord {
                device_id: derive_device_id(&public),
                user_id: [9; 16],
                public_key: public,
                created_at_unix_ms: NOW,
                last_authenticated_at_unix_ms: None,
                revoked_at_unix_ms: None,
            })
            .await
            .expect("device inserted");
        let service = service_with(FaultyUsers::default(), devices, &[9]);
        let (mut public, mut signature) = ([0; 32], [0; 64]);

        assert_eq!(
            service
                .authenticate(authentication(
                    &signer,
                    &[2; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(IdentityError::MissingUser)
        );
    }

    /// Builds a service whose device store already holds `signer`'s device.
    async fn with_enrolled_device(
        users: FaultyUsers,
        fault: Fault,
        signer: &SigningKey,
    ) -> TestService {
        let devices = FaultyDevices {
            fault,
            ..FaultyDevices::default()
        };
        let public = signer.verifying_key().to_bytes();
        devices
            .inner
            .insert_device(DeviceRecord {
                device_id: derive_device_id(&public),
                user_id: [9; 16],
                public_key: public,
                created_at_unix_ms: NOW,
                last_authenticated_at_unix_ms: None,
                revoked_at_unix_ms: None,
            })
            .await
            .expect("device inserted");
        service_with(users, devices, &[9])
    }

    /// A user store already holding the user those devices belong to.
    async fn users_with_owner() -> FaultyUsers {
        let users = FaultyUsers::default();
        users
            .insert_user(UserRecord {
                user_id: [9; 16],
                username: "Ada".to_owned(),
                normalized_username: "ada".to_owned(),
                discriminator: "7Q2XZ".to_owned(),
                created_at_unix_ms: NOW,
            })
            .await
            .expect("user inserted");
        users
    }

    #[tokio::test]
    async fn reports_storage_failures_during_registration() {
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);

        for (fault, users) in [
            (Fault::Find, FaultyUsers::default()),
            (
                Fault::None,
                FaultyUsers {
                    fault: Fault::Insert,
                    ..FaultyUsers::default()
                },
            ),
            (Fault::Insert, FaultyUsers::default()),
        ] {
            let expected = if fault == Fault::None {
                Fault::Insert
            } else {
                fault
            };
            let service = service_with(
                users,
                FaultyDevices {
                    fault,
                    ..FaultyDevices::default()
                },
                &[9],
            );

            assert_eq!(
                service
                    .register(registration(
                        &signer,
                        "Ada",
                        &[1; 32],
                        &mut public,
                        &mut signature
                    ))
                    .await,
                Err(unavailable(expected.label()))
            );
        }
    }

    #[tokio::test]
    async fn reports_storage_failures_during_authentication() {
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);

        let lookup_fails = with_enrolled_device(FaultyUsers::default(), Fault::Find, &signer).await;
        assert_eq!(
            lookup_fails
                .authenticate(authentication(
                    &signer,
                    &[2; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(unavailable("find"))
        );

        let user_lookup_fails = with_enrolled_device(
            FaultyUsers {
                fault: Fault::Find,
                ..FaultyUsers::default()
            },
            Fault::None,
            &signer,
        )
        .await;
        assert_eq!(
            user_lookup_fails
                .authenticate(authentication(
                    &signer,
                    &[2; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(unavailable("find"))
        );

        let touch_fails =
            with_enrolled_device(users_with_owner().await, Fault::Touch, &signer).await;
        assert_eq!(
            touch_fails
                .authenticate(authentication(
                    &signer,
                    &[2; 32],
                    &mut public,
                    &mut signature
                ))
                .await,
            Err(unavailable("touch"))
        );
    }

    #[tokio::test]
    async fn reports_storage_failures_during_revocation() {
        let signer = key(7);
        let device_id = derive_device_id(&signer.verifying_key().to_bytes());

        let lookup_fails = with_enrolled_device(FaultyUsers::default(), Fault::Find, &signer).await;
        assert_eq!(
            lookup_fails.revoke_device(device_id).await,
            Err(unavailable("find"))
        );

        let revoke_fails =
            with_enrolled_device(FaultyUsers::default(), Fault::Revoke, &signer).await;
        assert_eq!(
            revoke_fails.revoke_device(device_id).await,
            Err(unavailable("revoke"))
        );
    }

    /// The fault-injecting doubles must be transparent when nothing is set to
    /// fail, or the failure tests above could pass for the wrong reason.
    #[tokio::test]
    async fn the_doubles_pass_through_when_no_fault_is_injected() {
        let signer = key(7);
        let (mut public, mut signature) = ([0; 32], [0; 64]);

        let fresh = service(&[9]);
        fresh
            .register(registration(
                &signer,
                "Ada",
                &[1; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("registration passes through");

        let enrolled = with_enrolled_device(users_with_owner().await, Fault::None, &signer).await;
        enrolled
            .authenticate(authentication(
                &signer,
                &[2; 32],
                &mut public,
                &mut signature,
            ))
            .await
            .expect("authentication passes through");
        enrolled
            .revoke_device(derive_device_id(&signer.verifying_key().to_bytes()))
            .await
            .expect("revocation passes through");

        assert_eq!(Fault::None.label(), "none");
    }
}
