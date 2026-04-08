//! System identifier types.

use crate::error::ValidationError;
use serde::{Deserialize, Serialize};
use std::fmt;

/// System identifier newtype.
///
/// Wraps a String with validation:
/// - Cannot be empty
/// - Maximum 50 characters
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SystemId(String);

impl SystemId {
    /// Creates a validated `SystemId`.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::EmptySystemId`] if the string is empty.
    /// Returns [`ValidationError::SystemIdTooLong`] if length exceeds 50 characters.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdrtrunk_types::SystemId;
    ///
    /// let id = SystemId::new("police-dispatch").unwrap();
    /// assert_eq!(id.as_str(), "police-dispatch");
    /// ```
    pub fn new(s: impl Into<String>) -> Result<Self, ValidationError> {
        let s = s.into();
        if s.is_empty() {
            return Err(ValidationError::EmptySystemId);
        }
        if s.len() > 50 {
            return Err(ValidationError::SystemIdTooLong {
                length: s.len(),
                max: 50,
            });
        }
        Ok(Self(s))
    }

    /// Returns the system ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Consumes the wrapper and returns the inner String.
    #[must_use]
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl fmt::Display for SystemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "sqlx")]
mod sqlx_impl {
    use super::SystemId;
    use sqlx::encode::IsNull;
    use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
    use sqlx::{Decode, Encode, Postgres, Type};

    impl Type<Postgres> for SystemId {
        fn type_info() -> PgTypeInfo {
            <String as Type<Postgres>>::type_info()
        }

        fn compatible(ty: &PgTypeInfo) -> bool {
            <String as Type<Postgres>>::compatible(ty)
        }
    }

    impl<'r> Decode<'r, Postgres> for SystemId {
        fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
            let s = <String as Decode<'_, Postgres>>::decode(value)?;
            Ok(Self(s))
        }
    }

    impl Encode<'_, Postgres> for SystemId {
        fn encode_by_ref(
            &self,
            buf: &mut PgArgumentBuffer,
        ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
            <String as Encode<'_, Postgres>>::encode_by_ref(&self.0, buf)
        }
    }
}
