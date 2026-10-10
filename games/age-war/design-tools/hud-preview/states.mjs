// D06 screen/state -> intent/event -> asset map. Design contract data, not runtime code.
// Intent kinds: `command` is a D01 RULES R1 semantic command (validated by the
// rules owner); `local` stays in GamePresentation::Local and is never sent;
// `system` is a host/shell lifecycle action (GameHost seam), never a player command.
// Events are PROPOSED projected ViewEvents for C01: D01 defines no event enum yet.

export const VERSIONS = {
  rules: '0.1.0-d01 (UNBALANCED, owner review pending)',
  d05: 'reconstructed-v05',
  packs: '0.5.0',
  preview: 'd06-hud-0.1',
};

export const COMMANDS = ['Recruit', 'CancelRecruit', 'Research', 'AdvanceAge', 'BuildTurret', 'RefitTurret',
                         'SellTurret', 'Cast', 'Resign'];
export const SYSTEM = ['Launch', 'CancelLaunch', 'Retry', 'Pause', 'Resume', 'Restart', 'Quit', 'Rematch', 'Back'];
export const LOCAL = ['ToggleAgeSet', 'ShowCodex', 'ShowDetails', 'SelectSpell', 'MoveAim', 'CancelAim', 'PanCamera', 'OpenSocket',
                      'OpenTech', 'SelectQueueEntry', 'NextTip', 'SkipTutorial', 'SetSessionEffects'];

const pack = 'age-war-<age>@0.5.0';
export const ASSETS = {
  backdrop: `${pack}:scene/backdrop`,
  unit: `${pack}:unit/<unit>/<facing>/<clip>/<frame>`,
  mask: `${pack}:mask/<unit>/<facing>/<clip>/<frame>`,
  vfx: `${pack}:vfx/<recipe>/<A|B>/<phase>/<n>`,
  sfx: `${pack}|age-war-common@0.5.0:sfx/<id>`,
  clips: `${pack}:meta/clips`,
  // Not yet in any runtime pack: the preview derives stand-ins and labels them.
  unitIcon: 'MISSING hud/unit-icon/<unit> (D04 portrait/icon not packed; preview crops the D05 idle frame)',
  spellIcon: 'MISSING hud/spell-icon/<spell> (preview uses the D05 VFX contact frame)',
  turretIcon: 'MISSING hud/turret-icon/<turret> (D05 auxiliary study is PARTIAL)',
  base: 'MISSING base/<age>/<side>/<state> (D03 base art not in the D05 inputs)',
  uiIcons: 'shell icon set (tokens; no game art)',
};

const recruit = {ui: 'unit card tap/click, keys 1-6', kind: 'command', name: 'Recruit', payload: '{unit_id}',
                 event: 'RecruitAccepted{purchase, queue_index} | Rejected{reason}'};
const toggle = {ui: 'age-set chip, key G', kind: 'local', name: 'ToggleAgeSet', payload: 'current|previous'};
const details = {ui: 'long-press / hover / key I', kind: 'local', name: 'ShowDetails', payload: '{unit|spell|turret}'};
const spell = slot => ({ui: `spell ${slot} button, key ${slot === 1 ? 'Q' : 'E'}`, kind: 'local', name: 'SelectSpell',
                        payload: `{slot:${slot}}`});
const pause = {ui: 'pause button, Esc, system Back', kind: 'system', name: 'Pause', payload: '-',
               event: 'host freezes simulation, bot and presentation clock'};

