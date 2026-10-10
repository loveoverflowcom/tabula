// Offline presentation-only controller. Markers never create canonical facts.
const smooth = x => { x = Math.max(0, Math.min(1, x)); return x * x * (3 - 2 * x); };
export function blendPose(a, b, t) {
  if (t <= 0) return a;
  if (t >= 1) return b;
  if (typeof a === 'number') return a + (b - a) * t;
  if (Array.isArray(a)) return a.map((v, i) => blendPose(v, b[i], t));
  if (a && typeof a === 'object') {
    return Object.fromEntries(Object.keys(a).map(k => [k, blendPose(a[k], b[k], t)]));
  }
  return t < .5 ? a : b;
}
function validateClip(clip) {
  if (!Number.isFinite(clip.duration_ms) || clip.duration_ms <= 0) throw new Error('invalid clip duration');
  for (const marker of clip.markers || []) {
    if (!Number.isFinite(marker.time_ms) || marker.time_ms < 0 || marker.time_ms > clip.duration_ms) throw new Error('invalid visual marker time');
  }
}
export class PresentationController {
  constructor(rig, clip, sampler) {
    if (typeof sampler !== 'function') throw new Error('a presentation pose sampler is required');
    validateClip(clip);
    Object.assign(this, {rig, clip, sampler, time_ms:0, speed:1, generation:0, blend_from:null, cues:[], started:false});
  }
  advance(delta_ms) {
    if (![.5, 1, 2].includes(this.speed)) throw new Error('unsupported presentation speed');
    if (!Number.isFinite(delta_ms) || delta_ms < 0) throw new Error('invalid presentation delta');
    validateClip(this.clip);
    const before = this.time_ms, next = before + delta_ms * this.speed;
    if (!Number.isFinite(next) || next > Number.MAX_SAFE_INTEGER) throw new Error('presentation time overflow');
    const initial = !this.started && next > before, pending = [];
    for (const [ordinal, m] of (this.clip.markers || []).entries()) {
      const duration = this.clip.duration_ms;
      const first = this.clip.loop ? Math.max(0, Math.floor((before - m.time_ms) / duration) + 1) : 0;
      const last = this.clip.loop ? Math.floor((next - m.time_ms) / duration) : 0;
      // Zero-time is included once only when a new phase actually starts advancing.
      const start = initial && m.time_ms === 0 ? 0 : first;
      if (!Number.isSafeInteger(start) || !Number.isSafeInteger(last)) throw new Error('visual marker cycle overflow');
      if (last - start > 4096) throw new Error('visual marker catch-up budget exceeded');
      for (let cycle = start; cycle <= last; cycle++) {
        const at = m.time_ms + (this.clip.loop ? cycle * duration : 0);
        if ((at > before && at <= next) || (initial && at === 0)) {
          pending.push({at, ordinal, cue:{...m, authority:false, generation:this.generation, ...(this.clip.loop ? {cycle} : {})}});
          if (pending.length > 4096) throw new Error('visual marker catch-up budget exceeded');
        }
      }
    }
    pending.sort((a, b) => a.at - b.at || a.ordinal - b.ordinal);
    this.time_ms = next;
    this.started ||= next > before;
    this.cues.push(...pending.map(x => x.cue));
    return this.poseAt(next);
  }
  poseAt(time_ms) {
    const p = this.sampler(this.rig, this.clip, time_ms);
    return this.blend_from && time_ms < 120 ? blendPose(this.blend_from, p, smooth(time_ms / 120)) : p;
  }
  interrupt(target) {
    validateClip(target);
    const from = this.poseAt(this.time_ms);
    Object.assign(this, {clip:target, time_ms:0, cues:[], started:false, blend_from:from});
    this.generation++;
  }
  reset(target = this.clip) {
    validateClip(target);
    Object.assign(this, {clip:target, time_ms:0, cues:[], started:false, blend_from:null});
    this.generation++;
  }
}
