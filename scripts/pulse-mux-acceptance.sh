#!/usr/bin/env bash
# Reproducible operator-only PULSE-0.4B physical integration bench.
# Never flashes, opens WIT/CH340, starts publishers, touches CAN or motors.
# Runs one Pico USB client at a time; expects publisher to be managed separately.
set -euo pipefail

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
EXPECTED_BRANCH="feat/pulse-0.4b-adaptive-multiplex"
if [[ "$(git -C "$REPO" branch --show-current)" != "$EXPECTED_BRANCH" ]]; then
  echo "ABORT: run on $EXPECTED_BRANCH, not a frozen or deployed branch" >&2
  exit 1
fi

echo "=== PULSE MUX K3 PHYSICAL SOURCE PREFLIGHT (READ ONLY) ==="
python3 "$REPO/scripts/pulse_mux_imu_preflight.py"

echo "=== EXCLUSIVE PICO CDC IDENTIFICATION ==="
shopt -s nullglob
ports=(/dev/serial/by-id/usb-OpenRGD_RGD_Pulse_Pico_W_*-if00)
if [[ "${#ports[@]}" -ne 1 || ! -e "${ports[0]}" ]]; then
  echo "ABORT: exactly one verified OpenRGD Pico CDC by-id port required" >&2
  exit 1
fi
PICO="${ports[0]}"
if command -v fuser >/dev/null 2>&1 && fuser -s "$PICO" 2>/dev/null; then
  echo "ABORT: another program owns the Pico USB CDC, close it first" >&2
  exit 1
fi

OUT_DIR="$HOME/openrgd/reports"
mkdir -p "$OUT_DIR"
LOG="$OUT_DIR/pulse-mux-0.4b-$(date -u +%Y%m%dT%H%M%SZ).log"
if [[ -e "$LOG" ]]; then
  echo "ABORT: report path already exists" >&2
  exit 1
fi
printf 'REPORT=%s\n' "$LOG"
echo "Select K3 for >= 10s, then K4 for >= 5s, then K1/K2, then K3 again."
echo "Expect genuine K3 IMU #... and K4 HOST #... lines, not only WS connection."

(
  cd "$REPO"
  cargo run -p pulse-bridge -- mux-live "$PICO" 120
) 2>&1 | tee "$LOG"

echo "=== AUDIT PHYSICAL SAMPLES ==="
summary="$(grep -F 'PULSE MUX SUMMARY:' "$LOG" | tail -n 1 || true)"
if [[ -z "$summary" ]]; then
  echo "INCOMPLETE: no final mux summary; check physical link" >&2
  exit 1
fi
if [[ "$summary" =~ IMU[[:space:]]forwarded=([0-9]+) ]]; then
  imu_count="${BASH_REMATCH[1]}"
else
  echo "INCOMPLETE: IMU count missing in mux summary" >&2
  exit 1
fi
if [[ "$summary" =~ SYS[[:space:]]forwarded=([0-9]+) ]]; then
  sys_count="${BASH_REMATCH[1]}"
else
  echo "INCOMPLETE: SYS count missing in mux summary" >&2
  exit 1
fi
if (( imu_count == 0 || sys_count == 0 )); then
  echo "INCOMPLETE: need nonzero verified IMU and SYS forwarding" >&2
  echo "$summary" >&2
  exit 1
fi
for label in 'PICO VIEW = K3' 'PICO VIEW = K4' 'K3 IMU #' 'K4 HOST #'; do
  if ! grep -Fq "$label" "$LOG"; then
    echo "INCOMPLETE: missing $label; see $LOG" >&2
    exit 1
  fi
done
echo "MUX FUNCTIONAL PASS: IMU=$imu_count SYS=$sys_count; K3/K4 physical path observed."

echo "=== LEGACY PULSE/1 REGRESSION (MUX HAS CLOSED THE PORT) ==="
(
  cd "$REPO"
  cargo run -p pulse-bridge -- serial-demo "$PICO" 20
  cargo run -p pulse-bridge -- serial-demo "$PICO" 100
) 2>&1 | tee -a "$LOG"

if ! grep -q 'PULSE ACK SUMMARY: 20/20 confirmed, 0 unexpected frames' "$LOG"; then
  echo "INCOMPLETE: 20/20 ACK evidence missing" >&2
  exit 1
fi
if ! grep -q 'PULSE ACK SUMMARY: 100/100 confirmed, 0 unexpected frames' "$LOG"; then
  echo "INCOMPLETE: 100/100 ACK evidence missing" >&2
  exit 1
fi
echo "=== MUX K3/K4 + SYNTHETIC USB 20/100 PASS ==="
echo "Report: $LOG"
echo "Separate gates remain: publisher-stop STALE, cold-boot persistence and per-page CPU/RAM measurements."