export const SCREENS = [
  {id: 'shell.setup', owner: 'CMP / web shell', title: 'Chuẩn bị trận',
   states: ['ready', 'native-unavailable', 'pack-missing'],
   intents: [{ui: 'Bắt đầu', kind: 'system', name: 'Launch',
              payload: '{difficulty, start_age, tutorial, effects, audio}', event: 'GameHost Ready | Failed{reason}'},
             {ui: 'Back', kind: 'system', name: 'Back', payload: '-'}],
   events: [], assets: ['shell catalog cover (existing)', 'pack availability facts (no art download here)'],
   a11y: 'Native list/radio semantics; unavailable state names the unmet prerequisite.'},
  {id: 'host.loading', owner: 'Rust GameHost (Macroquad surface)', title: 'Đang chuẩn bị tài nguyên',
   states: ['progress', 'integrity-failed'],
   intents: [{ui: 'Huỷ', kind: 'system', name: 'CancelLaunch', payload: '-'},
             {ui: 'Thử lại', kind: 'system', name: 'Retry', payload: '{same pack@version}'}],
   events: ['verified bytes / declared bytes (real progress)'],
   assets: [ASSETS.clips, ASSETS.backdrop, 'unit/mask groups for the player tray (right) and visible enemy units (left)'],
   a11y: 'Announces stage name and percentage at most every 10%; failure is persistent text, not a toast.'},
  {id: 'battle.default', owner: 'Rust presenter -> RenderList', title: 'Trận đấu',
   states: ['idle', 'queue-training'],
   intents: [recruit, toggle, details, spell(1), spell(2), pause,
             {ui: 'codex in pause menu, key C', kind: 'local', name: 'ShowCodex', payload: '-'},
             {ui: 'minimap tap / drag lane / arrow keys', kind: 'local', name: 'PanCamera', payload: '{x_q}'},
             {ui: 'socket button, key T', kind: 'local', name: 'OpenSocket', payload: '{socket}'},
             {ui: 'age chip, key R', kind: 'local', name: 'OpenTech', payload: '-'}],
   events: ['Deployed{entity}', 'Impact{cue}', 'UnitDied{entity}', 'BaseDamaged{side, hp}', 'Income tick (projected gold/XP)'],
   assets: [ASSETS.backdrop, ASSETS.unit, ASSETS.mask, ASSETS.vfx, ASSETS.sfx, ASSETS.unitIcon, ASSETS.base],
   a11y: 'Status line: HP both bases, gold, population, age; tray items expose name, cost, pop and blocked reason.'},
  {id: 'battle.recruit-blocked', owner: 'Rust presenter', title: 'Không thể tuyển',
   states: ['insufficient-gold', 'population-full', 'queue-full'],
   intents: [recruit, details],
   events: ['Rejected{InsufficientGold|PopulationFull|QueueFull} (only if sent anyway)'],
   assets: [ASSETS.unitIcon, 'sfx/UI_denied_v01'],
   a11y: 'Disabled card keeps focus and states the reason ("Thiếu 25 vàng"); no modal.'},
  {id: 'battle.queue-cancel', owner: 'Rust presenter', title: 'Huỷ quân trong hàng chờ',
   states: ['waiting-entry (100% refund)', 'head-in-training (75% refund)'],
   intents: [{ui: 'queue slot tap, key Backspace', kind: 'local', name: 'SelectQueueEntry', payload: '{purchase}'},
             {ui: 'Xác nhận huỷ', kind: 'command', name: 'CancelRecruit', payload: '{purchase}',
              event: 'QueueCancelled{purchase, refund, pop}'}],
   events: ['QueueCancelled'], assets: [ASSETS.unitIcon, 'sfx/UI_cancel_v01'],
   a11y: 'Confirmation names the exact refund before sending.'},
  {id: 'battle.spell-cooldown', owner: 'Rust presenter', title: 'Phép đang hồi',
   states: ['cooling', 'ready', 'insufficient-gold'],
   intents: [spell(1), spell(2)],
   events: ['CastAccepted{slot, ready_at_tick}'],
   assets: [ASSETS.spellIcon],
   a11y: 'Remaining seconds as text; ring is decorative. Countdown is derived from ready_at, never authoritative.'},
  {id: 'battle.spell-targeting', owner: 'Rust presenter', title: 'Chọn vị trí phép',
   states: ['aiming', 'confirm (touch)', 'invalid-point'],
   intents: [{ui: 'drag / arrows', kind: 'local', name: 'MoveAim', payload: '{x_q}'},
             {ui: 'tap Confirm / click / Enter', kind: 'command', name: 'Cast', payload: '{slot, target_q}',
              event: 'CastAccepted | Rejected{EmptyTarget|Cooldown|Gold}'},
             {ui: 'Huỷ / right-click / Esc / Back', kind: 'local', name: 'CancelAim', payload: '-'}],
   events: ['TelegraphStarted{cast, x_q, due_tick}', 'Impact{cast}'],
   assets: [`${pack}:vfx/<spell>/A/start/<n>`, `${pack}:sfx/<spell>_start_v01`],
   a11y: 'Aim position announced as distance from own base; Esc/Back always cancels first.'},
  {id: 'battle.age-up', owner: 'Rust presenter', title: 'Nâng thời đại',
   states: ['locked (xp/gold)', 'ready', 'researching 5 s', 'completed', 'enemy-advanced'],
   intents: [{ui: 'advance button, key U', kind: 'command', name: 'AdvanceAge', payload: '-',
              event: 'AgeAdvanceStarted{to, done_tick} | Rejected{reason}'},
             {ui: 'tech sheet item', kind: 'command', name: 'Research', payload: '{tech_id}',
              event: 'ResearchStarted | Rejected'}, toggle],
   events: ['AgeAdvanced{side, age}', 'ResearchCompleted{tech}'],
   assets: ['next age pack groups preloaded during the 5 s research', 'sfx/age_change_v01'],
   a11y: 'Banner text announced once; skippable; never blocks input.'},
  {id: 'battle.turret', owner: 'Rust presenter', title: 'Tháp phòng thủ',
   states: ['empty socket', 'building', 'built', 'refit/sell'],
   intents: [{ui: 'branch option', kind: 'command', name: 'BuildTurret', payload: '{socket, turret_id}',
              event: 'TurretBuildStarted | Rejected'},
             {ui: 'refit option', kind: 'command', name: 'RefitTurret', payload: '{socket, turret_id}'},
             {ui: 'Bán', kind: 'command', name: 'SellTurret', payload: '{socket}'}],
   events: ['TurretBuilt{socket}', 'TurretDestroyed{socket}'],
   assets: [ASSETS.turretIcon, ASSETS.base], a11y: 'Sheet lists both branches with cost and role in words.'},
  {id: 'battle.base-critical', owner: 'Rust presenter', title: 'Căn cứ nguy cấp',
   states: ['own < 25%', 'enemy < 25%'], intents: [pause], events: ['BaseDamaged{hp}'],
   assets: ['sfx/base_critical_v01'],
   a11y: 'Gauge adds an icon and text; reduced motion replaces the pulse with a static outline.'},
  {id: 'battle.time-limit', owner: 'Rust presenter', title: 'Giới hạn thời gian',
   states: ['fatigue from 12 active minutes', 'hard limit at 20 active minutes'],
   intents: [pause], events: ['FatigueStarted', 'MatchEnded{outcome, reason:Timeout}'],
   assets: [], a11y: 'Persistent clock plus text announcement; never decide a winner from the HUD clock.'},
  {id: 'battle.tutorial', owner: 'Rust presenter (simulation paused per step)', title: 'Hướng dẫn',
   states: ['1 tuyển quân', '2 vàng & dân số', '3 tiền tuyến', '4 phép & chọn vị trí', '5 nâng thời đại'],
   intents: [{ui: 'Tiếp', kind: 'local', name: 'NextTip', payload: '-'},
             {ui: 'Bỏ qua', kind: 'local', name: 'SkipTutorial', payload: '-'}, recruit],
   events: [], assets: [ASSETS.unitIcon], a11y: 'Each tip is readable text with a focus target; no timed auto-advance.'},
  {id: 'overlay.pause', owner: 'Rust presenter', title: 'Tạm dừng',
   states: ['menu', 'confirm-restart', 'confirm-quit'],
   intents: [{ui: 'Tiếp tục / Esc', kind: 'system', name: 'Resume', payload: '-'},
             {ui: 'Chơi lại', kind: 'system', name: 'Restart', payload: '{same config}'},
             {ui: 'Thoát', kind: 'system', name: 'Quit', payload: '-', event: 'GameHost Exited{abandoned}'},
             {ui: 'Đầu hàng + xác nhận', kind: 'command', name: 'Resign', payload: '-', event: 'MatchEnded{reason:Resign}'},
             {ui: 'Hiệu ứng / âm thanh', kind: 'local', name: 'SetSessionEffects', payload: '{effects, volume}'}],
   events: [], assets: [], a11y: 'Focus trapped in the sheet; Back/Esc resumes.'},
  {id: 'overlay.result', owner: 'CMP / web shell after GameHost exit (doc 04 §1.1)', title: 'Kết quả',
   states: ['victory', 'defeat', 'draw (timeout)'],
   intents: [{ui: 'Đấu lại', kind: 'system', name: 'Rematch', payload: '{same config, new seed}'},
             {ui: 'Về thư viện', kind: 'system', name: 'Quit', payload: '-', event: 'GameHost Exited{outcome}'}],
   events: ['MatchEnded{outcome, reason}'], assets: ['sfx/win_v01', 'sfx/lose_v01'],
   a11y: 'Outcome and reason as text first; stats are a list.'},
  {id: 'portrait.rotate', owner: 'Rust presenter (+ shell orientation hint)', title: 'Xoay ngang',
   states: ['paused'], intents: [{ui: 'Thoát', kind: 'system', name: 'Quit', payload: '-'}],
   events: [], assets: [], a11y: 'Explains why; resumes only by an explicit Resume after rotating back.'},
  {id: 'error.asset', owner: 'Rust GameHost', title: 'Thiếu tài nguyên',
   states: ['manifest/integrity failure', 'budget exceeded', 'native host unavailable (mobile)'],
   intents: [{ui: 'Thử lại', kind: 'system', name: 'Retry', payload: '{same pack@version}'},
             {ui: 'Thoát', kind: 'system', name: 'Quit', payload: '-', event: 'GameHost Failed{reason}'}],
   events: [], assets: [], a11y: 'Persistent reason and safe error code; no substitute pack.'},
];

/** Every intent must be exactly one of the three kinds and a known name. */
export function intentProblems() {
  const problems = [];
  for (const screen of SCREENS) {
    for (const intent of screen.intents) {
      const known = {command: COMMANDS, local: LOCAL, system: SYSTEM}[intent.kind];
      if (!known || !known.includes(intent.name)) problems.push(`${screen.id}: ${intent.kind} ${intent.name}`);
    }
  }
  return problems;
}
