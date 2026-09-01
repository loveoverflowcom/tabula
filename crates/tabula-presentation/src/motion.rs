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
use tabula_design::{MotionCategory, MotionDuration, MotionProfile, Spring, SpringKind, Theme};

/// The elapsed-time boundary used when deciding whether a newly observed animation is stale.
///
/// This is a late-arrival policy, not a maximum animation lifetime. A timeline may legitimately
/// use a longer semantic duration such as the 800 ms `xlong` token. (doc 04 §9.1)
pub const STALE_ANIMATION_THRESHOLD_MS: u64 = 600;

/// Selects whether a semantic motion profile is played at full speed or resolved through the
/// theme's reduced-motion policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MotionMode {
    /// Play the profile's normal semantic duration.
    Full,
    /// Apply the theme's reduced-motion policy.
    Reduced,
}

/// The result of deciding whether an animation can be played when it arrives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MotionStart {
    /// The animation arrived in time and may be sampled from its original start timestamp.
    Animate(MotionTimeline),
    /// The animation arrived too late and should render its terminal state immediately.
    Snap,
}

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
    curve_duration_ms: u64,
    spring: Spring,
}

impl MotionTimeline {
    /// Constructs a motion timeline from validated duration and spring parameters.
    #[must_use]
    pub const fn new(started_at_ms: u64, duration_ms: u64, spring: Spring) -> Self {
        Self {
            started_at_ms,
            duration_ms,
            curve_duration_ms: duration_ms,
            spring,
        }
    }

