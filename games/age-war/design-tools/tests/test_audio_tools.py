"""Synthetic cases only. These tests do not contain delivered audio measurements."""
import importlib.util
import math
from pathlib import Path
import unittest


def load(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).resolve().parents[1] / 'core' / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


gain = load('audio_gain')
scheduler = load('audio_scheduler')


class GainTests(unittest.TestCase):
    def test_quiet_synthetic_measurements_gain_headroom(self):
        self.assertAlmostEqual(gain.preview_gain_db(-32, -10), 7.4)

    def test_high_crest_factor_preserves_headroom(self):
        self.assertAlmostEqual(gain.preview_gain_db(-40, -3), .9)

    def test_loud_signal_is_attenuated(self):
        self.assertLess(gain.preview_gain_db(-18, -.2), 0)

    def test_nonfinite_inputs_are_rejected(self):
        for value in (math.nan, math.inf, -math.inf):
            with self.assertRaises(ValueError):
                gain.preview_gain_db(value, -10)


class SchedulerTests(unittest.TestCase):
    def asset(self, key, priority=30, group='synthetic', duration=2000):
        return {'id': key, 'priority': priority, 'group': group, 'duration_ms': duration}

    def test_synthetic_crowd_respects_global_cap(self):
        index = {f'tone{i}': self.asset(f'tone{i}') for i in range(100)}
        events = [{'presentation_ms': 0, 'audio_id': key, 'entity_id': str(i)} for i, key in enumerate(index)]
        accepted, info = scheduler.schedule(events, index)
        self.assertEqual(len(accepted), 16)
        self.assertLessEqual(info['peak_scheduled_voices'], 24)
        self.assertTrue(info['simulation_only'])
        self.assertEqual(info['device_performance'], 'NOT_RUN')

    def test_priority_preempts_a_lower_world_voice(self):
        index = {'quiet': self.asset('quiet'), 'alert': self.asset('alert', 95)}
        events = [{'presentation_ms': 0, 'audio_id': 'quiet', 'entity_id': 'a'},
                  {'presentation_ms': 100, 'audio_id': 'alert', 'entity_id': 'b'}]
        accepted, info = scheduler.schedule(events, index, limit=1)
        self.assertEqual(len(accepted), 2)
        self.assertEqual(accepted[0]['stop_ms'], 100)
        self.assertEqual(info['peak_scheduled_voices'], 1)

    def test_family_cooldown_and_per_emitter_cap(self):
        index = {f'tone{i}': self.asset(f'tone{i}') for i in range(3)}
        events = [{'presentation_ms': i, 'audio_id': key, 'entity_id': 'a'} for i, key in enumerate(index)]
        accepted, info = scheduler.schedule(events, index)
        self.assertEqual(len(accepted), 2)
        self.assertEqual(info['drops'][0]['reason'], 'per_emitter_cap2')

    def test_speed_reschedules_starts_without_pitch_fields(self):
        index = {'tone': self.asset('tone')}
        event = {'presentation_ms': 300, 'audio_id': 'tone', 'entity_id': 'a'}
        accepted, _ = scheduler.schedule([event], index, speed=2)
        self.assertEqual(accepted[0]['start'], 150)
        self.assertNotIn('pitch', accepted[0])


if __name__ == '__main__':
    unittest.main()
