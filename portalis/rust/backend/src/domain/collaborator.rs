use super::identity::DeviceId;

/// A collaborator's standing within one collection. Note this is *not*
/// cryptographically enforced yet — see the backend README's "moderation
/// semantics" open question. For now it's locally-tracked metadata.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Role {
    Admin,
    Member,
}

#[derive(Clone, Debug)]
pub(crate) struct Collaborator {
    pub device_id: DeviceId,
    pub display_name: String,
    pub role: Role,
    pub joined_at_unix_ms: i64,
}

impl Collaborator {
    pub fn new(
        device_id: DeviceId,
        display_name: String,
        role: Role,
        joined_at_unix_ms: i64,
    ) -> Self {
        Self {
            device_id,
            display_name,
            role,
            joined_at_unix_ms,
        }
    }

    pub fn is_admin(&self) -> bool {
        matches!(self.role, Role::Admin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::identity::DeviceIdentity;

    #[test]
    fn is_admin_reflects_role() {
        let device_id = DeviceIdentity::generate().device_id();
        let admin = Collaborator::new(device_id, "Maya".into(), Role::Admin, 0);
        let member = Collaborator::new(device_id, "Theo".into(), Role::Member, 0);

        assert!(admin.is_admin());
        assert!(!member.is_admin());
    }
}
