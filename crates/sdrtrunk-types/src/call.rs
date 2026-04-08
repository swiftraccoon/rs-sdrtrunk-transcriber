//! Call identifier and radio call types.

use crate::{Frequency, RadioId, SystemId, TalkgroupId, TalkgroupLabel};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

/// Call identifier newtype (transparent UUID wrapper).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CallId(Uuid);

impl CallId {
    /// Creates a new random `CallId`.
    ///
    /// # Examples
    ///
    /// ```
    /// use sdrtrunk_types::CallId;
    ///
    /// let id = CallId::new();
    /// // Each call produces a unique ID
    /// assert_ne!(id.as_uuid(), CallId::new().as_uuid());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Creates a `CallId` from an existing UUID.
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    /// Returns the inner UUID.
    #[must_use]
    pub const fn as_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for CallId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for CallId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(feature = "sqlx")]
mod sqlx_impl {
    use super::CallId;
    use sqlx::encode::IsNull;
    use sqlx::postgres::{PgArgumentBuffer, PgTypeInfo, PgValueRef};
    use sqlx::{Decode, Encode, Postgres, Type};
    use uuid::Uuid;

    impl Type<Postgres> for CallId {
        fn type_info() -> PgTypeInfo {
            <Uuid as Type<Postgres>>::type_info()
        }
    }

    impl<'r> Decode<'r, Postgres> for CallId {
        fn decode(value: PgValueRef<'r>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
            let uuid = <Uuid as Decode<'_, Postgres>>::decode(value)?;
            Ok(Self(uuid))
        }
    }

    impl Encode<'_, Postgres> for CallId {
        fn encode_by_ref(
            &self,
            buf: &mut PgArgumentBuffer,
        ) -> Result<IsNull, Box<dyn std::error::Error + Send + Sync>> {
            <Uuid as Encode<'_, Postgres>>::encode_by_ref(&self.0, buf)
        }
    }
}

/// Represents a radio call with validated types.
///
/// This is the canonical representation of a radio call in the system.
/// All fields that represent domain identifiers use validated newtypes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadioCall {
    /// Unique call identifier
    pub id: Option<CallId>,
    /// When the call was created in our system
    pub created_at: DateTime<Utc>,
    /// When the call actually occurred
    pub call_timestamp: DateTime<Utc>,
    /// System identifier (validated)
    pub system_id: SystemId,
    /// Optional system label
    pub system_label: Option<String>,
    /// Frequency (validated)
    pub frequency: Option<Frequency>,
    /// Talkgroup ID (validated)
    pub talkgroup_id: Option<TalkgroupId>,
    /// Talkgroup label
    pub talkgroup_label: Option<TalkgroupLabel>,
    /// Talkgroup group
    pub talkgroup_group: Option<String>,
    /// Talkgroup tag
    pub talkgroup_tag: Option<String>,
    /// Source radio ID (validated)
    pub source_radio_id: Option<RadioId>,
    /// Talker alias
    pub talker_alias: Option<String>,
    /// Audio filename
    pub audio_filename: Option<String>,
    /// Audio file path
    pub audio_path: Option<String>,
    /// Audio duration in seconds
    pub audio_duration_seconds: Option<f64>,
    /// Audio file size in bytes
    pub audio_file_size_bytes: Option<i64>,
    /// Transcription status
    pub transcription_status: String,
    /// Transcription text
    pub transcription_text: Option<String>,
    /// Transcription confidence
    pub transcription_confidence: Option<f64>,
    /// Transcription error
    pub transcription_error: Option<String>,
}
