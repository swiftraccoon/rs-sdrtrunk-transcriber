//! Radio identifier types.

use crate::error::ValidationError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Radio identifier newtype.
///
/// Wraps an i32 with validation:
/// - Must be positive (> 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RadioId(i32);

impl RadioId {
    /// Creates a validated `RadioId`.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidRadioId`] if value is not positive.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdrtrunk_types::RadioId;
    ///
    /// let id = RadioId::new(98765).unwrap();
    /// assert_eq!(id.as_i32(), 98765);
    /// ```
    pub const fn new(value: i32) -> Result<Self, ValidationError> {
        if value <= 0 {
            return Err(ValidationError::InvalidRadioId { value });
        }
        Ok(Self(value))
    }

    /// Returns the radio ID as an i32.
    #[must_use]
    pub const fn as_i32(self) -> i32 {
        self.0
    }
}

impl fmt::Display for RadioId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "sqlx")]
mod sqlx_impl {
    use super::RadioId;
    use sqlx::encode::IsNull;
    use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
    use sqlx::{Decode, Encode, Postgres, Type};

    impl Type<Postgres> for RadioId {
        fn type_info() -> PgTypeInfo {
            <i32 as Type<Postgres>>::type_info()
        }
    }

    impl<'r> Decode<'r, Postgres> for RadioId {
        fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
            let v = <i32 as Decode<'_, Postgres>>::decode(value)?;
            Ok(Self(v))
        }
    }

    impl Encode<'_, Postgres> for RadioId {
        fn encode_by_ref(
            &self,
            buf: &mut PgArgumentBuffer,
        ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
            <i32 as Encode<'_, Postgres>>::encode_by_ref(&self.0, buf)
        }
    }
}
