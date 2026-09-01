//! Deterministic presentation motion and animation runtime. (doc 04 §9)
//!
//! # Absolute-Time Sampling (Frame-Partition Invariance)
//!
//! Animations are modeled as pure mathematical functions of absolute presentation time
//! [`crate::FrameCtx::now_ms`]. They are never numerically integrated frame-by-frame:
//!
//! ```text
//! sample(now_ms) -> MotionSample { factor, done }
//! ```
//!
//! Sampling at a terminal timestamp produces the exact same result whether zero, one,
//! or a thousand intermediate frame samples occurred.
//!
//! Animation is presentation-only: it never mutates canonical rules state, never
//! delays `Intent` submission, and never gates command legality. (I-10)

#![allow(clippy::doc_markdown)]

use glam::Vec2;
use tabula_design::{
    MotionCategory, MotionDuration, MotionProfile, ReducedMotion, Spring, SpringKind, Theme,
};

/// The maximum duration in milliseconds before an uncompleted animation snaps to its end state. (doc 04 §9.1)
pub const STALE_ANIMATION_THRESHOLD_MS: u64 = 600;

/// The evaluated progress and completion status of a motion at a specific timestamp.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionSample {
    /// The normalized interpolation factor (nominally `0.0..=1.0`, with possible spring overshoot).
    pub factor: f32,
    /// Whether the motion has reached or exceeded its terminal timestamp.
    pub done: bool,
}

impl MotionSample {
    /// Initial sample at or before the animation start.
    #[must_use]
    pub const fn initial() -> Self {
        Self {
            factor: 0.0,
            done: false,
        }
    }

    /// Terminal sample snapping exactly to completion.
    #[must_use]
    pub const fn finished() -> Self {
        Self {
            factor: 1.0,
            done: true,
        }
    }
}

/// An immutable presentation motion timeline sampled from absolute time.
///
/// @ai.role presentation-motion
/// @ai.domain presentation.motion
/// @ai.pure true
/// @ai.invariant no-canonical-state
/// @ai.law frame-partition-invariance
/// @ai.law terminal-snap
/// @ai.evidence crate::motion::tests::motion_sampling_depends_on_absolute_time_not_frame_history
/// @ai.evidence crate::motion::tests::motion_terminal_sample_is_exact
/// @ai.evidence crate::motion::tests::motion_final_state_is_invariant_to_frame_partition
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionTimeline {
    started_at_ms: u64,
    duration_ms: u64,
    spring: Spring,
}

impl MotionTimeline {
    /// Constructs a motion timeline from validated duration and spring parameters.
    #[must_use]
    pub const fn new(started_at_ms: u64, duration_ms: u64, spring: Spring) -> Self {
        Self {
            started_at_ms,
            duration_ms,
            spring,
        }
    }

    /// Constructs a motion timeline from a semantic profile and active theme.
    ///
    /// Resolves the spring family and reduced-motion policy from the theme.
    #[must_use]
    pub fn from_profile(started_at_ms: u64, profile: MotionProfile, theme: &Theme) -> Self {
        let spring = resolve_spring(theme, profile.spring);
        let duration_ms = resolve_duration(profile, theme.motion.reduced);
        Self {
            started_at_ms,
            duration_ms,
            spring,
        }
    }

    /// Returns the start timestamp in presentation milliseconds.
    #[must_use]
    pub const fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    /// Returns the effective duration in milliseconds.
    #[must_use]
    pub const fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Returns the active spring parameters.
    #[must_use]
    pub const fn spring(&self) -> Spring {
        self.spring
    }

    /// Samples the timeline at an absolute timestamp.
    ///
    /// The result is a pure function of `(self, now_ms)` and is independent of previous frames.
    ///
    /// @ai.role motion-sampling
    /// @ai.domain presentation.motion
    /// @ai.pure true
    /// @ai.law frame-partition-invariance
    /// @ai.law terminal-snap
    /// @ai.evidence crate::motion::tests::motion_sampling_depends_on_absolute_time_not_frame_history
    /// @ai.evidence crate::motion::tests::motion_terminal_sample_is_exact
    /// @ai.evidence crate::motion::tests::motion_final_state_is_invariant_to_frame_partition
    #[must_use]
    pub fn sample(&self, now_ms: u64) -> MotionSample {
        if now_ms < self.started_at_ms {
            return MotionSample::initial();
        }
        let elapsed = now_ms.saturating_sub(self.started_at_ms);
        if self.duration_ms == 0
            || elapsed >= self.duration_ms
            || elapsed > STALE_ANIMATION_THRESHOLD_MS
        {
            return MotionSample::finished();
        }

        let factor = evaluate_spring(self.spring, elapsed);
        MotionSample {
            factor,
            done: false,
        }
    }
}

