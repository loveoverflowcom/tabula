"""Bounded design-only audio scheduler. It never changes gameplay outcomes."""
import math


def family_key(asset):
    key = asset['id']
    if key.startswith('weapon_'): return key.rsplit('_v', 1)[0], 70
    if key.startswith('impact_'): return key.rsplit('_v', 1)[0], 60
    if asset['group'].startswith('auxiliary'): return key.split('_attack')[0], 100
    if asset['group'] == 'system': return key.rsplit('_v', 1)[0], 5000 if key.startswith('base_critical') else 80
    return key.split('_contact')[0].split('_start')[0].split('_end')[0], 60


def schedule(events, index, speed=1, limit=24):
    ordered = sorted(events, key=lambda e: (e['presentation_ms'], -index[e['audio_id']]['priority']))
    active, cooldown, accepted, dropped = [], {}, [], []
    peak_voices = 0
    for e in ordered:
        a = index[e['audio_id']]
        now = e['presentation_ms'] / speed
        active = [x for x in active if x['end'] > now]
        family, gap = family_key(a)
        emitter = e.get('entity_id', e.get('spell_id', 'system')) + ':' + str(e.get('team', e.get('pan', 0)))
        if cooldown.get(family, -math.inf) + gap > now:
            dropped.append({**e, 'reason': 'shared_family_cooldown', 'now_ms': now}); continue
        if sum(x['emitter'] == emitter for x in active) >= 2:
            dropped.append({**e, 'reason': 'per_emitter_cap2', 'now_ms': now}); continue
        priority = a['priority']
        category = 'critical' if priority >= 90 else 'commander' if priority >= 85 else 'UI' if a['group'] == 'system' else 'world'
        cap = {'critical': 2, 'commander': 4, 'UI': 2, 'world': 16}[category]
        if sum(x['category'] == category for x in active) >= cap or len(active) >= limit:
            lower = [x for x in active if x['category'] == 'world' and x['priority'] < priority]
            if not lower:
                dropped.append({**e, 'reason': 'voice_budget', 'now_ms': now}); continue
            victim = sorted(lower, key=lambda x: (x['priority'], x['start']))[0]
            victim['stop_ms'] = now
            active.remove(victim)
        item = {**e, 'start': now, 'end': now + a['duration_ms'], 'priority': priority, 'category': category, 'emitter': emitter}
        accepted.append(item); active.append(item); cooldown[family] = now; peak_voices = max(peak_voices, len(active))
    return accepted, {'requested': len(events), 'accepted': len(accepted), 'dropped': len(dropped),
                      'peak_scheduled_voices': peak_voices, 'global_cap': limit, 'drops': dropped,
                      'simulation_only': True, 'device_performance': 'NOT_RUN'}
