//! How durable records are shaped in `MongoDB`.
//!
//! Binary identifiers are stored as BSON binary rather than hex strings, so an
//! index keeps their natural size and ordering. Counts are `i64` because BSON
//! has no unsigned integer. Nanoseconds since the epoch fit with room to
//! spare: `i64` runs out in 2262.

use mongodb::bson::{Binary, spec::BinarySubtype};
use portalis_nexus_protocol::v1::FriendshipState;
use portalis_nexus_server_core::{
    DeviceRecord, FriendshipEdge, FriendshipRecord, KeyEnvelopeRecord, ShareMembershipRecord,
    ShareRecord, ShareSnapshotRecord, UserRecord,
};
use serde::{Deserialize, Serialize};

/// The shape version each document is written with, so a later migration can
/// tell what it is reading.
pub(crate) const SCHEMA_VERSION: i32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct UserDocument {
    #[serde(rename = "_id")]
    pub user_id: Binary,
    pub username: String,
    pub normalized_username: String,
    pub discriminator: String,
    pub created_at_unix_ns: i64,
    pub schema_version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct DeviceDocument {
    #[serde(rename = "_id")]
    pub device_id: Binary,
    pub user_id: Binary,
    pub public_key: Binary,
    pub encryption_public_key: Binary,
    pub created_at_unix_ns: i64,
    pub last_authenticated_at_unix_ns: Option<i64>,
    pub revoked_at_unix_ns: Option<i64>,
    pub schema_version: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct FriendshipDocument {
    pub user_low: Binary,
    pub user_high: Binary,
    pub requested_by: Binary,
    pub state: i32,
    pub version: i64,
    pub created_at_unix_ns: i64,
    pub updated_at_unix_ns: i64,
    pub schema_version: i32,
}

/// Wraps bytes as BSON binary.
pub(crate) fn binary(bytes: &[u8]) -> Binary {
    Binary {
        subtype: BinarySubtype::Generic,
        bytes: bytes.to_vec(),
    }
}

/// Reads a fixed-size identifier back out, or `None` if the stored value is
/// the wrong width for it.
pub(crate) fn fixed<const N: usize>(value: &Binary) -> Option<[u8; N]> {
    value.bytes.as_slice().try_into().ok()
}

/// Clamps a count into the signed range BSON stores. Unit-neutral: this
/// carries nanosecond timestamps and friendship versions alike.
pub(crate) fn signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Reads a stored count back, treating a negative value as zero.
pub(crate) fn unsigned(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

impl UserDocument {
    pub(crate) fn from_record(user: &UserRecord) -> Self {
        Self {
            user_id: binary(&user.user_id),
            username: user.username.clone(),
            normalized_username: user.normalized_username.clone(),
            discriminator: user.discriminator.clone(),
            created_at_unix_ns: signed(user.created_at_unix_ns),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub(crate) fn into_record(self) -> Option<UserRecord> {
        Some(UserRecord {
            user_id: fixed(&self.user_id)?,
            username: self.username,
            normalized_username: self.normalized_username,
            discriminator: self.discriminator,
            created_at_unix_ns: unsigned(self.created_at_unix_ns),
        })
    }
}

impl DeviceDocument {
    pub(crate) fn from_record(device: &DeviceRecord) -> Self {
        Self {
            device_id: binary(&device.device_id),
            user_id: binary(&device.user_id),
            public_key: binary(&device.public_key),
            encryption_public_key: binary(&device.encryption_public_key),
            created_at_unix_ns: signed(device.created_at_unix_ns),
            last_authenticated_at_unix_ns: device.last_authenticated_at_unix_ns.map(signed),
            revoked_at_unix_ns: device.revoked_at_unix_ns.map(signed),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub(crate) fn into_record(self) -> Option<DeviceRecord> {
        Some(DeviceRecord {
            device_id: fixed(&self.device_id)?,
            user_id: fixed(&self.user_id)?,
            public_key: fixed(&self.public_key)?,
            encryption_public_key: fixed(&self.encryption_public_key)?,
            created_at_unix_ns: unsigned(self.created_at_unix_ns),
            last_authenticated_at_unix_ns: self.last_authenticated_at_unix_ns.map(unsigned),
            revoked_at_unix_ns: self.revoked_at_unix_ns.map(unsigned),
        })
    }
}

impl FriendshipDocument {
    pub(crate) fn from_record(friendship: &FriendshipRecord) -> Self {
        Self {
            user_low: binary(&friendship.edge.user_low()),
            user_high: binary(&friendship.edge.user_high()),
            requested_by: binary(&friendship.requested_by),
            state: friendship.state as i32,
            version: signed(friendship.version),
            created_at_unix_ns: signed(friendship.created_at_unix_ns),
            updated_at_unix_ns: signed(friendship.updated_at_unix_ns),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub(crate) fn into_record(self) -> Option<FriendshipRecord> {
        Some(FriendshipRecord {
            edge: FriendshipEdge::between(fixed(&self.user_low)?, fixed(&self.user_high)?).ok()?,
            requested_by: fixed(&self.requested_by)?,
            state: FriendshipState::try_from(self.state).ok()?,
            version: unsigned(self.version),
            created_at_unix_ns: unsigned(self.created_at_unix_ns),
            updated_at_unix_ns: unsigned(self.updated_at_unix_ns),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct KeyEnvelopeDocument {
    pub share_id: Binary,
    pub recipient_device_id: Binary,
    pub ephemeral_public_key: Binary,
    pub ciphertext: Binary,
    pub created_at_unix_ns: i64,
    pub schema_version: i32,
}

impl KeyEnvelopeDocument {
    pub(crate) fn from_record(envelope: &KeyEnvelopeRecord) -> Self {
        Self {
            share_id: binary(&envelope.share_id),
            recipient_device_id: binary(&envelope.recipient_device_id),
            ephemeral_public_key: binary(&envelope.ephemeral_public_key),
            ciphertext: binary(&envelope.ciphertext),
            created_at_unix_ns: signed(envelope.created_at_unix_ns),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub(crate) fn into_record(self) -> Option<KeyEnvelopeRecord> {
        Some(KeyEnvelopeRecord {
            share_id: fixed(&self.share_id)?,
            recipient_device_id: fixed(&self.recipient_device_id)?,
            ephemeral_public_key: fixed(&self.ephemeral_public_key)?,
            ciphertext: self.ciphertext.bytes,
            created_at_unix_ns: unsigned(self.created_at_unix_ns),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ShareDocument {
    #[serde(rename = "_id")]
    pub share_id: Binary,
    pub owner_user_id: Binary,
    pub revision: i64,
    pub snapshot_id: Binary,
    pub capsule: Binary,
    pub capsule_signature: Binary,
    pub created_at_unix_ns: i64,
    pub updated_at_unix_ns: i64,
    pub schema_version: i32,
}

impl ShareDocument {
    pub(crate) fn from_record(share: &ShareRecord) -> Self {
        Self {
            share_id: binary(&share.share_id),
            owner_user_id: binary(&share.owner),
            revision: signed(share.revision),
            snapshot_id: binary(&share.snapshot_id),
            capsule: binary(&share.capsule),
            capsule_signature: binary(&share.capsule_signature),
            created_at_unix_ns: signed(share.created_at_unix_ns),
            updated_at_unix_ns: signed(share.updated_at_unix_ns),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub(crate) fn into_record(self) -> Option<ShareRecord> {
        Some(ShareRecord {
            share_id: fixed(&self.share_id)?,
            owner: fixed(&self.owner_user_id)?,
            revision: unsigned(self.revision),
            snapshot_id: fixed(&self.snapshot_id)?,
            capsule: self.capsule.bytes,
            capsule_signature: self.capsule_signature.bytes,
            created_at_unix_ns: unsigned(self.created_at_unix_ns),
            updated_at_unix_ns: unsigned(self.updated_at_unix_ns),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ShareSnapshotDocument {
    pub share_id: Binary,
    pub revision: i64,
    pub snapshot_id: Binary,
    pub capsule: Binary,
    pub capsule_signature: Binary,
    pub created_at_unix_ns: i64,
    pub schema_version: i32,
}

impl ShareSnapshotDocument {
    pub(crate) fn from_record(snapshot: &ShareSnapshotRecord) -> Self {
        Self {
            share_id: binary(&snapshot.share_id),
            revision: signed(snapshot.revision),
            snapshot_id: binary(&snapshot.snapshot_id),
            capsule: binary(&snapshot.capsule),
            capsule_signature: binary(&snapshot.capsule_signature),
            created_at_unix_ns: signed(snapshot.created_at_unix_ns),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub(crate) fn into_record(self) -> Option<ShareSnapshotRecord> {
        Some(ShareSnapshotRecord {
            share_id: fixed(&self.share_id)?,
            revision: unsigned(self.revision),
            snapshot_id: fixed(&self.snapshot_id)?,
            capsule: self.capsule.bytes,
            capsule_signature: self.capsule_signature.bytes,
            created_at_unix_ns: unsigned(self.created_at_unix_ns),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ShareMembershipDocument {
    pub share_id: Binary,
    pub user_id: Binary,
    pub granted_at_unix_ns: i64,
    pub schema_version: i32,
}

impl ShareMembershipDocument {
    pub(crate) fn from_record(membership: &ShareMembershipRecord) -> Self {
        Self {
            share_id: binary(&membership.share_id),
            user_id: binary(&membership.user_id),
            granted_at_unix_ns: signed(membership.granted_at_unix_ns),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub(crate) fn into_record(self) -> Option<ShareMembershipRecord> {
        Some(ShareMembershipRecord {
            share_id: fixed(&self.share_id)?,
            user_id: fixed(&self.user_id)?,
            granted_at_unix_ns: unsigned(self.granted_at_unix_ns),
        })
    }
}

#[cfg(test)]
mod tests {
    //! `into_record` is the boundary that keeps a document malformed by
    //! outside hands — a byte length changed by a migration gone wrong, a
    //! driver bug, a hand edit — from ever reaching a typed record. Every
    //! test here confirms a specific corruption is treated as absent rather
    //! than trusted or panicked on.

    use super::*;

    const USER_ID: [u8; 16] = [1; 16];
    const OTHER_USER_ID: [u8; 16] = [2; 16];
    const DEVICE_ID: [u8; 32] = [3; 32];
    const PUBLIC_KEY: [u8; 32] = [4; 32];
    const ENCRYPTION_PUBLIC_KEY: [u8; 32] = [5; 32];
    const WRONG_LENGTH: &[u8] = &[0, 0, 0];

    fn valid_user() -> UserDocument {
        UserDocument {
            user_id: binary(&USER_ID),
            username: "Ada".to_owned(),
            normalized_username: "ada".to_owned(),
            discriminator: "7Q2XZ".to_owned(),
            created_at_unix_ns: 0,
            schema_version: SCHEMA_VERSION,
        }
    }

    fn valid_device() -> DeviceDocument {
        DeviceDocument {
            device_id: binary(&DEVICE_ID),
            user_id: binary(&USER_ID),
            public_key: binary(&PUBLIC_KEY),
            encryption_public_key: binary(&ENCRYPTION_PUBLIC_KEY),
            created_at_unix_ns: 0,
            last_authenticated_at_unix_ns: None,
            revoked_at_unix_ns: None,
            schema_version: SCHEMA_VERSION,
        }
    }

    fn valid_friendship() -> FriendshipDocument {
        FriendshipDocument {
            user_low: binary(&USER_ID),
            user_high: binary(&OTHER_USER_ID),
            requested_by: binary(&USER_ID),
            state: FriendshipState::Pending as i32,
            version: 1,
            created_at_unix_ns: 0,
            updated_at_unix_ns: 0,
            schema_version: SCHEMA_VERSION,
        }
    }

    #[test]
    fn a_well_formed_user_document_round_trips() {
        assert!(valid_user().into_record().is_some());
    }

    #[test]
    fn a_user_id_of_the_wrong_length_is_treated_as_absent() {
        let document = UserDocument {
            user_id: binary(WRONG_LENGTH),
            ..valid_user()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_well_formed_device_document_round_trips() {
        assert!(valid_device().into_record().is_some());
    }

    #[test]
    fn a_device_id_of_the_wrong_length_is_treated_as_absent() {
        let document = DeviceDocument {
            device_id: binary(WRONG_LENGTH),
            ..valid_device()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_device_user_id_of_the_wrong_length_is_treated_as_absent() {
        let document = DeviceDocument {
            user_id: binary(WRONG_LENGTH),
            ..valid_device()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_device_public_key_of_the_wrong_length_is_treated_as_absent() {
        let document = DeviceDocument {
            public_key: binary(WRONG_LENGTH),
            ..valid_device()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_device_encryption_public_key_of_the_wrong_length_is_treated_as_absent() {
        let document = DeviceDocument {
            encryption_public_key: binary(WRONG_LENGTH),
            ..valid_device()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_well_formed_friendship_document_round_trips() {
        assert!(valid_friendship().into_record().is_some());
    }

    #[test]
    fn a_friendship_user_low_of_the_wrong_length_is_treated_as_absent() {
        let document = FriendshipDocument {
            user_low: binary(WRONG_LENGTH),
            ..valid_friendship()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_friendship_user_high_of_the_wrong_length_is_treated_as_absent() {
        let document = FriendshipDocument {
            user_high: binary(WRONG_LENGTH),
            ..valid_friendship()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_friendship_between_a_user_and_itself_is_treated_as_absent() {
        // `FriendshipEdge::between` refuses two equal halves; a document that
        // somehow stored one is corrupt the same way a bad length is.
        let document = FriendshipDocument {
            user_high: binary(&USER_ID),
            ..valid_friendship()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_friendship_requested_by_of_the_wrong_length_is_treated_as_absent() {
        let document = FriendshipDocument {
            requested_by: binary(WRONG_LENGTH),
            ..valid_friendship()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_friendship_state_outside_the_known_values_is_treated_as_absent() {
        let document = FriendshipDocument {
            state: 99,
            ..valid_friendship()
        };
        assert_eq!(document.into_record(), None);
    }

    const SHARE_ID: [u8; 16] = [6; 16];

    fn valid_key_envelope() -> KeyEnvelopeDocument {
        KeyEnvelopeDocument {
            share_id: binary(&SHARE_ID),
            recipient_device_id: binary(&DEVICE_ID),
            ephemeral_public_key: binary(&ENCRYPTION_PUBLIC_KEY),
            ciphertext: binary(b"sealed"),
            created_at_unix_ns: 0,
            schema_version: SCHEMA_VERSION,
        }
    }

    #[test]
    fn a_well_formed_key_envelope_document_round_trips() {
        let record = valid_key_envelope().into_record().expect("well formed");
        assert_eq!(record.share_id, SHARE_ID);
        assert_eq!(record.recipient_device_id, DEVICE_ID);
        assert_eq!(record.ephemeral_public_key, ENCRYPTION_PUBLIC_KEY);
        assert_eq!(record.ciphertext, b"sealed");
    }

    #[test]
    fn a_key_envelope_share_id_of_the_wrong_length_is_treated_as_absent() {
        let document = KeyEnvelopeDocument {
            share_id: binary(WRONG_LENGTH),
            ..valid_key_envelope()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_key_envelope_recipient_device_id_of_the_wrong_length_is_treated_as_absent() {
        let document = KeyEnvelopeDocument {
            recipient_device_id: binary(WRONG_LENGTH),
            ..valid_key_envelope()
        };
        assert_eq!(document.into_record(), None);
    }

    #[test]
    fn a_key_envelope_ephemeral_public_key_of_the_wrong_length_is_treated_as_absent() {
        let document = KeyEnvelopeDocument {
            ephemeral_public_key: binary(WRONG_LENGTH),
            ..valid_key_envelope()
        };
        assert_eq!(document.into_record(), None);
    }
}
