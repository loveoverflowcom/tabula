import {layout, violations, VIEWPORTS, STAGE} from './layout.mjs';
import {SCREENS, VERSIONS} from './states.mjs';
import {poseAt, sample, nextAge, ROLES, PERIOD} from './storyboard.mjs';

// Every number below is a named review fixture. No income, damage, training,
// cooldown, research or legality is advanced here; buttons log proposed intents.
const PRESETS = [
  ['battle', 'Trận đấu'], ['queue', 'Đang huấn luyện'], ['gold', 'Thiếu vàng'], ['population', 'Hết dân số'],
  ['queue-full', 'Hàng chờ đầy'], ['cooldown', 'Phép đang hồi'], ['targeting', 'Chọn vị trí phép'],
  ['research', 'Đang nâng thời đại'], ['transition', 'Đổi thời đại'], ['critical', 'Căn cứ nguy cấp'],
  ['fatigue', 'Mệt mỏi · 12 phút'], ['timeout', 'Sắp hết 20 phút'], ['pause', 'Tạm dừng'],
  ['victory', 'Chiến thắng'], ['defeat', 'Thất bại'], ['draw', 'Hoà'], ['setup', 'Chuẩn bị'],
  ['loading', 'Đang tải'], ['error', 'Lỗi tài nguyên'], ['native', 'Native chưa khả dụng'], ['tutorial', 'Hướng dẫn'],
];
const params = new URLSearchParams(location.search);
const app = document.querySelector('#app');
const el = (tag, text, className) => {
  const node = document.createElement(tag);
  if (text != null) node.textContent = text;
  if (className) node.className = className;
  return node;
};
const button = (text, action, className) => {
  const node = el('button', text, className);
  node.type = 'button'; node.addEventListener('click', action); return node;
};
const position = (node, rect) => {
  Object.assign(node.style, {left: `${rect.x}px`, top: `${rect.y}px`, width: `${rect.w}px`, height: `${rect.h}px`});
  return node;
};
const assetUrl = file => new URL(`assets/${file}`, location.href).href;
const picture = (record, alt = '') => {
  const node = el('img'); node.src = assetUrl(record.file); node.alt = alt; node.title = record.asset || record.resource || '';
  return node;
};
const selector = (label, options, selected, change) => {
  const wrap = el('label', label), select = el('select');
  for (const [value, text] of options) { const option = el('option', text); option.value = value; select.append(option); }
  select.value = selected; select.addEventListener('change', () => change(select.value)); wrap.append(select); return wrap;
};

