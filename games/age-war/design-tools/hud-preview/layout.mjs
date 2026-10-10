// Pure HUD layout for the D06 design preview. Design data, not a runtime layout engine.
// All values are CSS/logical pixels. The battlefield camera always shows the D03
// 1,920-unit focus window (10,000 q) across the full width; HUD never covers the lane band.

export const STAGE = {width: 1920, height: 698, ground: 490};
// Lane band in stage units: tallest body/weapon reach above the ground row, plus
// the shape-coded ground marker below it.
export const LANE = {above: 135, below: 12};
export const TOUCH_TARGET = 44;
export const POINTER_TARGET = 36;
export const QUEUE_SLOTS = 5;
export const TRAY_SLOTS = 6;

const round = value => Math.round(value * 10) / 10;

function box(x, y, w, h) {
  return {x: round(x), y: round(y), w: round(w), h: round(h)};
}

export function intersects(a, b) {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

/**
 * @param {{vw:number, vh:number, safe?:{top:number,right:number,bottom:number,left:number},
 *          font?:number, input?:'touch'|'pointer'}} options
 */
export function layout({vw, vh, safe = {top: 0, right: 0, bottom: 0, left: 0}, font = 1, input = 'touch'}) {
  if (!(vw > 0 && vh > 0 && font > 0)) throw new Error('invalid viewport');
  const target = input === 'touch' ? TOUCH_TARGET : POINTER_TARGET;
  if (vh > vw) {
    // Portrait: the lane would shrink below a readable body size, so the match
    // pauses behind an explained rotate prompt (the D06 portrait decision).
    const card = box(safe.left + 16, vh * 0.3, vw - safe.left - safe.right - 32, Math.max(180, 120 * font));
    return {orientation: 'portrait', mode: 'rotate', target, prompt: card,
            actions: [box(card.x + 16, card.y + card.h - target - 16, card.w - 32, target)]};
  }
  const inner = box(safe.left, safe.top, vw - safe.left - safe.right, vh - safe.top - safe.bottom);
  const scale = vw / STAGE.width;
  const gap = Math.min(8, Math.max(4, Math.round(6 * font)));
  const compact = vh < 500;

  // Top bar: base HP, clock/pause, age/XP. Text-bearing, so it grows with font.
  const topH = Math.max(target, Math.round((compact ? 18 : 22) * font + 16));
  const top = box(inner.x, inner.y, inner.w, topH);
  // Top bar: both base HP gauges at the edges; centre holds age chip, advance,
  // pause and clock. Technology research opens from the age chip as a sheet.
  const hpW = Math.round(inner.w * (compact ? 0.26 : 0.28));
  const hpLeft = box(inner.x + gap, top.y, hpW, topH);
  const hpRight = box(inner.x + inner.w - gap - hpW, top.y, hpW, topH);
  const pause = box(inner.x + inner.w / 2 - target / 2, top.y, target, target);
  const ageW = compact ? 96 : Math.max(96, Math.round(104 * Math.min(font, 1.5)));
  const advance = box(pause.x - gap - target, top.y, target, target);
  const age = box(advance.x - gap - ageW, top.y, ageW, topH);
  const clock = box(pause.x + target + gap, top.y, Math.round(Math.max(56, (compact ? 40 : 48) * Math.min(font, 1.6))), topH);
  // Gauges yield to the centre cluster; a narrow gauge shows icon + number only.
  const gaugeW = Math.min(hpW, age.x - gap - hpLeft.x, hpRight.x + hpRight.w - (clock.x + clock.w + gap));
  hpLeft.w = gaugeW;
  hpRight.x = round(hpRight.x + hpRight.w - gaugeW);
  hpRight.w = gaugeW;
  const minimapH = Math.round(Math.max(10, 8 * font));
  const minimap = box(inner.x + inner.w * 0.2, top.y + topH + 2, inner.w * 0.6, minimapH);

  // Bottom dock: economy, tray (6 + age toggle), queue strip, spells, sockets.
  // Compact landscape keeps six 48px cards; names move to tooltips (cost stays
  // on the card face) once text is enlarged. Wider screens grow cards with text.
  const cardW = compact ? target + 4 : Math.max(target, Math.round(64 * Math.min(font, 1.3)));
  const labelH = font >= 2 || (compact && font > 1) ? 0 : Math.round(12 * font + 2);
  const cardH = Math.max(target, cardW) + labelH;
  const dockH = cardH + 2 * gap;
  const dock = box(inner.x, inner.y + inner.h - dockH, inner.w, dockH);
  const spellD = compact ? target + 4 : Math.max(target, Math.round(cardW * 1.1));
  const socketW = target;

  const econW = Math.min(96, Math.max(76, Math.round(72 * font)));
  const econH = Math.max(cardH, Math.ceil(45 * font + 8));
  const economy = box(dock.x + gap, dock.y + dock.h - gap - econH, econW, econH);
  const spells = [0, 1].map(i =>
    box(dock.x + dock.w - gap - (2 - i) * (spellD + gap), dock.y + dock.h - gap - spellD, spellD, spellD));
  const socketsX = spells[0].x - gap - 2 * (socketW + gap);
  const sockets = [0, 1].map(i => box(socketsX + i * (socketW + gap), dock.y + dock.h - gap - socketW, socketW, socketW));

  const trayX = economy.x + economy.w + gap;
  const trayAvail = socketsX - gap - trayX;
  const toggleW = target;
  const step = Math.min(cardW + gap, (trayAvail - toggleW - gap) / TRAY_SLOTS);
  const fitW = step - gap;
  const cards = Array.from({length: TRAY_SLOTS}, (_, i) =>
    box(trayX + toggleW + gap + i * step, dock.y + gap, fitW, cardH));
  const ageToggle = box(trayX, dock.y + gap, toggleW, cardH);
  const tray = box(trayX, dock.y, trayAvail, dockH);
  const queueH = target; // each purchase is a focusable/touchable cancel target
  const queue = box(cards[0].x, dock.y - queueH - 2, Math.min(QUEUE_SLOTS * (fitW + gap), trayAvail - toggleW), queueH);

  // Camera: fit the 1,920-unit focus window to the width; put the lane band in
  // the free space between top HUD and queue strip, nearer the D03 desktop origin.
  const stageH = STAGE.height * scale;
  const availTop = minimap.y + minimap.h + 4;
  const availBottom = Math.min(queue.y, economy.y) - 4;
  const laneTopOffset = (STAGE.ground - LANE.above) * scale;
  const laneBottomOffset = (STAGE.ground + LANE.below) * scale;
  const preferred = (vh - stageH) * 0.35;
  const stageY = Math.max(availTop - laneTopOffset, Math.min(availBottom - laneBottomOffset, preferred));
  const stage = box(0, stageY, vw, stageH);
  const lane = box(0, stageY + laneTopOffset, vw, laneBottomOffset - laneTopOffset);
  const queueEntries = Array.from({length: QUEUE_SLOTS}, (_, i) =>
    box(queue.x + i * (queue.w / QUEUE_SLOTS), queue.y, queue.w / QUEUE_SLOTS, queueH));
  const hud = {top, hpLeft, hpRight, age, advance, pause, clock, minimap, dock, queue, economy, ageToggle,
               cards, spells, sockets};
  const interactive = [age, ageToggle, ...cards, ...spells, ...sockets, ...queueEntries, advance, pause];
  return {orientation: 'landscape', mode: 'battle', target, scale: round(scale), font, compact, stage, lane,
          bodyPx: round(108 * scale), labelH, hud, queueEntries, interactive, inner};
}

/** Invariants the D06 contract requires of every landscape layout. */
export function violations(result) {
  const out = [];
  if (result.mode !== 'battle') return out;
  const {hud, lane, interactive, target, inner} = result;
  for (const [name, region] of Object.entries(hud)) {
    for (const r of Array.isArray(region) ? region : [region]) {
      if (intersects(r, lane) && name !== 'minimap') out.push(`${name} covers the lane band`);
      if (r.x < inner.x - 0.5 || r.y < inner.y - 0.5 || r.x + r.w > inner.x + inner.w + 0.5 || r.y + r.h > inner.y + inner.h + 0.5) {
        out.push(`${name} leaves the safe area`);
      }
    }
  }
  if (intersects(hud.minimap, lane)) out.push('minimap covers the lane band');
  for (const r of interactive) if (r.w + 0.5 < target || r.h + 0.5 < target) out.push(`target ${r.w}x${r.h} < ${target}`);
  for (let i = 0; i < interactive.length; i++) for (let j = i + 1; j < interactive.length; j++) {
    if (intersects(interactive[i], interactive[j])) out.push(`controls ${i} and ${j} overlap`);
  }
  const topRow = [hud.hpLeft, hud.age, hud.advance, hud.pause, hud.clock, hud.hpRight];
  for (let i = 0; i < topRow.length - 1; i++) if (intersects(topRow[i], topRow[i + 1])) out.push(`top bar items ${i}/${i + 1} overlap`);
  if (result.bodyPx < 38) out.push(`unit body ${result.bodyPx}px is below the 38px readability floor`);
  return out;
}

export const VIEWPORTS = [
  {name: '844x390', vw: 844, vh: 390, input: 'touch', safe: {top: 0, right: 47, bottom: 21, left: 47}},
  {name: '1024x768', vw: 1024, vh: 768, input: 'touch', safe: {top: 20, right: 0, bottom: 20, left: 0}},
  {name: '1280x720', vw: 1280, vh: 720, input: 'pointer', safe: {top: 0, right: 0, bottom: 0, left: 0}},
  {name: '1920x1080', vw: 1920, vh: 1080, input: 'pointer', safe: {top: 0, right: 0, bottom: 0, left: 0}},
];
