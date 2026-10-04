//! Verification-only policies, compiled solely with `testkit` (W-D18).
//! This adapter is never the production `WerewolfModule` or a runtime substitute.
use crate::{Config, Phase, View, WerewolfModule, WerewolfRules};
use tabula_core::{BotLevel, DetRng, Millis, SeatId, SeatRoster};
use tabula_game_api::{ConfigError, GameBot, GameCapabilities, GameMetadata, GameModule};
/// Test-only module for the generic deterministic self-play harness.
#[derive(Debug)]
pub struct SimulationModule;
impl GameModule for SimulationModule {
    type Rules = WerewolfRules;
    fn metadata() -> &'static GameMetadata {
        WerewolfModule::metadata()
    }
    fn capabilities() -> &'static GameCapabilities {
        WerewolfModule::capabilities()
    }
    fn declared_bot_levels() -> &'static [BotLevel] {
        &[BotLevel::Trivial]
    }
    fn bot(level: BotLevel) -> Option<Box<dyn GameBot<WerewolfRules>>> {
        (level == BotLevel::Trivial)
            .then(|| Box::new(ProjectedPolicy) as Box<dyn GameBot<WerewolfRules>>)
    }
    fn validate_config(cfg: &Config, roster: &SeatRoster) -> Result<(), ConfigError> {
        WerewolfModule::validate_config(cfg, roster)
    }
}
struct ProjectedPolicy;
impl GameBot<WerewolfRules> for ProjectedPolicy {
    fn level(&self) -> BotLevel {
        BotLevel::Trivial
    }
    fn choose(&self, view: &View, seat: SeatId, rng: &mut DetRng) -> Option<crate::Command> {
        // Cast once per Vote so all seats get a chance before the fixed timer.
        if view.phase == Phase::Vote && view.votes.contains_key(&seat) {
            return None;
        }
        if view.legal_commands.is_empty() {
            return None;
        }
        let count = u32::try_from(view.legal_commands.len()).ok()?;
        view.legal_commands
            .get(usize::try_from(rng.below(count)).ok()?)
            .copied()
    }
    fn think_time(&self, _: &View) -> Millis {
        Millis(1)
    }
}
