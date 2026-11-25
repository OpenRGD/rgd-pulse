#!/usr/bin/env python3
"""Read-only K3 source readiness gate for Pulse mux: Console GET only.

No USB/CH340/IMU opens, no publisher start/stop, no Console POST and no
sensor privilege. A claimed physical observation is not hardware attestation.
"""
from __future__ import annotations

import json
from pathlib import Path
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import build_opener, HTTPRedirectHandler, ProxyHandler, Request

URL = "http://127.0.0.1:8088/api/v1/robots/paky-01/imu"
MAX_BYTES = 16 * 1024
MAX_AGE_NS = 3_000_000_000


class NoRedirect(HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


def check_observation(data: object, now_ns: int) -> tuple[str, int | None, int | None]:
    """Return status, sequence and host-age milliseconds. Never trust claim as attestation."""
    if data is None:
        return ("NO_FRESH_SOURCE", None, None)
    if not isinstance(data, dict):
        return ("INVALID_SOURCE_SHAPE", None, None)
    expected = {
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
    }
    for key, value in expected.items():
        if key not in data or type(data[key]) is not type(value) or data[key] != value:
            return ("REJECTED_CONSOLE_PROVENANCE", None, None)
    seq = data.get("sequence")
    timestamp = data.get("host_monotonic_ns")
    if type(seq) is not int or seq <= 0:
        return ("INVALID_SEQUENCE", None, None)
    if type(timestamp) is not int or timestamp <= 0 or timestamp > now_ns:
        return ("INVALID_HOST_CLOCK", None, None)
    age = now_ns - timestamp
    if age > MAX_AGE_NS:
        return ("STALE_CONSOLE_OBSERVATION", seq, age // 1_000_000)
    return ("FRESH_CONSOLE_OBSERVATION", seq, age // 1_000_000)


def probe() -> int:
    if Path("/.dockerenv").exists() or Path("/run/.containerenv").exists():
        print("ABORT: run on the actual Ubuntu host, not inside Docker.")
        return 2
    req = Request(URL, method="GET", headers={"Accept": "application/json"})
    opener = build_opener(ProxyHandler({}), NoRedirect())
    try:
        with opener.open(req, timeout=3) as response:
            if response.status != 200:
                print("CONSOLE_NOT_OK")
                return 2
            payload = response.read(MAX_BYTES + 1)
    except (HTTPError, URLError, TimeoutError, OSError):
        print("CONSOLE_UNAVAILABLE_OR_REDIRECTED")
        return 2
    if len(payload) > MAX_BYTES:
        print("CONSOLE_RESPONSE_TOO_LARGE")
        return 2
    try:
        data = json.loads(payload)
    except (ValueError, UnicodeError):
        print("INVALID_CONSOLE_JSON")
        return 2
    status, seq, age_ms = check_observation(data, time.monotonic_ns())
    print(f"IMU_CONSOLE_STATUS={status}")
    if seq is not None:
        print(f"sequence={seq} host_observation_age_ms={age_ms}")
    if status != "FRESH_CONSOLE_OBSERVATION":
        print("Check the sole Paky-01 IMU publisher before mux-live; do not start a duplicate serial reader.")
        return 2
    print("K3 SOURCE READY: Console has a fresh claimed physical sensor observation.")
    print("This does NOT attest robot pose, publisher exclusivity or motor safety.")
    return 0


if __name__ == "__main__":
    sys.exit(probe())