async function boot() {
  const [catalogResponse, artResponse] = await Promise.all([fetch('catalog.generated.json'), fetch('assets/assets.json')]);
  if (!catalogResponse.ok || !artResponse.ok) throw new Error('Chưa stage đủ catalog/atlas. Xem README của hud-preview.');
  const catalog = await catalogResponse.json(), art = await artResponse.json();
  let age = catalog.ages.find(a => a.age === params.get('age')) || catalog.ages[3];
  let state = PRESETS.some(([id]) => id === params.get('state')) ? params.get('state') : 'battle';
  let viewport = VIEWPORTS.find(v => v.name === params.get('size')) || VIEWPORTS[2];
  let font = [1, 1.3, 2].includes(Number(params.get('font'))) ? Number(params.get('font')) : 1;
  let effects = ['full', 'low', 'reduced'].includes(params.get('effects')) ? params.get('effects') :
    (matchMedia('(prefers-reduced-motion: reduce)').matches ? 'reduced' : 'full');
  let playing = false, speed = 1, time = Number(params.get('t')) || 2800, previous = false;
  let selectedSpell = state === 'targeting' ? 0 : null, targetQ = 50000, cameraQ = 50000, lastUnit = null, tutorial = 0;
  let geometry, battle, canvas, queueFixture = [], imageEpoch = 0, toastTimer;
  let sheets = new Map();
  const log = [], tools = el('nav', null, 'review-tools'); tools.setAttribute('aria-label', 'Điều khiển bản thiết kế');
  const scroll = el('div', null, 'viewport-scroll'), info = el('section', null, 'review-info');
  const logText = el('pre', 'Chưa có intent.'), geometryText = el('pre'), status = el('p', '', 'evidence-note');
  status.setAttribute('role', 'status');
  app.append(tools, scroll, status, info);
  for (const [title, content] of [['Intent minh hoạ · không gửi vào game', logText], ['Bố cục / phiên bản / phần thiếu', geometryText]]) {
    const details = el('details'), summary = el('summary', title); details.append(summary, content); info.append(details);
  }
  const dialog = el('dialog'); dialog.style.setProperty('--review-font', font); document.body.append(dialog);
  let restoreFocus;
  function closeDialog() { dialog.close(); restoreFocus?.focus(); }
  dialog.addEventListener('cancel', event => { event.preventDefault(); closeDialog(); });
  function sheet(title, text, actions = [], body = null) {
    playing = false; restoreFocus = document.activeElement;
    dialog.replaceChildren(el('h2', title), el('p', text));
    if (body) dialog.append(body);
    const row = el('div', null, 'dialog-actions');
    for (const [label, action] of actions) row.append(button(label, action));
    row.append(button('Đóng · Esc', closeDialog)); dialog.append(row);
    if (!dialog.open) dialog.showModal();
  }
  function notice(text) {
    battle.querySelector('.notice')?.remove(); clearTimeout(toastTimer);
    const node = el('div', text, 'notice'); node.setAttribute('role', 'status'); battle.append(node);
    toastTimer = setTimeout(() => node.remove(), 3200);
  }
  function intent(kind, name, payload = {}) {
    const entry = {kind, name, payload, authority: false}; log.push(entry);
    logText.textContent = log.slice(-12).map(row => JSON.stringify(row)).join('\n');
    notice(`${name} · intent minh hoạ`); return entry;
  }
  function blockedReason(unit) {
    if (state === 'gold') return `Thiếu ${unit.cost} vàng (fixture có 0)`;
    if (state === 'population') return 'Đã đủ 24 dân số';
    if (state === 'queue-full') return 'Hàng chờ đã đủ 5';
    return '';
  }
  function recruit(unit) {
    const reason = blockedReason(unit); if (reason) { notice(reason); return; }
    intent('command', 'Recruit', {unit_id: unit.id});
  }
  function detail(unit) {
    lastUnit = unit; intent('local', 'ShowDetails', {unit: unit.id});
    const [role, hint] = ROLES[unit.role];
    sheet(unit.name_vi, `${role} · ${unit.cost} vàng · ${unit.pop} dân số · ${unit.train_s}s huấn luyện · ${unit.hp} HP. ${hint} Giả thuyết khắc chế chưa cân bằng bằng mô phỏng. Kỹ năng: ${unit.skills.join(', ')}.`, [
      ['Tuyển · intent', () => { closeDialog(); recruit(unit); }],
    ]);
  }
  function showCodex() {
    intent('local', 'ShowCodex'); const grid = el('div', null, 'codex');
    for (const era of catalog.ages) for (const unit of era.units) {
      const b = button(`${era.label_vi} · ${unit.name_vi}\n${ROLES[unit.role][0]} · ${unit.cost} vàng`, () => detail(unit));
      b.prepend(picture(art.ages[era.age].units[unit.id].icon)); grid.append(b);
    }
    sheet('Bách khoa · 36 quân', 'Chỉ 6 quân của bộ thời đại đang chọn nằm trên HUD. Quân đang sống và hàng chờ cũ giữ nguyên sau nâng age.', [], grid);
  }
  function spellSelect(slot) {
    if (state === 'cooldown') { notice('Đang hồi · còn 12s (fixture)'); return; }
    if (state === 'gold') { notice('Thiếu vàng để dùng phép'); return; }
    selectedSpell = slot; intent('local', 'SelectSpell', {slot}); renderBattle();
    canvas.focus();
  }
  function cancelAim() { selectedSpell = null; intent('local', 'CancelAim'); renderBattle(); }
  function confirmCast() {
    if (selectedSpell === null) return;
    intent('command', 'Cast', {slot: selectedSpell + 1, target_q: targetQ});
    selectedSpell = null; renderBattle();
  }
  function advance() {
    const next = nextAge(catalog, age.age);
    if (!next) { notice('Đã ở thời đại cuối'); return; }
    if (state === 'research') { notice('Ô nghiên cứu đang bận · không thể huỷ'); return; }
    if (state === 'gold') { notice(`Cần ${next.advance_to.gold} vàng và tổng XP ≥ ${next.advance_to.total_xp}`); return; }
    sheet(`Nâng lên ${next.label_vi}`, `${next.advance_to.gold} vàng · tổng XP ≥ ${next.advance_to.total_xp} · ${next.advance_to.duration_s}s. Không hồi HP căn cứ, không reset phép hoặc đổi giá quân đã mua.`, [
      ['Nâng · intent', () => { closeDialog(); intent('command', 'AdvanceAge'); }],
      ['Xem fixture chuyển age', () => { closeDialog(); previous = false; age = next; state = 'transition'; render(); }],
    ]);
  }
  function techSheet() {
    intent('local', 'OpenTech'); const body = el('div', null, 'codex');
    const eras = [age]; if (age.number > 1) eras.push(catalog.ages[age.number - 2]);
    for (const era of eras) for (const tech of era.techs) body.append(button(
      `${era.label_vi} · ${tech.kind === 'Attack' ? 'Công kích' : 'Phòng thủ'} · ${tech.cost} vàng · ${tech.research_s}s`,
      () => { closeDialog(); if (state === 'research' || state === 'gold') notice('Fixture: chưa thể nghiên cứu'); else intent('command', 'Research', {tech_id: tech.id}); }));
    sheet('Nghiên cứu', 'Chung một ô với nâng thời đại; mỗi tech một lần, chỉ age hiện tại/liền trước. Không huỷ, không reset HP. C01 kiểm tra cap và legality.', [['Nâng thời đại', advance]], body);
  }
  function socketSheet(socket) {
    intent('local', 'OpenSocket', {socket}); const body = el('div', null, 'codex');
    for (const turret of age.turrets) {
      body.append(button(`${turret.name_vi} · ${turret.branch} · ${turret.cost} vàng · ${turret.build_s}s`, () => {
        closeDialog(); if (state === 'gold') notice('Thiếu vàng'); else intent('command', 'BuildTurret', {socket, turret_id: turret.id});
      }));
      body.append(button(`Cải tạo cùng nhánh · ${turret.name_vi}`, () => { closeDialog(); intent('command', 'RefitTurret', {socket, turret_id: turret.id}); }));
    }
    body.append(button('Bán · xác nhận theo HP', () => sheet('Bán tháp?', 'Hoàn floor(giá đã trả × 0.5 × HP hiện tại / HP tối đa); không bán khi đang xây/cải tạo. Ví dụ HP đủ và giá 100 → 50 vàng. Giá hoàn thực tế phải do View cung cấp.', [
      ['Bán · intent', () => { closeDialog(); intent('command', 'SellTurret', {socket}); }],
    ])));
    sheet(`Ô tháp ${socket + 1}`, 'Hai nhánh cố định. Art tháp còn PARTIAL; chưa minh hoạ công trình bằng hình thay thế.', [], body);
  }
  function queueSheet(index) {
    const unit = queueFixture[index]; if (!unit) { notice('Ô hàng chờ trống'); return; }
    const purchase = `fixture-purchase-${index + 1}`;
    intent('local', 'SelectQueueEntry', {purchase});
    const refund = index === 0 ? Math.floor(unit.cost * .75) : unit.cost;
    sheet(`Huỷ ${unit.name_vi}?`, `${index === 0 ? 'Đầu hàng đang huấn luyện: 75%' : 'Đang chờ: 100%'} giá đã trả. Fixture hoàn ${refund} vàng và ${unit.pop} dân số. Production dùng refund từ projection.`, [
      [`Huỷ · hoàn ${refund} · intent`, () => { closeDialog(); intent('command', 'CancelRecruit', {purchase}); }],
    ]);
  }
  function pauseSheet() {
    intent('system', 'Pause'); sheet('Tạm dừng', 'Đề xuất offline PVE: dừng clock logic, bot và trình diễn. Resume không cộng thời gian nền. Preview chỉ dừng hoạt ảnh.', [
      ['Tiếp tục', () => { closeDialog(); state = 'battle'; intent('system', 'Resume'); render(); }],
      ['Bách khoa', showCodex],
      ['Chơi lại', () => sheet('Chơi lại trận?', 'Production cần host kết thúc phiên cũ và khởi tạo trận mới với cùng cấu hình.', [['Xác nhận · intent', () => { closeDialog(); intent('system', 'Restart'); }]])],
      ['Thoát', () => sheet('Thoát trận?', 'Trận local chưa lưu sẽ bị bỏ; không có resume ở phạm vi này.', [['Xác nhận · intent', () => { closeDialog(); intent('system', 'Quit'); }]])],
      ['Đầu hàng', () => sheet('Đầu hàng?', 'Resign là lệnh rules, khác với thoát host. Production chờ kết quả được chiếu.', [['Đầu hàng · intent', () => { closeDialog(); intent('command', 'Resign'); }]])],
    ]);
  }
  function presetSheet() {
    if (state === 'pause') pauseSheet();
    else if (['victory', 'defeat', 'draw'].includes(state)) sheet(
      {victory: 'Chiến thắng', defeat: 'Thất bại', draw: 'Hoà'}[state],
      'Fixture kết quả · shell nhận outcome từ GameHost rồi hiện tài liệu kết quả. Hoà khi hai căn cứ chết cùng tick hoặc HP fraction bằng nhau ở hard limit. Không dùng vàng/age để phá hoà.', [
        ['Đấu lại · intent', () => { closeDialog(); intent('system', 'Rematch', {same_config: true, new_seed: true}); }],
        ['Về thư viện · intent', () => { closeDialog(); intent('system', 'Quit'); }],
      ]);
    else if (state === 'setup') {
      const body = el('div');
      body.append(selector('Độ khó', [['easy', 'Dễ'], ['normal', 'Thường'], ['hard', 'Khó']], 'normal', () => {}));
      body.append(el('p', 'Ba policy bot cùng projection và tài nguyên; chưa có bot chạy thật. Tutorial có thể bật trước trận.'));
      sheet('Age War · chuẩn bị', 'Chrome thuộc CMP/web shell. Đưa cấu hình đã kiểm tra sang đúng một GameHost; không có HUD thứ hai trong CMP.', [
        ['Bắt đầu · intent', () => { const difficulty = body.querySelector('select').value; closeDialog(); intent('system', 'Launch', {difficulty, start_age: age.age, tutorial: true, effects, audio: false}); }],
      ], body);
    } else if (state === 'loading') sheet('Đang chuẩn bị tài nguyên', 'Fixture: xác minh 42/100 byte. Production lấy tiến độ thật; không có thanh tải giả theo đồng hồ.', [['Huỷ · intent', () => { closeDialog(); intent('system', 'CancelLaunch'); }]]);
    else if (state === 'error' || state === 'native') sheet(
      state === 'native' ? 'Gameplay native chưa khả dụng' : 'Không thể xác minh tài nguyên',
      state === 'native' ? 'ADR-0043: cần adapter và runtime/assets native trong cùng app CMP. Preview HTML không thay thế native GameHost.' : 'Fixture ASSET_INTEGRITY: cần đúng pack@version và giới hạn bộ nhớ. Giữ lý do lỗi trên màn hình.', [
        ['Thử lại · intent', () => intent('system', 'Retry', {pack: `age-war-${age.age.toLowerCase()}@0.5.0`})],
        ['Thoát · intent', () => { closeDialog(); intent('system', 'Quit'); }],
      ]);
    else if (state === 'tutorial') tutorialSheet();
  }
  function tutorialSheet() {
    const tips = ['Tuyển một quân bằng thẻ dưới cùng hoặc phím 1–6.', 'Vàng, dân số và 5 ô chờ nằm bên trái/dưới. Chỉ đầu hàng được huấn luyện.', 'Tiền tuyến là vùng ngang trống; kéo để xem làn. Marker tròn/kim cương phân biệt hai phe.', 'Q/E chọn phép, chạm vị trí, rồi Xác nhận. Esc/Back huỷ trước.', 'Nâng age tiêu vàng và đòi tổng XP; quân và phép cũ không reset.'];
    sheet(`Hướng dẫn ${tutorial + 1}/5`, tips[tutorial], [
      [tutorial === 4 ? 'Hoàn tất' : 'Tiếp', () => { intent('local', 'NextTip'); if (tutorial === 4) closeDialog(); else { tutorial++; tutorialSheet(); } }],
      ['Bỏ qua', () => { closeDialog(); intent('local', 'SkipTutorial'); }],
    ]);
  }

  function renderTools() {
    const brand = el('strong', 'Tabula / D06'), mark = el('img');
    mark.src = 'tabula-mark.svg'; mark.alt = ''; mark.width = 24; mark.height = 24; brand.prepend(mark);
    tools.replaceChildren(brand);
    tools.append(
      selector('Age', catalog.ages.map(a => [a.age, a.label_vi]), age.age, value => { age = catalog.ages.find(a => a.age === value); previous = false; selectedSpell = null; render(); }),
      selector('State', PRESETS, state, value => { closeDialog(); state = value; selectedSpell = value === 'targeting' ? 0 : null; render(); }),
      selector('Khung', [...VIEWPORTS.map(v => [v.name, v.name]), ['portrait', '390×844 · xoay ngang']], viewport.name, value => { viewport = VIEWPORTS.find(v => v.name === value) || {name: 'portrait', vw: 390, vh: 844, input: 'touch', safe: {top: 47, right: 0, bottom: 34, left: 0}}; render(); }),
      selector('Chữ', [['1', '100%'], ['1.3', '130%'], ['2', '200%']], String(font), value => { font = Number(value); render(); }),
      selector('VFX', [['full', 'Đầy đủ'], ['low', 'Ít hiệu ứng'], ['reduced', 'Giảm chuyển động']], effects, value => { effects = value; intent('local', 'SetSessionEffects', {effects}); draw(); }),
      selector('Tốc độ', [['0.5', '0.5×'], ['1', '1×'], ['2', '2×']], String(speed), value => { speed = Number(value); }),
      button(playing ? 'Dừng motion' : 'Xem motion', () => { playing = !playing; renderTools(); }),
      button('Bách khoa · C', showCodex),
      selector('Theme', [['dark', 'Tối'], ['light', 'Sáng']], document.documentElement.dataset.theme, value => { document.documentElement.dataset.theme = value; draw(); }),
    );
    const range = el('input'); range.type = 'range'; range.min = '0'; range.max = String(PERIOD - 1); range.step = '1'; range.value = String(time % PERIOD);
    range.setAttribute('aria-label', 'Thời gian storyboard, millisecond'); range.addEventListener('input', () => { playing = false; time = Number(range.value); draw(); });
    tools.append(range);
  }
  function control(text, rect, action, label, css = '') {
    const node = button(text, action, `control ${css}`); node.setAttribute('aria-label', label || text); node.title = label || text;
    battle.append(position(node, rect)); return node;
  }
  function renderBattle() {
    geometry = layout({...viewport, font});
    battle = el('section', null, 'battle'); battle.style.width = `${viewport.vw}px`; battle.style.height = `${viewport.vh}px`; battle.style.setProperty('--review-font', font);
    battle.setAttribute('aria-label', 'Chiến trường · design preview, không phải game'); scroll.replaceChildren(battle);
    dialog.style.setProperty('--review-font', font);
    if (geometry.mode === 'rotate') {
      const prompt = position(el('div', null, 'hud panel'), geometry.prompt);
      prompt.style.padding = '16px'; prompt.append(el('h2', 'Xoay ngang để tiếp tục'), el('p', 'Trận sẽ tạm dừng. Quay lại landscape rồi chọn Tiếp tục; không tự thu nhỏ HUD.'), button('Thoát · intent', () => intent('system', 'Quit'))); battle.append(prompt); return;
    }
    const h = geometry.hud, resources = art.ages[age.age];
    battle.append(position(picture(resources.backdrop), geometry.stage)); battle.firstChild.className = 'stage';
    canvas = el('canvas'); canvas.style.position = 'absolute'; canvas.style.width = '100%'; canvas.style.height = '100%';
    const dpr = Math.min(devicePixelRatio || 1, 2); canvas.width = viewport.vw * dpr; canvas.height = viewport.vh * dpr;
    canvas.tabIndex = 0; canvas.setAttribute('aria-label', 'Làn quân. Kéo để xem, chọn phép rồi chạm vị trí; phím mũi tên di chuyển, Enter xác nhận, Escape huỷ.'); battle.append(canvas);
    let drag = null;
    canvas.addEventListener('pointerdown', event => { drag = {x: event.clientX, cameraQ}; canvas.setPointerCapture(event.pointerId); });
    canvas.addEventListener('pointermove', event => {
      if (!drag) return;
      if (selectedSpell === null) cameraQ = Math.max(10000, Math.min(90000, drag.cameraQ - (event.clientX - drag.x) / viewport.vw * 10000));
      else targetQ = Math.max(5000, Math.min(95000, cameraQ - 5000 + (event.clientX - canvas.getBoundingClientRect().left) / viewport.vw * 10000));
      draw();
    });
    canvas.addEventListener('pointerup', event => {
      if (selectedSpell !== null) { targetQ = Math.round(Math.max(5000, Math.min(95000, cameraQ - 5000 + (event.clientX - canvas.getBoundingClientRect().left) / viewport.vw * 10000))); intent('local', 'MoveAim', {x_q: targetQ}); renderBattle(); }
      else if (drag) intent('local', 'PanCamera', {x_q: Math.round(cameraQ)});
      drag = null;
    });
    canvas.addEventListener('pointercancel', () => { drag = null; });
    canvas.addEventListener('contextmenu', event => { event.preventDefault(); if (selectedSpell !== null) cancelAim(); });
    battle.append(position(el('div', '', 'hud panel topbar'), h.top), position(el('div', '', 'hud panel dock'), h.dock));
    for (const [enemy, rect] of [[false, h.hpLeft], [true, h.hpRight]]) {
      const critical = !enemy && state === 'critical'; const hp = critical ? 620 : enemy ? 2460 : 2780;
      const node = position(el('div', `${enemy ? '◆' : '●'} ${hp}`, `hud hp ${enemy ? 'enemy' : ''} ${critical ? 'critical' : ''}`), rect);
      node.setAttribute('aria-label', `${enemy ? 'Địch' : 'Ta'}: ${hp}/3000 HP${critical ? ', nguy cấp' : ''}`);
      const progress = el('progress'); progress.max = 3000; progress.value = hp; progress.setAttribute('aria-hidden', 'true'); node.append(progress); battle.append(node);
    }
    control(geometry.compact && font > 1 ? ['I','II','III','IV','V','VI'][age.number - 1] : age.label_vi,
      h.age, techSheet, `${age.label_vi} · mở nghiên cứu · R`);
    const next = nextAge(catalog, age.age);
    control(state === 'research' ? '3s' : '↑', h.advance, advance, next ? `Nâng lên ${next.label_vi} · ${next.advance_to.gold} vàng · XP ${next.advance_to.total_xp} · U` : 'Thời đại cuối');
    control('Ⅱ', h.pause, pauseSheet, 'Tạm dừng · Escape');
    battle.append(position(el('div', state === 'fatigue' ? '12:00' : state === 'timeout' ? '19:50' : '04:32', 'hud clock'), h.clock));
    const minimap = position(el('div', null, 'hud minimap'), h.minimap); minimap.setAttribute('aria-hidden', 'true'); battle.append(minimap);
    const econ = position(el('div', null, 'hud economy'), h.economy);
    econ.append(el('span', state === 'gold' ? '◈ 0' : '◈ 680'), el('span', state === 'population' ? '24/24' : '16/24'), el('span', 'XP 280')); econ.setAttribute('aria-label', `Vàng ${state === 'gold' ? 0 : 680}, dân số ${state === 'population' ? 24 : 16}/24, tổng XP 280`); battle.append(econ);
    const trayAge = previous && age.number > 1 ? catalog.ages[age.number - 2] : age;
    const toggle = control(previous ? '↩' : '↔', h.ageToggle, () => { if (age.number === 1) { notice('Chưa có age liền trước'); return; } previous = !previous; intent('local', 'ToggleAgeSet', {set: previous ? 'previous' : 'current'}); renderBattle(); }, 'Đổi bộ quân hiện tại/liền trước · G');
    toggle.setAttribute('aria-pressed', String(previous));
    for (const [index, unit] of trayAge.units.entries()) {
      const record = art.ages[trayAge.age].units[unit.id].icon, reason = blockedReason(unit);
      const node = control('', h.cards[index], () => { if (!node.dataset.longPress) recruit(unit); delete node.dataset.longPress; }, `${unit.name_vi} · ${ROLES[unit.role][0]} · ${unit.cost} vàng · ${unit.pop} dân số · ${unit.train_s}s${reason ? ` · ${reason}` : ''} · phím ${index + 1}`);
      node.append(picture(record), el('span', `${unit.cost}`, 'price')); if (geometry.labelH) node.append(el('span', unit.name_vi, 'short-name'));
      node.setAttribute('aria-disabled', String(!!reason)); node.addEventListener('focus', () => { lastUnit = unit; });
      node.addEventListener('contextmenu', event => { event.preventDefault(); detail(unit); });
      let hold;
      node.addEventListener('pointerdown', () => { hold = setTimeout(() => { node.dataset.longPress = 'true'; detail(unit); }, 500); });
      for (const name of ['pointerup', 'pointercancel', 'pointerleave']) node.addEventListener(name, () => clearTimeout(hold));
    }
    queueFixture = ['queue', 'queue-full'].includes(state) ? age.units.slice(0, state === 'queue-full' ? 5 : 2) : [];
    for (const [index, rect] of geometry.queueEntries.entries()) {
      const unit = queueFixture[index];
      const node = control(unit ? `${index === 0 ? '◷ ' : ''}${unit.cost}` : '·', rect, () => queueSheet(index), unit ? `${unit.name_vi} · ${index === 0 ? 'đang huấn luyện, 75% hoàn' : 'đang chờ, 100% hoàn'} · chọn để huỷ` : `Ô chờ ${index + 1} trống`, 'queue-entry');
      node.setAttribute('aria-disabled', String(!unit));
    }
    for (const [index, rect] of h.sockets.entries()) control(`♜${index + 1}`, rect, () => socketSheet(index), `Ô tháp ${index + 1} · xây/cải tạo/bán · T`, 'socket');
    for (const [index, spell] of age.spells.entries()) {
      const node = control('', h.spells[index], () => spellSelect(index), `${spell.name_vi} · ${spell.cost} vàng · hồi ${spell.cooldown_s}s${state === 'cooldown' ? ' · còn 12s' : ''} · ${index ? 'E' : 'Q'}`, 'spell');
      const record = resources.spells[spell.id]; if (record) node.append(picture(record));
      node.append(el('span', state === 'cooldown' ? '12s' : String(spell.cost), 'price')); node.setAttribute('aria-disabled', String(state === 'cooldown' || state === 'gold'));
    }
    const note = position(el('span', 'D03/D05 art · base/turret art chưa đủ · marker ● ta / ◆ địch', 'lane-note'), {x: viewport.safe.left + 4, y: geometry.lane.y + geometry.lane.h + 8, w: Math.min(490, geometry.inner.w - 8), h: 18}); battle.append(note);
    if (selectedSpell !== null) {
      const row = el('div', null, 'aim-actions'); row.style.left = `${h.queue.x}px`; row.style.top = `${geometry.lane.y + geometry.lane.h + 30}px`;
      row.append(button('Xác nhận phép · Enter', confirmCast, 'primary'), button('Huỷ · Esc', cancelAim)); battle.append(row);
    }
    if (['transition', 'critical', 'fatigue', 'timeout'].includes(state)) notice({transition: `Đã sang ${age.label_vi} · quân cũ và phép giữ nguyên`, critical: 'Căn cứ nguy cấp · HP dưới 25%', fatigue: '12:00 · Mệt mỏi gây mất HP hai căn cứ mỗi giây', timeout: 'Còn 10s · sẽ so tỷ lệ HP căn cứ'}[state]);
    const problems = violations(geometry);
    geometryText.textContent = JSON.stringify({versions: VERSIONS, viewport, font, lane: geometry.lane, layout_problems: problems, missing: ['base art', 'production HUD icons', 'complete turret art', 'SourceQA', 'owner/rights approval', 'native/browser acceptance']}, null, 2);
    status.textContent = `${viewport.name} · ${font * 100}% · ${age.label_vi} · fixture ${state} · ${problems.length ? problems.join('; ') : 'bố cục hình học PASS'} · C01 gate PENDING`;
    draw();
  }
  async function loadSheets() {
    const epoch = ++imageEpoch, pending = new Map(); sheets = new Map();
    try {
      await Promise.all(Object.keys(art.ages[age.age].motion.files).map(async file => {
        const image = new Image(); image.src = assetUrl(file); await image.decode(); pending.set(file, image);
      }));
      if (epoch === imageEpoch) { sheets = pending; draw(); }
    } catch (error) { if (epoch === imageEpoch) { playing = false; status.textContent = `Lỗi atlas: ${error.message}`; } }
  }
  function drawSprite(ctx, sprite, box, x, y, scale) {
    const image = sheets.get(sprite.file); if (!image) return;
    const r = sprite.rect || {x: 0, y: 0, width: image.width, height: image.height};
    ctx.drawImage(image, r.x, r.y, r.width, r.height, x + box[0] * scale, y + box[1] * scale, box[2] * scale, box[3] * scale);
  }
  function draw() {
    if (!canvas || geometry.mode !== 'battle') return;
    const ctx = canvas.getContext('2d'), dpr = canvas.width / viewport.vw, scale = viewport.vw / STAGE.width;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0); ctx.clearRect(0, 0, viewport.vw, viewport.vh);
    const motion = art.ages[age.age].motion, ground = geometry.stage.y + STAGE.ground * scale;
    const colours = getComputedStyle(document.documentElement);
    for (const [side, facing] of ['right', 'left'].entries()) for (const [index, unit] of age.units.entries()) {
      const names = Object.keys(motion.clips).filter(k => k.startsWith(`${unit.id}/${facing}/`)).map(k => k.split('/')[2]);
      const pose = poseAt(time, names), clip = motion.clips[`${unit.id}/${facing}/${pose.clip}`];
      const frame = sample(clip, pose.elapsed, pose.clip === 'attack' || pose.clip.startsWith('skill_'));
      const x = (side === 0 ? 280 + index * 118 : 1640 - index * 118) * scale - (cameraQ - 50000) / 10000 * viewport.vw;
      ctx.strokeStyle = colours.getPropertyValue(`--sys-color-team-${side + 1}`).trim(); ctx.lineWidth = 2;
      ctx.beginPath();
      if (!side) ctx.ellipse(x, ground + 3 * scale, 17 * scale, 5 * scale, 0, 0, Math.PI * 2);
      else { ctx.moveTo(x, ground - 4 * scale); ctx.lineTo(x + 18 * scale, ground + 3 * scale); ctx.lineTo(x, ground + 10 * scale); ctx.lineTo(x - 18 * scale, ground + 3 * scale); ctx.closePath(); }
      ctx.stroke(); if (frame) drawSprite(ctx, frame.sprite, frame.box, x, ground, scale);
      // Static fixture status cue persists in all effects modes; production
      // will use projected status facts and the Board Reader from the same View.
      if (unit.role === 'Support' || unit.role === 'Tank') {
        ctx.font = '12px system-ui'; ctx.fillStyle = ctx.strokeStyle;
        ctx.fillText(unit.role === 'Support' ? '+' : '◇', x - 4, ground - 116 * scale);
      }
    }
    // Commander spell showcase uses two authored cue times. It awards no hit.
    const t = time % PERIOD;
    for (const [index, spell] of age.spells.entries()) {
      const elapsed = t - [2600, 4600][index]; if (elapsed < 0 || elapsed > 900 || effects === 'reduced') continue;
      const phase = elapsed < 400 ? 'start' : elapsed < 650 ? 'contact' : 'end';
      const frames = motion.vfx[`${spell.id}/A/${phase}`]; if (!frames?.length) continue;
      const f = frames[effects === 'low' ? 0 : Math.min(1, frames.length - 1)];
      drawSprite(ctx, f.sprite, [-f.pivot[0], -f.pivot[1], ...f.size], (index ? 1050 : 900) * scale, ground, 2 * scale);
    }
    if (selectedSpell !== null) {
      const spell = age.spells[selectedSpell], x = (targetQ - cameraQ + 5000) / 10000 * viewport.vw;
      ctx.strokeStyle = colours.getPropertyValue('--sys-color-legal-target').trim(); ctx.lineWidth = 2;
      ctx.setLineDash([6, 4]); ctx.beginPath(); ctx.ellipse(x, ground, spell.aoe_radius_q / 10000 * viewport.vw, 16, 0, 0, Math.PI * 2); ctx.stroke(); ctx.setLineDash([]);
    }
  }
  function render() { renderTools(); renderBattle(); loadSheets(); presetSheet(); }
  document.addEventListener('keydown', event => {
    if (dialog.open || event.ctrlKey || event.metaKey || event.altKey || ['INPUT', 'SELECT', 'TEXTAREA'].includes(event.target.tagName)) return;
    // Native buttons keep Enter/Space and ordinary Tab focus semantics.
    if (event.key === 'Escape') { event.preventDefault(); if (selectedSpell !== null) cancelAim(); else pauseSheet(); }
    else if (event.key.toLowerCase() === 'q') spellSelect(0);
    else if (event.key.toLowerCase() === 'e') spellSelect(1);
    else if (event.key.toLowerCase() === 'r') techSheet();
    else if (event.key.toLowerCase() === 'u') advance();
    else if (event.key.toLowerCase() === 't') socketSheet(0);
    else if (event.key.toLowerCase() === 'c') showCodex();
    else if (event.key.toLowerCase() === 'i' && lastUnit) detail(lastUnit);
    else if (/^[1-6]$/.test(event.key)) recruit((previous && age.number > 1 ? catalog.ages[age.number - 2] : age).units[Number(event.key) - 1]);
    else if (event.key.toLowerCase() === 'g') { previous = age.number > 1 && !previous; intent('local', 'ToggleAgeSet', {set: previous ? 'previous' : 'current'}); renderBattle(); }
    else if (event.key === 'Backspace' && queueFixture.length) { event.preventDefault(); queueSheet(0); }
    else if (event.key === 'Enter' && selectedSpell !== null && event.target === canvas) { event.preventDefault(); confirmCast(); }
    else if (['ArrowLeft', 'ArrowRight'].includes(event.key) && event.target === canvas) {
      event.preventDefault(); const direction = event.key === 'ArrowLeft' ? -1 : 1;
      if (selectedSpell !== null) { targetQ = Math.max(5000, Math.min(95000, targetQ + direction * 500)); intent('local', 'MoveAim', {x_q: targetQ}); }
      else { cameraQ = Math.max(10000, Math.min(90000, cameraQ + direction * 500)); intent('local', 'PanCamera', {x_q: cameraQ}); }
      draw();
    }
  });
  // Navigation shortcuts never manufacture a host exit or cancel the browser Back action.
  // Actual Android/iOS system Back must be mapped by the native host (unimplemented).
  let prior;
  function tick(now) {
    if (playing && prior != null && !document.hidden && !dialog.open) { time += Math.min(now - prior, 100) * speed; draw(); }
    prior = now; requestAnimationFrame(tick);
  }
  document.addEventListener('visibilitychange', () => { if (document.hidden) { playing = false; renderTools(); } });
  render(); requestAnimationFrame(tick);
}

boot().catch(error => { app.replaceChildren(el('div', error.message, 'boot-error')); });
