//! User handles: `<username>#<discriminator>`.
//!
//! A handle is what people type at each other. The username keeps the casing
//! its owner chose, while lookups use a normalized form so `Ada` and `ada`
//! cannot both be claimed. The discriminator is random rather than sequential,
//! so allocation never scans for the next free value and knowing one handle
//! tells you nothing about which others exist.

use std::fmt;

use portalis_nexus_protocol::{DISCRIMINATOR_CHARS, MAX_USERNAME_CHARS, MIN_USERNAME_CHARS};
use thiserror::Error;

/// Crockford Base32, which omits I, L, O, and U so handles cannot be misread
/// or spell unintended words.
const DISCRIMINATOR_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
pub const HANDLE_SEPARATOR: char = '#';

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HandleError {
    #[error("username must be at least {MIN_USERNAME_CHARS} characters, got {actual}")]
    UsernameTooShort { actual: usize },
    #[error("username must be at most {MAX_USERNAME_CHARS} characters, got {actual}")]
    UsernameTooLong { actual: usize },
    #[error("username may only contain letters, digits, and underscores")]
    UsernameCharset,
    #[error("discriminator must be {DISCRIMINATOR_CHARS} Crockford Base32 characters")]
    InvalidDiscriminator,
    #[error("a handle must read as <username>{HANDLE_SEPARATOR}<discriminator>")]
    Malformed,
}

/// A validated user handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Handle {
    username: String,
    normalized_username: String,
    discriminator: String,
}

impl Handle {
    /// Validates a username and pairs it with an existing discriminator.
    ///
    /// # Errors
    ///
    /// Returns [`HandleError`] when the username breaks the charset or length
    /// rules, or the discriminator is not Crockford Base32 of the right size.
    pub fn new(username: &str, discriminator: &str) -> Result<Self, HandleError> {
        validate_username(username)?;
        validate_discriminator(discriminator)?;
        Ok(Self {
            username: username.to_owned(),
            normalized_username: normalize_username(username),
            discriminator: discriminator.to_ascii_uppercase(),
        })
    }

    /// Parses a handle written as `<username>#<discriminator>`.
    ///
    /// # Errors
    ///
    /// Returns [`HandleError`] when the text has no separator or either half
    /// is invalid.
    pub fn parse(handle: &str) -> Result<Self, HandleError> {
        let (username, discriminator) = handle
            .split_once(HANDLE_SEPARATOR)
            .ok_or(HandleError::Malformed)?;
        Self::new(username, discriminator)
    }

    /// The username as its owner typed it.
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// The form stored and indexed for lookups.
    #[must_use]
    pub fn normalized_username(&self) -> &str {
        &self.normalized_username
    }

    #[must_use]
    pub fn discriminator(&self) -> &str {
        &self.discriminator
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}{HANDLE_SEPARATOR}{}",
            self.username, self.discriminator
        )
    }
}

/// Folds a username to the form used for uniqueness and lookup.
#[must_use]
pub fn normalize_username(username: &str) -> String {
    username.to_lowercase()
}

/// Checks a username against the charset and length rules.
///
/// # Errors
///
/// Returns [`HandleError`] when the username is too short, too long, or
/// contains anything but letters, digits, and underscores.
pub fn validate_username(username: &str) -> Result<(), HandleError> {
    // Counted in characters, not bytes, so multi-byte letters are not
    // penalised twice.
    let length = username.chars().count();
    if length < MIN_USERNAME_CHARS {
        return Err(HandleError::UsernameTooShort { actual: length });
    }
    if length > MAX_USERNAME_CHARS {
        return Err(HandleError::UsernameTooLong { actual: length });
    }
    if !username
        .chars()
        .all(|character| character.is_alphanumeric() || character == '_')
    {
        return Err(HandleError::UsernameCharset);
    }
    Ok(())
}

/// Checks a discriminator against the Crockford Base32 alphabet.
///
/// # Errors
///
/// Returns [`HandleError::InvalidDiscriminator`] when the length or alphabet
/// is wrong.
pub fn validate_discriminator(discriminator: &str) -> Result<(), HandleError> {
    if discriminator.chars().count() != DISCRIMINATOR_CHARS {
        return Err(HandleError::InvalidDiscriminator);
    }
    // Byte-wise is enough: the alphabet is ASCII, so any byte of a multi-byte
    // character fails the membership test on its own.
    if !discriminator
        .bytes()
        .all(|byte| DISCRIMINATOR_ALPHABET.contains(&byte.to_ascii_uppercase()))
    {
        return Err(HandleError::InvalidDiscriminator);
    }
    Ok(())
}

