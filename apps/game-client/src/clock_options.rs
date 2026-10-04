//! Bounded local clock configuration, resolved before constructing a match.
//!
//! This is setup data, not canonical time or a clock estimate. Both browser
//! launch arguments and native setup use the same finite limits.

pub const MAX_INITIAL_MS: u64 = 10_800_000;
pub const MAX_ADJUSTMENT_MS: u64 = 60_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LocalClockControl {
    Untimed,
    #[default]
    Fischer,
    Bronstein,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalClockOptions {
    pub control: LocalClockControl,
    pub initial_ms: u64,
    pub adjustment_ms: u64,
}

impl Default for LocalClockOptions {
    fn default() -> Self {
        Self {
            control: LocalClockControl::Fischer,
            initial_ms: 300_000,
            adjustment_ms: 2_000,
        }
    }
}

impl LocalClockOptions {
    pub fn set_control(&mut self, value: &str) -> Result<(), &'static str> {
        self.control = match value {
            "untimed" => LocalClockControl::Untimed,
            "fischer" => LocalClockControl::Fischer,
            "bronstein" => LocalClockControl::Bronstein,
            _ => return Err("clock must be untimed, fischer or bronstein"),
        };
        Ok(())
    }

    pub fn set_initial(&mut self, value: &str) -> Result<(), &'static str> {
        let parsed = value.parse::<u64>().map_err(|_| "invalid initial time")?;
        if !(1_000..=MAX_INITIAL_MS).contains(&parsed) {
            return Err("initial time must be 1000..10800000 milliseconds");
        }
        self.initial_ms = parsed;
        Ok(())
    }

    pub fn set_adjustment(&mut self, value: &str) -> Result<(), &'static str> {
        let parsed = value
            .parse::<u64>()
            .map_err(|_| "invalid clock adjustment")?;
        if parsed > MAX_ADJUSTMENT_MS {
            return Err("clock adjustment must be 0..60000 milliseconds");
        }
        self.adjustment_ms = parsed;
        Ok(())
    }

    pub const fn timed(self) -> bool {
        !matches!(self.control, LocalClockControl::Untimed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_values_preserve_the_previous_setup() {
        let mut options = LocalClockOptions::default();
        for value in ["", "0", "999", "10800001", "-1", "18446744073709551616"] {
            assert!(options.set_initial(value).is_err());
            assert_eq!(options, LocalClockOptions::default());
        }
        for value in ["60001", "-1", "nan"] {
            assert!(options.set_adjustment(value).is_err());
            assert_eq!(options, LocalClockOptions::default());
        }
        assert!(options.set_control("online").is_err());
        assert_eq!(options, LocalClockOptions::default());
    }

    #[test]
    fn endpoints_and_all_controls_are_reachable() {
        let mut options = LocalClockOptions::default();
        for value in ["1000", "10800000"] {
            options.set_initial(value).unwrap();
            assert_eq!(options.initial_ms.to_string(), value);
        }
        for value in ["0", "60000"] {
            options.set_adjustment(value).unwrap();
            assert_eq!(options.adjustment_ms.to_string(), value);
        }
        for value in ["untimed", "fischer", "bronstein"] {
            options.set_control(value).unwrap();
            assert_eq!(options.timed(), value != "untimed");
        }
    }
}
