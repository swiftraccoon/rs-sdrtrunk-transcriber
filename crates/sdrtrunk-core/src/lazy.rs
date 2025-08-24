//! Lazy initialization utilities (replacement for `once_cell`)
//!
//! This module provides `std::sync::LazyLock` re-exports and helper utilities
//! for lazy initialization patterns commonly used in the codebase.

/// Re-export of `std::sync::LazyLock` for consistent imports
pub use std::sync::LazyLock;

/// Re-export of `std::sync::OnceLock` for single-use initialization
pub use std::sync::OnceLock;

/// Helper macro for creating static lazy values
///
/// # Examples
///
/// ```
/// use sdrtrunk_core::lazy_static;
/// use std::collections::HashMap;
///
/// lazy_static! {
///     static ref CONFIG: HashMap<String, String> = HashMap::new();
/// }
///
/// // Access the lazy static
/// assert!(CONFIG.is_empty());
/// ```
#[macro_export]
macro_rules! lazy_static {
    ($(static ref $name:ident: $type:ty = $init:expr;)*) => {
        $(
            static $name: std::sync::LazyLock<$type> = std::sync::LazyLock::new(|| $init);
        )*
    };
}

/// Create a lazy static value
///
/// # Examples
///
/// ```
/// use sdrtrunk_core::lazy::LazyLock;
/// use std::collections::HashMap;
///
/// static CACHE: LazyLock<HashMap<String, i32>> = LazyLock::new(HashMap::new);
///
/// // Access the lazy static
/// assert!(CACHE.is_empty());
/// ```
#[must_use]
pub fn lazy<T>(init: fn() -> T) -> LazyLock<T> {
    LazyLock::new(init)
}

/// Create a once lock for single initialization
///
/// # Examples
///
/// ```
/// use sdrtrunk_core::lazy::OnceLock;
///
/// static LOGGER: OnceLock<String> = OnceLock::new();
/// ```
#[must_use]
pub const fn once<T>() -> OnceLock<T> {
    OnceLock::new()
}

#[cfg(test)]
#[allow(clippy::missing_panics_doc)]
#[allow(
    clippy::missing_panics_doc,
    clippy::uninlined_format_args,
    clippy::needless_collect
)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn test_lazy_lock_initialization() {
        static COUNTER: LazyLock<AtomicUsize> = LazyLock::new(|| AtomicUsize::new(42));

        assert_eq!(COUNTER.load(Ordering::SeqCst), 42);
        COUNTER.store(100, Ordering::SeqCst);
        assert_eq!(COUNTER.load(Ordering::SeqCst), 100);
    }

    #[test]
    fn test_once_lock_initialization() {
        static VALUE: OnceLock<String> = OnceLock::new();

        assert!(VALUE.get().is_none());
        VALUE.set("hello".to_string()).unwrap();
        assert_eq!(VALUE.get(), Some(&"hello".to_string()));

        // Second set should fail
        assert!(VALUE.set("world".to_string()).is_err());
    }

    #[test]
    fn test_lazy_static_macro() {
        lazy_static! {
            static ref TEST_VALUE: String = "test".to_string();
            static ref TEST_COUNTER: AtomicUsize = AtomicUsize::new(0);
        }

        assert_eq!(TEST_VALUE.as_str(), "test");
        assert_eq!(TEST_COUNTER.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn test_lazy_helper() {
        fn init_cache() -> Vec<i32> {
            vec![1, 2, 3]
        }
        static CACHE: LazyLock<Vec<i32>> = LazyLock::new(init_cache);

        assert_eq!(CACHE.as_slice(), &[1, 2, 3]);
    }

    #[test]
    fn test_once_helper() {
        static CONFIG: OnceLock<String> = OnceLock::new();

        assert!(CONFIG.get().is_none());
        CONFIG.set("config".to_string()).unwrap();
        assert_eq!(CONFIG.get(), Some(&"config".to_string()));
    }

    #[test]
    fn test_multiple_access_lazy() {
        static SHARED: LazyLock<Vec<String>> = LazyLock::new(|| vec!["shared".to_string()]);

        // Multiple threads accessing the same lazy value
        let handles: Vec<_> = (0..10)
            .map(|_| std::thread::spawn(|| SHARED.len()))
            .collect();

        for handle in handles {
            assert_eq!(handle.join().unwrap(), 1);
        }
    }

    #[test]
    fn test_lazy_with_expensive_computation() {
        static EXPENSIVE: LazyLock<i32> = LazyLock::new(|| {
            // Simulate expensive computation
            std::thread::sleep(std::time::Duration::from_millis(1));
            42
        });

        let start = std::time::Instant::now();
        let value1 = *EXPENSIVE;
        let first_duration = start.elapsed();

        let start2 = std::time::Instant::now();
        let value2 = *EXPENSIVE;
        let second_duration = start2.elapsed();

        assert_eq!(value1, 42);
        assert_eq!(value2, 42);

        // Second access should be much faster (already computed)
        assert!(second_duration < first_duration);
    }

    #[test]
    fn test_once_lock_thread_safety() {
        use std::thread;

        static ONCE_VALUE: OnceLock<String> = OnceLock::new();

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    // Only the first thread should succeed in setting the value
                    ONCE_VALUE.set(format!("thread-{}", i)).is_ok()
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Exactly one thread should have succeeded
        assert_eq!(results.iter().filter(|&&success| success).count(), 1);

        // Value should be set by one of the threads
        assert!(ONCE_VALUE.get().is_some());
        assert!(ONCE_VALUE.get().unwrap().starts_with("thread-"));
    }
}