/// Resolves a spring family reference into concrete parameters from the theme.
#[must_use]
pub const fn resolve_spring(theme: &Theme, kind: SpringKind) -> Spring {
    match kind {
        SpringKind::Snappy => theme.motion.spring_snappy,
        SpringKind::Standard => theme.motion.spring_standard,
        SpringKind::Weighty => theme.motion.spring_weighty,
        SpringKind::Bouncy => theme.motion.spring_bouncy,
    }
}

/// Resolves effective motion duration under the active reduced-motion policy.
///
/// Ambient motions are disabled if requested; informative motions are scaled
/// by the duration percentage scale.
#[must_use]
pub const fn resolve_duration(profile: MotionProfile, reduced: ReducedMotion) -> u64 {
    if matches!(profile.category, MotionCategory::Ambient) && reduced.disable_ambient {
        return 0;
    }
    let scale = reduced.duration_scale.get() as u64;
    if scale == 0 {
        return 0;
    }
    (profile.duration.milliseconds() as u64) * scale / 100
}

/// Computes the absolute start time for an item in a staggered sequence.
///
/// Uses saturating arithmetic to prevent integer overflow.
#[must_use]
pub const fn staggered_start(started_at_ms: u64, index: u32, stagger: MotionDuration) -> u64 {
    let offset = (index as u64).saturating_mul(stagger.milliseconds() as u64);
    started_at_ms.saturating_add(offset)
}

/// Linearly interpolates between two 2D positions.
#[must_use]
#[allow(clippy::float_arithmetic)]
pub fn lerp_vec2(from: Vec2, to: Vec2, factor: f32) -> Vec2 {
    from + (to - from) * factor
}

/// Linearly interpolates between two scalar values.
#[must_use]
#[allow(clippy::float_arithmetic)]
pub fn lerp_f32(from: f32, to: f32, factor: f32) -> f32 {
    from + (to - from) * factor
}