    /// Constructs a motion timeline from a semantic profile, active theme, and explicit mode.
    ///
    /// `MotionMode::Full` is the normal execution mode; the theme's reduced policy is never
    /// implicitly applied.
    #[must_use]
    pub fn from_profile(
        started_at_ms: u64,
        profile: MotionProfile,
        theme: &Theme,
        mode: MotionMode,
    ) -> Self {
        let spring = resolve_spring(theme, profile.spring);
        let duration_ms = resolve_duration(profile, theme, mode);
        Self {
            started_at_ms,
            duration_ms,
            curve_duration_ms: u64::from(profile.duration.milliseconds()),
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
        if self.duration_ms == 0 || elapsed >= self.duration_ms {
            return MotionSample::finished();
        }

        // Duration scaling changes playback speed while preserving the profile's curve. For
        // example, a 280 ms profile resolved to 80 ms still evaluates its spring over the full
        // 280 ms shape before the effective timeline reaches its terminal sample.
        let curve_elapsed = elapsed.saturating_mul(self.curve_duration_ms) / self.duration_ms;
        let factor = evaluate_spring(self.spring, curve_elapsed);
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

/// Resolves effective motion duration under an explicit motion mode.
///
/// Full motion uses the profile duration exactly. Reduced motion disables ambient profiles when
/// requested and otherwise scales durations. Strict reduced motion keeps informative state
/// changes alive at the semantic `instant` floor when `keep_informative` is enabled.
#[must_use]
pub const fn resolve_duration(profile: MotionProfile, theme: &Theme, mode: MotionMode) -> u64 {
    let original = profile.duration.milliseconds() as u64;
    if matches!(mode, MotionMode::Full) {
        return original;
    }

    let reduced = theme.motion.reduced;
    if matches!(profile.category, MotionCategory::Ambient) && reduced.disable_ambient {
        return 0;
    }
    let scale = reduced.duration_scale.get() as u64;
    if scale == 0 {
        return if matches!(profile.category, MotionCategory::Informative)
            && reduced.keep_informative
        {
            let instant = theme.motion.instant.milliseconds() as u64;
            if original < instant {
                original
            } else {
                instant
            }
        } else {
            0
        };
    }
    original * scale / 100
}

/// Returns whether an animation is stale when it is first observed.
///
/// The exact threshold remains playable; only an age strictly greater than the threshold snaps.
/// Saturating subtraction treats an observation before the start as not stale.
#[must_use]
pub const fn is_stale_on_arrival(started_at_ms: u64, observed_at_ms: u64) -> bool {
    observed_at_ms.saturating_sub(started_at_ms) > STALE_ANIMATION_THRESHOLD_MS
}

/// Resolves a motion profile into an animation or an immediate terminal state.
///
/// Callers with an event timestamp should use this boundary once, when the event is observed.
/// [`MotionTimeline::sample`] intentionally does not apply this late-arrival rule.
#[must_use]
pub fn resolve_motion_start(
    started_at_ms: u64,
    observed_at_ms: u64,
    profile: MotionProfile,
    theme: &Theme,
    mode: MotionMode,
) -> MotionStart {
    if is_stale_on_arrival(started_at_ms, observed_at_ms) {
        MotionStart::Snap
    } else {
        MotionStart::Animate(MotionTimeline::from_profile(
            started_at_ms,
            profile,
            theme,
            mode,
        ))
    }
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
    fn normal_800ms_motion_is_not_stale_at_601ms() {
        let timeline = MotionTimeline::new(1000, 800, test_spring());
        for now in [1600, 1601, 1799] {
            assert!(
                !timeline.sample(now).done,
                "sample at {now} should remain active"
            );
        }
        assert_eq!(timeline.sample(1800), MotionSample::finished());
    }

    #[test]
    fn full_motion_does_not_apply_reduced_policy_implicitly() {
        let theme = Theme::by_kind(ThemeKind::Light);
        assert_eq!(
            resolve_duration(theme.motion.piece_move, &theme, MotionMode::Full),
            280
        );
        let timeline =
            MotionTimeline::from_profile(1000, theme.motion.piece_move, &theme, MotionMode::Full);
        assert_eq!(timeline.duration_ms(), 280);
    }

    #[test]
    fn reduced_motion_duration_resolution_honors_categories_and_floor() {
        let mut theme = Theme::by_kind(ThemeKind::Light);
        let mut reduced = theme.motion.reduced;

        // Strict reduced motion keeps informative motion at the semantic instant floor.
        reduced.duration_scale = Percent::new(0).unwrap();
        assert_eq!(
            resolve_duration(theme.motion.piece_move, &theme, MotionMode::Reduced),
            u64::from(theme.motion.instant.milliseconds())
        );

        // Ambient motion is disabled by policy.
        reduced.duration_scale = Percent::new(100).unwrap();
        reduced.disable_ambient = true;
        theme.motion.reduced = reduced;
        assert_eq!(
            resolve_duration(theme.motion.phase_change, &theme, MotionMode::Reduced),
            0
        );

        // Informative motion with a non-zero scale uses the scaled semantic duration.
        reduced.duration_scale = Percent::new(50).unwrap();
        theme.motion.reduced = reduced;
        let piece_move_duration = theme.motion.piece_move.duration.milliseconds();
        assert_eq!(
            resolve_duration(theme.motion.piece_move, &theme, MotionMode::Reduced),
            u64::from(piece_move_duration) * 50 / 100
        );
    }

    #[test]
    fn late_arriving_motion_snaps_but_exact_boundary_animates() {
        let theme = Theme::by_kind(ThemeKind::Light);
        let profile = theme.motion.win;
        assert!(!is_stale_on_arrival(1000, 1600));
        assert!(is_stale_on_arrival(1000, 1601));
        assert!(matches!(
            resolve_motion_start(1000, 1600, profile, &theme, MotionMode::Full),
            MotionStart::Animate(_)
        ));
        assert_eq!(
            resolve_motion_start(1000, 1601, profile, &theme, MotionMode::Full),
            MotionStart::Snap
        );
    }

    #[test]
    fn reduced_duration_preserves_the_profile_curve_until_terminal_sample() {
        let mut theme = Theme::by_kind(ThemeKind::Light);
        let mut reduced = theme.motion.reduced;
        reduced.duration_scale = Percent::new(50).unwrap();
        theme.motion.reduced = reduced;
        let timeline = MotionTimeline::from_profile(
            1000,
            theme.motion.piece_move,
            &theme,
            MotionMode::Reduced,
        );
        assert_eq!(timeline.duration_ms(), 140);
        assert!(!timeline.sample(1139).done);
        assert_eq!(timeline.sample(1140), MotionSample::finished());
        assert!(timeline.sample(1139).factor > 0.9);
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
