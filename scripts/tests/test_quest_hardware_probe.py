"""Offline contract tests. No ADB subprocess or physical device is used."""
import copy
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

SPEC = importlib.util.spec_from_file_location('quest_probe', Path(__file__).parents[1] / 'quest-hardware-probe.py')
probe = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(probe)


def fixture(variant='candidate', frames=300):
    expected = dict(frames=frames, warmup=60, size=1728, period_ms=16.667)
    samples = [dict(gpu=dict(sampleFrame=frame, supported=True, available=True, error=None,
                            ageFrames=10, skippedSamples=0, passesMs={'scene.main': 2., 'final.postprocess.overlays': 8.},
                            sectionsMs={'detail.neon': 999.})) for frame in range(90, 61 + frames, 30)]
    config = dict(frames=frames, warmupFrames=60, totalRenderedFrames=frames + 60, width=1728, height=1728,
                  periodMs=16.667, eyesPerFrame=2, ssrEnabled=True, glowOff=False, frozenSimulation=True,
                  fontGlowAtlas=variant == 'candidate', format='Rgba8UnormSrgb', depthStore='Discard',
                  effectParams=dict(probe.EFFECTS), renderer=dict(fontGlowAtlas={'drawnInstances': 5} if variant == 'candidate' else None))
    return dict(executionOS='android', physicalQuest=True, deviceLabel='Quest_3', openxrRuntime=False,
                fpsMeasurement=False, adapter=dict(backend='Vulkan', name='Adreno (TM) 740', requestedFeatures='TIMESTAMP_QUERY'),
                config=config, samples=samples, finalGpu=dict(error=None)), expected


