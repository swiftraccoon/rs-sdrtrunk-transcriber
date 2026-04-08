#![allow(
    missing_docs,
    unused_results,
    clippy::unwrap_used,
    clippy::missing_panics_doc
)]

use sdrtrunk_types::{RadioId, TalkgroupId, ValidationError};

#[test]
fn test_talkgroup_id_valid() {
    let id = TalkgroupId::new(12345).unwrap();
    assert_eq!(id.as_i32(), 12345);
}

#[test]
fn test_talkgroup_id_zero_rejected() {
    let result = TalkgroupId::new(0);
    assert!(matches!(
        result,
        Err(ValidationError::InvalidTalkgroupId { value: 0 })
    ));
}

#[test]
fn test_talkgroup_id_negative_rejected() {
    let result = TalkgroupId::new(-1);
    assert!(matches!(
        result,
        Err(ValidationError::InvalidTalkgroupId { value: -1 })
    ));
}

#[test]
fn test_radio_id_valid() {
    let id = RadioId::new(98765).unwrap();
    assert_eq!(id.as_i32(), 98765);
}

#[test]
fn test_radio_id_negative_rejected() {
    let result = RadioId::new(-1);
    assert!(matches!(
        result,
        Err(ValidationError::InvalidRadioId { value: -1 })
    ));
}
