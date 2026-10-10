import test from 'node:test';
import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {layout, violations, VIEWPORTS, intersects} from '../hud-preview/layout.mjs';
import {SCREENS, COMMANDS, intentProblems} from '../hud-preview/states.mjs';
import {poseAt, sample, nextAge} from '../hud-preview/storyboard.mjs';

for (const viewport of VIEWPORTS) for (const font of [1, 1.3, 2]) {
  test(`HUD safe area, lane, focus/touch targets: ${viewport.name} font ${font}`, () => {
    const result = layout({...viewport, font});
    assert.deepEqual(violations(result), []);
    assert.equal(result.hud.cards.length, 6); assert.equal(result.queueEntries.length, 5);
    assert.equal(result.hud.spells.length, 2); assert.equal(result.hud.sockets.length, 2);
    for (const entry of result.queueEntries) assert.ok(entry.h >= result.target && entry.w >= result.target);
    assert.ok(result.hud.economy.h >= 45 * font + 8, 'three enlarged economy lines fit vertically');
    assert.ok(!intersects(result.hud.dock, result.lane));
  });
}
test('portrait explains rotate instead of compressing the HUD', () => {
  const result = layout({vw: 390, vh: 844, font: 2});
  assert.equal(result.mode, 'rotate'); assert.ok(result.actions[0].h >= 44);
});
test('geometry check detects a plausible control/lane regression', () => {
  const result = layout(VIEWPORTS[0]); result.hud.cards[0].y = result.lane.y;
  assert.ok(violations(result).some(s => s.includes('covers the lane')));
  result.hud.spells[0].w = 20;
  assert.ok(violations(result).some(s => s.includes('< 44')));
});
test('all commands and lifecycle surfaces have an explicit owner', () => {
  assert.deepEqual(intentProblems(), []);
  const commandSet = new Set(SCREENS.flatMap(s => s.intents).filter(i => i.kind === 'command').map(i => i.name));
  for (const command of COMMANDS) assert.ok(commandSet.has(command), command);
  assert.equal(new Set(SCREENS.map(s => s.id)).size, SCREENS.length);
  assert.match(SCREENS.find(s => s.id === 'overlay.result').owner, /shell/);
});
test('complete clip sampler honours marker frames and one-shot endpoints', () => {
  const clip = {duration: 1000, loop: false, frames: [{t: 0}, {t: 416}, {t: 450}, {t: 1000}]};
  assert.equal(sample(clip, 449).t, 416); assert.equal(sample(clip, 450).t, 450);
  assert.equal(sample(clip, 9000).t, 1000);
  assert.equal(sample({...clip, loop: true}, 1450).t, 450);
  assert.equal(sample({duration: 0, frames: []}, 0), null);
});
test('storyboard is elapsed-time based and repeats without combat state', () => {
  const names = ['idle', 'walk', 'attack', 'skill_heal', 'hit', 'death'];
  for (const [t, expected] of [[0,'walk'], [1400,'attack'], [4800,'skill_heal'], [6300,'hit'], [7400,'death'], [9000,'idle']]) {
    assert.equal(poseAt(t, names).clip, expected); assert.deepEqual(poseAt(t + 11000, names), poseAt(t, names));
  }
  assert.equal(poseAt(4000, ['idle','run','attack']).clip, 'attack');
});
test('age advance reads the NEXT catalog entry, preserving its actual D01 costs', () => {
  const catalog = JSON.parse(readFileSync(new URL('../hud-preview/catalog.generated.json', import.meta.url)));
  assert.equal(catalog.ages.length, 6); assert.equal(catalog.ages.flatMap(a => a.units).length, 36);
  assert.deepEqual(nextAge(catalog, 'Primitive').advance_to, {gold: 250, total_xp: 60, duration_s: 5});
  assert.equal(nextAge(catalog, 'Industrial').advance_to.gold, 1150);
  assert.equal(nextAge(catalog, 'Future'), null);
});