class ProbeTests(unittest.TestCase):
    def test_half_glow_cut_age_is_bound_to_actual_arguments(self):
        data, expected = fixture('candidate')
        expected.update(experiment='half-glow', view='cut', cut_age=0.4)
        data['config'].update(glowResolution=0.5, view='cut', arguments=['--look-at-cut', '--cut-age', '0.4'])
        data['config']['renderer']['halfGlow'] = dict(requestedScale=0.5, activeScale=0.5, draws=2, width=864, height=864)
        probe.validate_benchmark(data, 'candidate', expected)
        data['config']['arguments'][-1] = '0.08'
        with self.assertRaises(ValueError):
            probe.validate_benchmark(data, 'candidate', expected)

    def test_half_glow_requires_actual_half_target_and_keeps_font_atlas(self):
        for variant in ('baseline', 'candidate'):
            data, expected = fixture('candidate')
            expected.update(experiment='half-glow', view='player')
            scale = 0.5 if variant == 'candidate' else 1.0
            data['config'].update(glowResolution=scale, view='player')
            data['config']['renderer']['halfGlow'] = dict(requestedScale=scale, activeScale=scale,
                                                         draws=4, width=864, height=864)
            probe.validate_benchmark(data, variant, expected)
            # A fruit close-up can legitimately have no visible numeric glyphs.
            data['config']['renderer']['fontGlowAtlas']['drawnInstances'] = 0
            probe.validate_benchmark(data, variant, expected)
            if variant == 'candidate':
                for key, value in [('activeScale', 1.0), ('draws', 0), ('width', 1728)]:
                    broken = copy.deepcopy(data)
                    broken['config']['renderer']['halfGlow'][key] = value
                    with self.subTest(key=key), self.assertRaises(ValueError):
                        probe.validate_benchmark(broken, variant, expected)
                data['config']['fontGlowAtlas'] = False
                with self.assertRaises(ValueError):
                    probe.validate_benchmark(data, variant, expected)

    def test_pass_totals_exclude_nested_sections(self):
        data, expected = fixture()
        result = probe.validate_benchmark(data, 'candidate', expected)
        self.assertEqual(result['wholePass']['medianMs'], 10.)
        self.assertEqual(result['wholePass']['n'], 10)

    def test_baseline_contract_and_float32_effects(self):
        data, expected = fixture('baseline')
        data['config']['effectParams']['ssrStrength'] = 0.9200000166893005
        probe.validate_benchmark(data, 'baseline', expected)

    def test_reject_wrong_device_backend_settings_and_inactive_atlas(self):
        cases = [('executionOS', 'linux'), ('physicalQuest', False), ('deviceLabel', 'Quest_3S')]
        for key, value in cases:
            data, expected = fixture()
            data[key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                probe.validate_benchmark(data, 'candidate', expected)
        for section, key, value in [('adapter', 'backend', 'Gl'), ('adapter', 'name', 'Apple'),
                                    ('config', 'frozenSimulation', False), ('config', 'fontGlowAtlas', False),
                                    ('config', 'width', 640), ('config', 'effectParams', {})]:
            data, expected = fixture()
            data[section][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                probe.validate_benchmark(data, 'candidate', expected)
        data, expected = fixture()
        data['config']['renderer']['fontGlowAtlas']['drawnInstances'] = 0
        with self.assertRaises(ValueError):
            probe.validate_benchmark(data, 'candidate', expected)

    def test_reject_duplicate_nested_invalid_or_insufficient_timestamps(self):
        for mutation in ('duplicate', 'nested', 'nan', 'missing', 'error', 'warmup'):
            data, expected = fixture()
            if mutation == 'duplicate':
                data['samples'][1] = copy.deepcopy(data['samples'][0])
            elif mutation == 'nested':
                data['samples'][0]['gpu']['passesMs']['detail.neon'] = 1
            elif mutation == 'nan':
                data['samples'][0]['gpu']['passesMs']['scene.main'] = float('nan')
            elif mutation == 'missing':
                data['samples'] = data['samples'][:4]
            elif mutation == 'error':
                data['samples'][0]['gpu']['error'] = 'readback failed'
            else:
                data['samples'][0]['gpu']['sampleFrame'] = 60
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                probe.validate_benchmark(data, 'candidate', expected)

    def test_smoke_can_have_one_sample_and_ab_ba_ab(self):
        data, expected = fixture(frames=30)
        self.assertEqual(probe.validate_benchmark(data, 'candidate', expected)['wholePass']['n'], 1)
        self.assertEqual(probe.paired_order(3), [('baseline', 'candidate'), ('candidate', 'baseline'), ('baseline', 'candidate')])

    def test_elf_architecture_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'probe'
            elf = bytearray(64)
            elf[:6] = b'\x7fELF\x02\x01'
            struct.pack_into('<HH', elf, 16, 3, 183)
            path.write_bytes(elf)
            probe.validate_elf(path)
            struct.pack_into('<H', elf, 18, 62)
            path.write_bytes(elf)
            with self.assertRaises(ValueError):
                probe.validate_elf(path)

    def test_owned_pid_and_exact_argv0_safety(self):
        executable = '/data/local/tmp/neon-hw-probe-unique/neon-preview'
        self.assertEqual(probe.parse_probe_pid('1234\n'), 1234)
        for invalid in ('', '0', '-12', '12;kill 1', '12 13', '1\n2'):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                probe.parse_probe_pid(invalid)
        self.assertTrue(probe.owns_probe_cmdline(1234, executable + '\0--size\01728\0', executable))
        for other in ('/system/bin/sh\0' + executable, executable + '-other\0', '/other/neon-preview\0', executable):
            self.assertFalse(probe.owns_probe_cmdline(1234, other, executable))
        signal = probe.guarded_signal(1234, executable, 'TERM')
        self.assertIn('/proc/1234/cmdline', signal)
        self.assertIn('kill -TERM 1234', signal)
        self.assertIn(executable, signal)
        with self.assertRaises(ValueError):
            probe.guarded_signal('123;kill 1', executable, 'TERM')
        wrapper = probe.probe_wrapper([executable, '--size', '1728'], '/assets', '/owned/pid')
        self.assertIn('exec env NEON_PHYSICAL_DEVICE=Quest_3', wrapper)
        self.assertIn('"$$"', wrapper)

    def test_remote_cleanup_signals_only_exact_owned_executable(self):
        executable = '/data/local/tmp/neon-hw-probe-unique/neon-preview'
        class FakeAdb:
            def __init__(self, cmdlines):
                self.cmdlines = iter(cmdlines)
                self.calls = []
            def text(self, *args, **kwargs):
                return '1234'
            def shell(self, command, **kwargs):
                self.calls.append(command)
                if command.startswith('cat /proc/'):
                    value = next(self.cmdlines)
                    return dict(returncode=1 if value is None else 0, stdout=value or '')
                return dict(returncode=0, stdout='')
        owned = FakeAdb([executable + '\0--size\0', None])
        with patch.object(probe.time, 'sleep'):
            result = probe.terminate_owned_probe(owned, '/unique/owned.pid', executable)
        self.assertTrue(result['success'])
        self.assertEqual([s['signal'] for s in result['signals']], ['TERM'])
        other = FakeAdb(['/system/bin/app_process64\0'] * 20)
        with patch.object(probe.time, 'sleep'):
            result = probe.terminate_owned_probe(other, '/unique/owned.pid', executable)
        self.assertFalse(result['success'])
        self.assertFalse(any('kill -' in command for command in other.calls))

    def test_explicit_sdk_adb_executable(self):
        adb = probe.Adb(5038, 'explicit', Path('/unused'), '/sdk/platform-tools/adb')
        self.assertEqual(adb.prefix, ['/sdk/platform-tools/adb', '-P', '5038', '-s', 'explicit'])

    def test_safe_cli_bounds(self):
        argv = ['--adb-port', '5037', '--serial', 'explicit-device', '--binary', 'binary', '--assets', 'assets', '--out', 'out']
        args = probe.parse_args(argv)
        self.assertEqual((args.frames, args.pairs, args.size), (300, 3, 1728))


if __name__ == '__main__':
    unittest.main()
