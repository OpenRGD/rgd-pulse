"""Pure source-gate tests: no network, USB, sensor, file writes or motors."""
import unittest

from pulse_mux_imu_preflight import check_observation, MAX_AGE_NS


def example(now: int) -> dict:
    return {
        "schema_version": "console.imu.v0.1-candidate",
        "robot_id": "paky-01",
        "sensor_id": "torso-imu-01",
        "source_id": "paky01-wit-serial",
        "sensor_frame_id": "paky01_imu_sensor_unverified",
        "source_kind": "physical",
        "reference_frame": "imu_sensor_axes_unverified",
        "time_quality": "host_receive_only_not_chronograph",
        "heading_calibrated": False,
        "decision_eligible": False,
        "observation_only": True,
        "calibration_revision": None,
        "sequence": 100,
        "host_monotonic_ns": now - 100_000_000,
    }


class SourceGateTests(unittest.TestCase):
    NOW = 100_000_000_000

    def test_fresh_console_not_a_sensor_attestation(self):
        self.assertEqual(
            check_observation(example(self.NOW), self.NOW),
            ("FRESH_CONSOLE_OBSERVATION", 100, 100),
        )

    def test_null_and_malformed_are_not_fresh(self):
        for value in (None, [], "simulated", 42):
            self.assertNotEqual(check_observation(value, self.NOW)[0], "FRESH_CONSOLE_OBSERVATION")

    def test_cannot_promote_simulated_or_unsafe_claims(self):
        for key, value in (
            ("source_kind", "simulated"),
            ("observation_only", False),
            ("decision_eligible", True),
            ("heading_calibrated", True),
            ("calibration_revision", "fake-cal"),
            ("source_id", "other-sensor"),
            ("sensor_frame_id", "robot_world"),
            ("schema_version", "ri.consumer-observation-ref.v0.1-candidate"),
        ):
            obs = example(self.NOW)
            obs[key] = value
            self.assertEqual(check_observation(obs, self.NOW)[0], "REJECTED_CONSOLE_PROVENANCE")

    def test_unknown_and_future_host_clock_fail_closed(self):
        for ts in (0, -1, self.NOW + 1, "1234", True):
            obs = example(self.NOW)
            obs["host_monotonic_ns"] = ts
            self.assertEqual(check_observation(obs, self.NOW)[0], "INVALID_HOST_CLOCK")

    def test_stale_and_boundary(self):
        obs = example(self.NOW)
        obs["host_monotonic_ns"] = self.NOW - MAX_AGE_NS
        self.assertEqual(check_observation(obs, self.NOW)[0], "FRESH_CONSOLE_OBSERVATION")
        obs["host_monotonic_ns"] -= 1
        self.assertEqual(check_observation(obs, self.NOW)[0], "STALE_CONSOLE_OBSERVATION")

    def test_sequence_zero_bool_and_invalid_are_not_allowed(self):
        for seq in (0, -1, "1", True):
            obs = example(self.NOW)
            obs["sequence"] = seq
            self.assertEqual(check_observation(obs, self.NOW)[0], "INVALID_SEQUENCE")


if __name__ == "__main__":
    unittest.main()
