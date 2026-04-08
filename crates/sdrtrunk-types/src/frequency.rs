//! Radio frequency types.

use crate::error::ValidationError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Radio frequency in Hz.
///
/// Wraps an i64 with validation:
/// - Must be positive (> 0)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Frequency(i64);

impl Frequency {
    /// Creates a validated Frequency.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidFrequency`] if value is not positive.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdrtrunk_types::Frequency;
    ///
    /// let freq = Frequency::new(851_012_500).unwrap();
    /// assert_eq!(freq.as_hz(), 851_012_500);
    ///
    /// assert!(Frequency::new(0).is_err());
    /// ```
    pub const fn new(hz: i64) -> Result<Self, ValidationError> {
        if hz <= 0 {
            return Err(ValidationError::InvalidFrequency { value: hz });
        }
        Ok(Self(hz))
    }

    /// Returns the frequency in Hz.
    #[must_use]
    pub const fn as_hz(self) -> i64 {
        self.0
    }
}

impl fmt::Display for Frequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Hz", self.0)
    }
}

#[cfg(feature = "sqlx")]
mod sqlx_impl {
    use super::Frequency;
    use sqlx::encode::IsNull;
    use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
    use sqlx::{Decode, Encode, Postgres, Type};

    impl Type<Postgres> for Frequency {
        fn type_info() -> PgTypeInfo {
            <i64 as Type<Postgres>>::type_info()
        }
    }

    impl<'r> Decode<'r, Postgres> for Frequency {
        fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
            let v = <i64 as Decode<'_, Postgres>>::decode(value)?;
            Ok(Self(v))
        }
    }

    impl Encode<'_, Postgres> for Frequency {
        fn encode_by_ref(
            &self,
            buf: &mut PgArgumentBuffer,
        ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
            <i64 as Encode<'_, Postgres>>::encode_by_ref(&self.0, buf)
        }
    }
}
