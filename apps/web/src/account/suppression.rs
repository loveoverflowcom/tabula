//! Non-authorizing per-target logout marker protocol (ADR-0031 §6).
//!
//! Immutable independent keys avoid a cross-document read/modify/write register.
//! A matching durable logout receipt can remove only its own target. This is
//! local suppression, never atomic global output fencing or revocation authority.

use super::core::{decode_logout_fingerprint, InvalidLogoutIntent};

/// Same-origin marker namespace; the suffix is one canonical fingerprint.
pub const PREFIX: &str = "tabula-logout-suppression-v1:";
/// Marker payload has no credential, identity or authorization meaning.
pub const VALUE: &str = "v1";

/// Minimal synchronous storage boundary for the per-target marker protocol.
pub trait MarkerStorage {
    fn get(&self, key: &str) -> Result<Option<String>, InvalidLogoutIntent>;
    fn set(&self, key: &str, value: &str) -> Result<(), InvalidLogoutIntent>;
    fn remove(&self, key: &str) -> Result<(), InvalidLogoutIntent>;
}

/// Keys hold only a bounded SHA-256 fingerprint, never identity or credentials.
pub fn key(intent: &str) -> Result<String, InvalidLogoutIntent> {
    decode_logout_fingerprint(intent)?;
    Ok(format!("{PREFIX}{}", &intent[3..]))
}

/// Save only this target and verify the write; another target is never overwritten.
pub fn save(storage: &impl MarkerStorage, intent: &str) -> Result<(), InvalidLogoutIntent> {
    let key = key(intent)?;
    storage.set(&key, VALUE)?;
    if storage.get(&key)?.as_deref() != Some(VALUE) {
        return Err(InvalidLogoutIntent);
    }
    Ok(())
}

/// Remove only the target named by an acknowledged, generation-current receipt.
pub fn clear(storage: &impl MarkerStorage, intent: &str) -> Result<(), InvalidLogoutIntent> {
    let key = key(intent)?;
    storage.remove(&key)?;
    if storage.get(&key)?.is_some() {
        return Err(InvalidLogoutIntent);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::core::AccountCore;
    use super::*;
    use std::{cell::RefCell, collections::BTreeMap};

    #[derive(Default)]
    struct Storage(RefCell<BTreeMap<String, String>>);
    impl MarkerStorage for Storage {
        fn get(&self, key: &str) -> Result<Option<String>, InvalidLogoutIntent> {
            Ok(self.0.borrow().get(key).cloned())
        }
        fn set(&self, key: &str, value: &str) -> Result<(), InvalidLogoutIntent> {
            self.0.borrow_mut().insert(key.to_owned(), value.to_owned());
            Ok(())
        }
        fn remove(&self, key: &str) -> Result<(), InvalidLogoutIntent> {
            self.0.borrow_mut().remove(key);
            Ok(())
        }
    }

    #[test]
    fn every_cross_document_save_clear_order_preserves_unrevoked_other_target() {
        let a = format!("v1:{}", "a".repeat(64));
        let b = format!("v1:{}", "b".repeat(64));
        // Exhaust all six interleavings of writes from documents A/B and a
        // durable receipt for A. Receipt A is already server-ordered; local
        // Save A may lag it and leave a conservative availability-only marker.
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let storage = Storage::default();
            for event in order {
                match event {
                    0 => save(&storage, &a).unwrap(),
                    1 => save(&storage, &b).unwrap(),
                    2 => clear(&storage, &a).unwrap(),
                    _ => unreachable!(),
                }
            }
            assert_eq!(
                storage.get(&key(&b).unwrap()).unwrap().as_deref(),
                Some(VALUE)
            );
            // Every surviving marker is only a local fail-closed hint.
            let mut reloaded = AccountCore::default();
            for key in storage.0.borrow().keys() {
                reloaded
                    .restore_logout_intent(Some(&format!(
                        "v1:{}",
                        key.strip_prefix(PREFIX).unwrap()
                    )))
                    .unwrap();
            }
            assert!(!reloaded.snapshot().login_available);
            assert!(reloaded.login().is_none());
            assert_eq!(
                reloaded.snapshot().status,
                super::super::core::AccountStatus::LogoutPending
            );
        }
    }

    #[test]
    fn target_receipt_cannot_remove_another_marker_or_clear_the_origin() {
        let a = format!("v1:{}", "a".repeat(64));
        let b = format!("v1:{}", "b".repeat(64));
        let storage = Storage::default();
        save(&storage, &a).unwrap();
        save(&storage, &b).unwrap();
        storage.set("public-preference", "unchanged").unwrap();
        clear(&storage, &a).unwrap();
        assert!(storage.get(&key(&a).unwrap()).unwrap().is_none());
        assert!(storage.get(&key(&b).unwrap()).unwrap().is_some());
        assert_eq!(
            storage.get("public-preference").unwrap().as_deref(),
            Some("unchanged")
        );
        clear(&storage, &a).unwrap(); // Duplicate receipt is local idempotent.
        assert!(storage.get(&key(&b).unwrap()).unwrap().is_some());
    }
}
