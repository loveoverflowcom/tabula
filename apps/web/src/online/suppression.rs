//! A non-authorizing unresolved-admission bit across same-tab document reloads.
//! No session/match enumeration or recovery authority is encoded (ADR-0041).
const KEY: &str = "tabula-direct-admission-unconfirmed-v1";
const VALUE: &str = "1";

pub(super) trait Store {
    fn get(&self, key: &str) -> Result<Option<String>, ()>;
    fn set(&self, key: &str, value: &str) -> Result<(), ()>;
    fn remove(&self, key: &str) -> Result<(), ()>;
}
pub(super) fn restore(store: &impl Store) -> Result<bool, ()> {
    // An unexpected value cannot turn unresolved work into a retry license.
    Ok(store.get(KEY)?.is_some())
}
pub(super) fn save(store: &impl Store, pending: bool) -> Result<(), ()> {
    if pending {
        store.set(KEY, VALUE)?;
        if store.get(KEY)?.as_deref() != Some(VALUE) {
            return Err(());
        }
    } else {
        store.remove(KEY)?;
        if store.get(KEY)?.is_some() {
            return Err(());
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    #[derive(Default)]
    struct MemoryStore {
        value: RefCell<Option<String>>,
        fail_read: Cell<bool>,
        fail_write: Cell<bool>,
        fail_remove: Cell<bool>,
    }
    impl Store for MemoryStore {
        fn get(&self, key: &str) -> Result<Option<String>, ()> {
            assert_eq!(key, KEY);
            if self.fail_read.get() {
                Err(())
            } else {
                Ok(self.value.borrow().clone())
            }
        }
        fn set(&self, key: &str, value: &str) -> Result<(), ()> {
            assert_eq!(key, KEY);
            assert_eq!(
                value, VALUE,
                "the bit contains no code, identity or routing data"
            );
            if self.fail_write.get() {
                Err(())
            } else {
                self.value.replace(Some(value.into()));
                Ok(())
            }
        }
        fn remove(&self, key: &str) -> Result<(), ()> {
            assert_eq!(key, KEY);
            if self.fail_remove.get() {
                Err(())
            } else {
                self.value.replace(None);
                Ok(())
            }
        }
    }
    #[test]
    fn unresolved_post_survives_reload_and_only_confirmed_receipt_clears_bit() {
        let store = MemoryStore::default();
        assert_eq!(restore(&store), Ok(false));
        assert_eq!(save(&store, true), Ok(()));
        assert_eq!(restore(&store), Ok(true));
        assert_eq!(save(&store, false), Ok(()));
        assert_eq!(restore(&store), Ok(false));
        store.value.replace(Some("unexpected".into()));
        assert_eq!(restore(&store), Ok(true));
    }
    #[test]
    fn storage_failures_cannot_approve_dispatch_or_claim_cleanup() {
        let store = MemoryStore::default();
        store.fail_write.set(true);
        assert_eq!(save(&store, true), Err(()));
        store.fail_write.set(false);
        store.fail_read.set(true);
        assert_eq!(save(&store, true), Err(()));
        assert_eq!(restore(&store), Err(()));
        store.fail_read.set(false);
        store.fail_remove.set(true);
        assert_eq!(save(&store, false), Err(()));
        assert_eq!(restore(&store), Ok(true));
    }
}