/// Evaluates a damped harmonic oscillator at elapsed milliseconds.
#[allow(
    clippy::cast_precision_loss,
    clippy::float_arithmetic,
    clippy::many_single_char_names
)]
fn evaluate_spring(spring: Spring, elapsed_ms: u64) -> f32 {
    let k = spring.stiffness.get();
    let c = spring.damping.get();
    let m = spring.mass.get();
    let t = elapsed_ms as f32 / 1000.0;

    let omega0 = (k / m).sqrt();
    let zeta = c / (2.0 * (k * m).sqrt());

    if (zeta - 1.0).abs() < 1e-5 {
        // Critically damped
        let y = (-omega0 * t).exp() * (1.0 + omega0 * t);
        1.0 - y
    } else if zeta < 1.0 {
        // Underdamped
        let omega_d = omega0 * (1.0 - zeta * zeta).sqrt();
        let decay = (-zeta * omega0 * t).exp();
        let y = decay
            * ((omega_d * t).cos() + (zeta / (1.0 - zeta * zeta).sqrt()) * (omega_d * t).sin());
        1.0 - y
    } else {
        // Overdamped
        let omega_d = omega0 * (zeta * zeta - 1.0).sqrt();
        let decay = (-zeta * omega0 * t).exp();
        let y = decay
            * ((omega_d * t).cosh() + (zeta / (zeta * zeta - 1.0).sqrt()) * (omega_d * t).sinh());
        1.0 - y
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use tabula_design::{Percent, Positive, ThemeKind};

    fn test_spring() -> Spring {
        Spring {
            stiffness: Positive::new(260.0).unwrap(),
            damping: Positive::new(24.0).unwrap(),
            mass: Positive::new(1.0).unwrap(),
        }
    }

    #[test]
    fn motion_sampling_depends_on_absolute_time_not_frame_history() {
        let timeline = MotionTimeline::new(1000, 300, test_spring());
        // Sampling at t=1150 directly
        let direct_sample = timeline.sample(1150);

        // Sampling at intermediate frames first
        let _ = timeline.sample(1050);
        let _ = timeline.sample(1100);
        let sequential_sample = timeline.sample(1150);

        assert_eq!(direct_sample, sequential_sample);
    }

    #[test]
    fn motion_terminal_sample_is_exact() {
        let timeline = MotionTimeline::new(1000, 280, test_spring());
        // Exact completion timestamp
        let sample_at_end = timeline.sample(1280);
        assert_eq!(sample_at_end.factor, 1.0);
        assert!(sample_at_end.done);

        // Past completion timestamp
        let sample_past_end = timeline.sample(1500);
        assert_eq!(sample_past_end.factor, 1.0);
        assert!(sample_past_end.done);
    }

    #[test]
    fn motion_final_state_is_invariant_to_frame_partition() {
        let timeline = MotionTimeline::new(1000, 280, test_spring());
        let final_time = 1280;

        // Path A: no intermediate frames
        let path_a = timeline.sample(final_time);

        // Path B: 60 Hz (~16 ms step)
        let mut t = 1000;
        while t < final_time {
            let _ = timeline.sample(t);
            t += 16;
        }
        let path_b = timeline.sample(final_time);

        // Path C: 120 Hz (~8 ms step)
        t = 1000;
        while t < final_time {
            let _ = timeline.sample(t);
            t += 8;
        }
        let path_c = timeline.sample(final_time);

        // Path D: irregular / stalls
        let _ = timeline.sample(1010);
        let _ = timeline.sample(1015);
        let _ = timeline.sample(1150);
        let _ = timeline.sample(1279);
        let path_d = timeline.sample(final_time);

        assert_eq!(path_a, MotionSample::finished());
        assert_eq!(path_a, path_b);
        assert_eq!(path_a, path_c);
        assert_eq!(path_a, path_d);
    }

    #[test]
    fn zero_duration_motion_finishes_immediately() {
        let timeline = MotionTimeline::new(1000, 0, test_spring());
        assert_eq!(timeline.sample(999), MotionSample::initial());
        assert_eq!(timeline.sample(1000), MotionSample::finished());
        assert_eq!(timeline.sample(1001), MotionSample::finished());
    }

    #[test]
    fn stale_motion_snaps_to_terminal_state() {
        let timeline = MotionTimeline::new(1000, 800, test_spring());
        // At 1600 ms (elapsed 600 ms <= threshold), still active
        let active = timeline.sample(1600);
        assert!(!active.done);

        // At 1601 ms (elapsed 601 ms > threshold), snaps to terminal
        let stale = timeline.sample(1601);
        assert_eq!(stale, MotionSample::finished());
    }

    #[test]
    fn reduced_motion_duration_resolution_handles_all_categories() {
        let theme = Theme::by_kind(ThemeKind::Light);
        let mut reduced = theme.motion.reduced;

        // duration_scale = 0 -> 0 duration
        reduced.duration_scale = Percent::new(0).unwrap();
        assert_eq!(resolve_duration(theme.motion.piece_move, reduced), 0);

        // ambient with disable_ambient -> 0 duration
        reduced.duration_scale = Percent::new(100).unwrap();
        reduced.disable_ambient = true;
        assert_eq!(resolve_duration(theme.motion.phase_change, reduced), 0);

        // informative with scale 50%
        reduced.duration_scale = Percent::new(50).unwrap();
        let piece_move_duration = theme.motion.piece_move.duration.milliseconds();
        assert_eq!(
            resolve_duration(theme.motion.piece_move, reduced),
            u64::from(piece_move_duration) * 50 / 100
        );
    }

    #[test]
    fn time_arithmetic_boundaries_do_not_panic() {
        let timeline = MotionTimeline::new(u64::MAX - 100, 50, test_spring());
        assert_eq!(timeline.sample(0), MotionSample::initial());
        assert_eq!(timeline.sample(u64::MAX), MotionSample::finished());

        let stagger = staggered_start(u64::MAX - 10, 10, MotionDuration::from_millis(40));
        assert_eq!(stagger, u64::MAX);
    }

    #[test]
    fn lerp_helpers_evaluate_endpoints_and_midpoint() {
        let a = Vec2::new(10.0, 20.0);
        let b = Vec2::new(30.0, 40.0);
        assert_eq!(lerp_vec2(a, b, 0.0), a);
        assert_eq!(lerp_vec2(a, b, 1.0), b);
        assert_eq!(lerp_vec2(a, b, 0.5), Vec2::new(20.0, 30.0));

        assert_eq!(lerp_f32(0.0, 100.0, 0.0), 0.0);
        assert_eq!(lerp_f32(0.0, 100.0, 1.0), 100.0);
        assert_eq!(lerp_f32(0.0, 100.0, 0.25), 25.0);
    }
}
