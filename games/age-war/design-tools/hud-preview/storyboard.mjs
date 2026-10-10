// Local authored presentation timeline, never a reducer or a source of combat facts.
export const PERIOD = 11000;

export function poseAt(time, available) {
  const t = ((time % PERIOD) + PERIOD) % PERIOD;
  let clip = 'idle', elapsed = t;
  if (t < 1200) { clip = available.includes('walk') ? 'walk' : 'run'; }
  else if (t < 4000) { clip = 'attack'; elapsed = t - 1200; }
  else if (t < 6200) { clip = available.find(c => c.startsWith('skill_')) || 'attack'; elapsed = t - 4000; }
  else if (t < 6600) { clip = 'hit'; elapsed = t - 6200; }
  else if (t < 8000) { clip = 'death'; elapsed = t - 6600; }
  return {clip: available.includes(clip) ? clip : 'idle', elapsed};
}

export function sample(clip, elapsed, repeat = false) {
  if (!clip || !clip.frames.length) return null;
  const t = clip.loop || repeat ? elapsed % clip.duration : Math.min(elapsed, clip.duration);
  let result = clip.frames[0];
  for (const frame of clip.frames) { if (frame.t > t + 1e-6) break; result = frame; }
  return result;
}

export function nextAge(catalog, current) {
  const index = catalog.ages.findIndex(a => a.age === current);
  return catalog.ages[index + 1] || null;
}

// Friendly counter hypotheses from D01 MATH/CONTENT, not damage multipliers.
export const ROLES = {
  Frontline: ['Tiền tuyến', 'Giữ nhịp giao tranh; dùng tuyến trước bảo vệ quân bắn.'],
  Ranged: ['Tầm xa', 'Bắn sau tuyến chắn; dễ bị quân áp sát khi thiếu bảo vệ.'],
  Tank: ['Chống chịu', 'Hấp thụ áp lực; thử phối hợp quân xuyên giáp.'],
  Skirmisher: ['Đột kích', 'Áp sát quân bắn; gặp bất lợi trước tuyến chắn vững.'],
  Support: ['Hỗ trợ', 'Giữ đồng đội hoạt động; cần bảo vệ và chọn vị trí.'],
  Siege: ['Công thành', 'Gây áp lực lên công trình; nhịp chậm, cần quân che chắn.'],
};
