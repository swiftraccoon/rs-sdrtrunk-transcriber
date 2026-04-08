#![allow(
    missing_docs,
    unused_results,
    clippy::unwrap_used,
    clippy::missing_panics_doc
)]

use sdrtrunk_types::{SystemId, ValidationError};

#[test]
fn test_system_id_valid() {
    let id = SystemId::new("police-dispatch").unwrap();
    assert_eq!(id.as_str(), "police-dispatch");
}

#[test]
fn test_system_id_empty_rejected() {
    let result = SystemId::new("");
    assert!(matches!(result, Err(ValidationError::EmptySystemId)));
}

#[test]
fn test_system_id_too_long_rejected() {
    let long_id = "a".repeat(51);
    let result = SystemId::new(long_id);
    assert!(matches!(
        result,
        Err(ValidationError::SystemIdTooLong {
            length: 51,
            max: 50
        })
    ));
}

#[test]
fn test_system_id_display() {
    let id = SystemId::new("test").unwrap();
    assert_eq!(format!("{id}"), "test");
}
