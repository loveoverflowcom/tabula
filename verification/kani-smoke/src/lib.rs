#![forbid(unsafe_code)]

/// Tiny pure function used only to prove the Kani toolchain is wired correctly.
#[must_use]
pub const fn saturating_next(value: u8) -> u8 {
    value.saturating_add(1)
}

#[cfg(kani)]
mod proofs {
    use super::saturating_next;

    /// The smoke proof is intentionally generic rather than game-specific.
    /// Real Tabula invariants belong in focused follow-up PRs.
    #[kani::proof]
    fn saturating_next_never_decreases() {
        let value: u8 = kani::any();
        let next = saturating_next(value);

        assert!(next >= value);
        assert!(next <= u8::MAX);
    }
}

#[cfg(test)]
mod tests {
    use super::saturating_next;

    #[test]
    fn smoke_function_has_obvious_boundary_behavior() {
        assert_eq!(saturating_next(0), 1);
        assert_eq!(saturating_next(u8::MAX), u8::MAX);
    }
}