/// Renders random bytes as a discriminator.
///
/// Callers supply the entropy so allocation stays deterministic under test.
/// Bytes beyond [`DISCRIMINATOR_CHARS`] are ignored, and missing bytes are
/// treated as zero, so a short slice still yields a well-formed value.
#[must_use]
pub fn discriminator_from_entropy(entropy: &[u8]) -> String {
    (0..DISCRIMINATOR_CHARS)
        .map(|index| {
            let byte = entropy.get(index).copied().unwrap_or_default();
            // Masking to 5 bits maps each byte onto the 32-character alphabet.
            char::from(DISCRIMINATOR_ALPHABET[usize::from(byte & 0b0001_1111)])
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_renders_a_handle() {
        let handle = Handle::new("Ada", "7Q2XZ").expect("valid handle");

        assert_eq!(handle.username(), "Ada");
        assert_eq!(handle.normalized_username(), "ada");
        assert_eq!(handle.discriminator(), "7Q2XZ");
        assert_eq!(handle.to_string(), "Ada#7Q2XZ");
    }

    #[test]
    fn refuses_to_build_a_handle_from_invalid_parts() {
        assert_eq!(
            Handle::new("ad", "7Q2XZ"),
            Err(HandleError::UsernameTooShort { actual: 2 })
        );
        assert_eq!(
            Handle::new("ada", "7Q2X"),
            Err(HandleError::InvalidDiscriminator)
        );
    }

    #[test]
    fn normalizes_case_so_one_name_cannot_be_claimed_twice() {
        let first = Handle::new("Ada", "7Q2XZ").expect("valid handle");
        let second = Handle::new("aDA", "7Q2XZ").expect("valid handle");

        assert_ne!(first.username(), second.username());
        assert_eq!(first.normalized_username(), second.normalized_username());
    }

    #[test]
    fn parses_a_written_handle_and_uppercases_its_discriminator() {
        let handle = Handle::parse("Ada#7q2xz").expect("valid handle");

        assert_eq!(handle.username(), "Ada");
        assert_eq!(handle.discriminator(), "7Q2XZ");
        assert_eq!(Handle::parse("Ada"), Err(HandleError::Malformed));
    }

    #[test]
    fn rejects_usernames_outside_the_length_bounds() {
        assert_eq!(
            validate_username("ad"),
            Err(HandleError::UsernameTooShort { actual: 2 })
        );
        assert_eq!(validate_username("ada"), Ok(()));
        assert_eq!(validate_username(&"a".repeat(MAX_USERNAME_CHARS)), Ok(()));
        assert_eq!(
            validate_username(&"a".repeat(MAX_USERNAME_CHARS + 1)),
            Err(HandleError::UsernameTooLong {
                actual: MAX_USERNAME_CHARS + 1
            })
        );
    }

    #[test]
    fn rejects_usernames_outside_the_charset() {
        assert_eq!(validate_username("ada_99"), Ok(()));
        assert_eq!(
            validate_username("ada lovelace"),
            Err(HandleError::UsernameCharset)
        );
        assert_eq!(
            validate_username("ada#99"),
            Err(HandleError::UsernameCharset)
        );
        assert_eq!(validate_username("ada!"), Err(HandleError::UsernameCharset));
    }

    #[test]
    fn rejects_discriminators_outside_the_alphabet_or_length() {
        assert_eq!(validate_discriminator("7Q2XZ"), Ok(()));
        assert_eq!(
            validate_discriminator("7Q2X"),
            Err(HandleError::InvalidDiscriminator)
        );
        assert_eq!(
            validate_discriminator("7Q2XZ1"),
            Err(HandleError::InvalidDiscriminator)
        );
        // I, L, O, and U are excluded to avoid misreading.
        for excluded in ["7Q2XI", "7Q2XL", "7Q2XO", "7Q2XU", "7Q2X-"] {
            assert_eq!(
                validate_discriminator(excluded),
                Err(HandleError::InvalidDiscriminator),
                "{excluded} should be rejected"
            );
        }
    }

    #[test]
    fn renders_discriminators_from_entropy() {
        assert_eq!(discriminator_from_entropy(&[0, 1, 2, 3, 4]), "01234");
        // High bits are masked away rather than overflowing the alphabet.
        assert_eq!(discriminator_from_entropy(&[0xff; 5]), "ZZZZZ");
        // A short slice still yields a well-formed discriminator.
        assert_eq!(discriminator_from_entropy(&[]), "00000");
        assert_eq!(
            validate_discriminator(&discriminator_from_entropy(&[9, 30, 17, 4, 22, 99])),
            Ok(())
        );
    }
}
