//! Talkgroup identifier types.

use crate::error::ValidationError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Talkgroup identifier newtype.
///
/// Wraps an i32 with validation:
/// - Must be positive (> 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TalkgroupId(i32);

impl TalkgroupId {
    /// Creates a validated `TalkgroupId`.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidTalkgroupId`] if value is not positive.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdrtrunk_types::TalkgroupId;
    ///
    /// let id = TalkgroupId::new(12345).unwrap();
    /// assert_eq!(id.as_i32(), 12345);
    /// ```
    pub const fn new(value: i32) -> Result<Self, ValidationError> {
        if value <= 0 {
            return Err(ValidationError::InvalidTalkgroupId { value });
        }
        Ok(Self(value))
    }

    /// Returns the talkgroup ID as an i32.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.0
    }
}

impl fmt::Display for TalkgroupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "sqlx")]
mod sqlx_impl {
    use super::TalkgroupId;
    use sqlx::encode::IsNull;
    use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
    use sqlx::{Decode, Encode, Postgres, Type};

    impl Type<Postgres> for TalkgroupId {
        fn type_info() -> PgTypeInfo {
            <i32 as Type<Postgres>>::type_info()
        }
    }

    impl<'r> Decode<'r, Postgres> for TalkgroupId {
        fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
            let v = <i32 as Decode<'_, Postgres>>::decode(value)?;
            Ok(Self(v))
        }
    }

    impl Encode<'_, Postgres> for TalkgroupId {
        fn encode_by_ref(
            &self,
            buf: &mut PgArgumentBuffer,
        ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
            <i32 as Encode<'_, Postgres>>::encode_by_ref(&self.0, buf)
        }
    }
}

/// Talkgroup label newtype (max 255 chars).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TalkgroupLabel(String);

impl TalkgroupLabel {
    /// Creates a `TalkgroupLabel`.
    #[must_use]
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Returns the label as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
