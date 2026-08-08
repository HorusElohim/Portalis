use portalis_nexus_protocol::v1::ProtocolRange;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolPolicy {
    supported: ProtocolRange,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum NegotiationError {
    #[error("protocol range minimum {minimum} exceeds maximum {maximum}")]
    InvalidRange { minimum: u32, maximum: u32 },
    #[error("client and server have no protocol version in common")]
    Incompatible,
}

impl ProtocolPolicy {
    /// Creates a server protocol policy with an inclusive version range.
    ///
    /// # Errors
    ///
    /// Returns [`NegotiationError::InvalidRange`] when `minimum` exceeds
    /// `maximum`.
    pub fn new(minimum: u32, maximum: u32) -> Result<Self, NegotiationError> {
        validate_range(minimum, maximum)?;
        Ok(Self {
            supported: ProtocolRange { minimum, maximum },
        })
    }

    #[must_use]
    pub fn supported(&self) -> &ProtocolRange {
        &self.supported
    }

    /// Chooses the highest protocol version supported by both peers.
    ///
    /// # Errors
    ///
    /// Returns [`NegotiationError::InvalidRange`] for a malformed client
    /// range or [`NegotiationError::Incompatible`] when no version overlaps.
    pub fn negotiate(&self, client: &ProtocolRange) -> Result<u32, NegotiationError> {
        validate_range(client.minimum, client.maximum)?;
        let minimum = self.supported.minimum.max(client.minimum);
        let maximum = self.supported.maximum.min(client.maximum);
        if minimum > maximum {
            return Err(NegotiationError::Incompatible);
        }
        Ok(maximum)
    }
}

fn validate_range(minimum: u32, maximum: u32) -> Result<(), NegotiationError> {
    if minimum > maximum {
        return Err(NegotiationError::InvalidRange { minimum, maximum });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_server_range() {
        assert_eq!(
            ProtocolPolicy::new(2, 1),
            Err(NegotiationError::InvalidRange {
                minimum: 2,
                maximum: 1,
            })
        );
    }

    #[test]
    fn rejects_invalid_client_range() {
        let policy = ProtocolPolicy::new(1, 2).expect("valid policy");

        assert_eq!(
            policy.negotiate(&ProtocolRange {
                minimum: 3,
                maximum: 2,
            }),
            Err(NegotiationError::InvalidRange {
                minimum: 3,
                maximum: 2,
            })
        );
    }

    #[test]
    fn chooses_highest_common_version() {
        let policy = ProtocolPolicy::new(1, 4).expect("valid policy");

        assert_eq!(
            policy.negotiate(&ProtocolRange {
                minimum: 2,
                maximum: 3,
            }),
            Ok(3)
        );
        assert_eq!(
            policy.supported(),
            &ProtocolRange {
                minimum: 1,
                maximum: 4,
            }
        );
    }

    #[test]
    fn rejects_incompatible_ranges() {
        let policy = ProtocolPolicy::new(3, 4).expect("valid policy");

        assert_eq!(
            policy.negotiate(&ProtocolRange {
                minimum: 1,
                maximum: 2,
            }),
            Err(NegotiationError::Incompatible)
        );
    }
}
