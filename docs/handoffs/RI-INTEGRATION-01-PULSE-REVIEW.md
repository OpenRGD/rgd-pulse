# RI-INTEGRATION-01 — Pulse/Chronoscope owner review

**Review date:** 2026-11-25  
**Status:** coordination-only; candidate/non-normative; **no runtime integration or physical deployment authorized**.  
**Pulse source:** OpenRGD/rgd-pulse; physically accepted 0.3C source frozen at \`release/pulse-0.3c-physical-20261027\`.  
**RI source:** [RI PR #4](https://github.com/OpenRGD/rgd-reality-ingress/pull/4), branch \`feat/ri-integration-01-pipeline-contract\`, read-only evidence-ref candidate.

This review adopts the [RI → Pulse handoff](https://github.com/OpenRGD/rgd-reality-ingress/blob/feat/ri-integration-01-pipeline-contract/docs/handoffs/RI-TO-PULSE-CHRONOSCOPE.md) as an **integration constraint**, not an instruction to rewrite existing Pulse, merge candidate schemas or flash hardware.

## 1. Verified ownership and feature inventory

| Component | Present source / owner | Evidence and limitations |
| --- | --- | --- |
| Physical WIT/CH340 IMU acquisition | Paky-01 \`scripts/imu_publish.py\` (one exclusive device owner) | Sends publisher observations to Console authenticated local POST; **Pulse and RI must not open the sensor serial** |
| Console IMU read-side | OpenRGD Console \`console.v1\` \`type=imu\`, \`console.imu.v0.1-candidate\` | Existing read-only loopback WS. Console latest GET freshness is ~3000ms; WS is loopback-restricted, **not user-authenticated** |
| K3 IMU | Pulse 0.3B/physically accepted 0.3C | Existing Console → Rust Pulse Bridge → \`PULSE/IMU-1\` USB; physical \`RAW/STALE/N/A\` observed. Pico stale threshold ~800ms, independently of Console TTL; sensor axes and yaw uncalibrated |
| 0.3C rollback | \`release/pulse-0.3c-physical-20261027\` | Accepted Pico W display colors, X=1/Y=2, joystick/skins, flash journal, CRC ACK 100/100; do not rewrite this source |
| K4 Linux diagnostics | [Pulse PR #12](https://github.com/OpenRGD/rgd-pulse/pull/12), Draft | \`PULSE/SYS-1\` source-backed host uptime/RAM + local Console \`/healthz\`. Real K4 display operator-confirmed. Motor/CAN/drive voltage = N/A |
| Adaptive K3/K4 | [Pulse PR #13](https://github.com/OpenRGD/rgd-pulse/pull/13), Draft stacked on #12 | One Pico CDC owner, page hints, selective host WS/HTTP. K4 host and page switching physically observed; **K3 mux forwarding not confirmed in supplied logs**. No merge until remaining source/USB regressions |
| RI evidence boundary | [RI PR #4](https://github.com/OpenRGD/rgd-reality-ingress/pull/4), Draft | Candidate local evidence-store projection and integrity verification. **No deployed RI→Pulse producer, transport or device access** evidenced |

**Do not make RI an inline dependency for the running K3 sensor stream.**
Console's accepted physical sensor observation and RI's evidence reference
are different data classes. RI's hash verification must not elevate a
Console sample, and conversely Console's K3 RAW must not assert that RI
stored or checked the sample.

## 2. Exact RI candidate contract and compatibility

RI PR #4 defines \`ri.consumer-observation-ref.v0.1-candidate\` at
\`contracts/consumer-observation-ref.v0.1-candidate.schema.json\`.
Its Python projection \`src/reality_ingress/consumer_ref.py\` has two modes:
\`unverified_ref_from_envelope\` (no store verification) and
\`verified_ref_from_store\` (store-wide local integrity check before lookup).
A locally verified digest **does not authenticate the physical sensor**.

The candidate includes \`observation_id\`, \`source_id\`, \`modality\`,
\`reference_frame_id\`, \`sample_sequence\`, \`payload_sha256_hex\`,
media type/size, origin and integrity statuses, and a time object.
It explicitly constrains all four authority flags
\`source_verified=false\`, \`robot_binding_verified=false\`,
\`history_committed=false\`, \`actuation_enabled=false\`.

| RI candidate claim | Pulse rendering/use allowed now | Forbidden promotion |
| --- | --- | --- |
| \`integrity_status=verified_local_hash\` | Label **LOCAL HASH VERIFIED** only after authorized local store verification | \`PHYSICAL VERIFIED\`, \`LIVE ROBOT\`, safe sensor pose, identity |
| \`integrity_status=not_verified\` | **UNVERIFIED REF**; no claim that media exists or is accessible | Present as a verified evidence item |
| \`origin_status=source_claimed_physical\` | **SOURCE CLAIM ONLY** | Map to verified physical data or K3 \`RAW\` |
| \`origin_status=source_declared_simulated\` | **SIMULATED** | Map to measured robot activity |
| \`origin_status=unknown\` | **UNKNOWN** | Default to physical/live |
| \`time.received_monotonic_ns\` + \`clock_domain_id\` + \`boot_id\` | Host receipt metadata only; calculate a receipt-age indicator **only if same clock and boot are established independently** | Sensor sample time, global time, Chronograf anchor |
| \`source_time_status=unknown\`, \`source_measurement_time=null\`, \`chronograf_anchor_ref=null\` | Mark **SOURCE TIME UNKNOWN / NO ANCHOR** | Create TimeAtom, AnchorRef64 or Chronon |
| \`history_committed=false\` | **NOT IN HISTORY** | K2 Chronon, canonical memory receipt |
| \`payload_sha256_hex\` and evidence metadata | Host-only validator/status after access policy; no payload/file path sent to Pico | Automatic payload/media access, hashing on Pico |

The candidate does **not** provide a signed source identity,
a verified robot binding, an authenticated live subscription,
dropped-frame counters, or end-to-end freshness. Gaps in
\`sample_sequence\` alone must not be relabelled as dropped media.
Those would require source-owner metrics and new explicit contracts.

## 3. Minimal adapter decision — DEFER IMPLEMENTATION

**No new adapter is required for compatibility today.** Pulse has no
current need to consume RI evidence to render K3/K4, and RI PR #4
is a source-only candidate without a verified read-side service
or granted access policy.

If a real RI diagnostic use case is later approved, the smallest change is:

1. **Producer remains RI**: an explicitly authorized, read-only host consumer
   reads only already-authorized RI \`ObservationRef\` metadata, validated
   against the exact candidate schema. It must not operate an EvidenceStore
   from the Pico, open cameras/audio/IMU, replay old buffers as live, or
   run expensive \`EvidenceStore.verify()\` in an IMU/USB critical loop.
2. **Bounded host projection**: map a *small allowlisted status* for
   optional diagnostics (reference present/unverified, local-hash
   verified, origin claim/simulated/unknown, source time unknown,
   history uncommitted); never forward media, filesystem paths,
   secrets, raw SHA, faces/audio or large arbitrary JSON to the RP2040.
3. **UI**: first consider a Console status view. K4 may expose
   a separate optional evidence detail only if it does not obscure
   Linux host status, CONSOLE/N/A, and K3 provenance. No RI-triggered
   change of joystick navigation, \`PULSE/1\`, \`PULSE/IMU-1\`, or flash
   preferences. A future host-side status projection should reuse the
   single Pulse CDC ownership architecture; **no new USB protocol is
   justified merely by the existence of RI**.
4. **Freshness and fail-closed**: missing/expired/unauthorized RI data
   becomes \`UNKNOWN\`/\`UNVERIFIED\`, not K3 \`RAW\` or hardware OFF;
   any clock comparison requires verified same-boot/domain assumptions.
   RI status **never gates or blocks** IMU streaming, rendering, VAD,
   Somatic, or Visual Feedback.
5. **Consumer boundary**: K2 Chronons can only display History/Chronograf
   receipts with independently verified authority, not RI
   \`observation_id\` or host receipt time.

No RI DTO, additional USB kind, new writer, or producer subscription is
introduced in this documentation PR. A future code candidate must live in a
**separate Draft PR** with a new explicit interface/ACL decision.

## 4. Ownership, compatibility and safety test matrix (future adapter)

| Gate | Expected outcome |
| --- | --- |
| RI ref with \`not_verified\` | Only UNVERIFIED; not physical, not \`LIVE\` |
| RI \`verified_local_hash\`, but \`source_verified=false\` | LOCAL HASH VERIFIED + SOURCE UNVERIFIED simultaneously |
| \`origin_status=source_declared_simulated\` | SIMULATED, cannot become physical |
| Fake \`history_committed=true\`, \`source_verified=true\`, non-null anchor, unknown fields, wrong version/hash | Reject/fail closed |
| Missing/ref withheld by ACL/retention or producer unavailable | No evidence claim, no fabricated zero/age |
| Clock domain or boot ID differs/unknown | Age UNKNOWN; no cross-host timing, no Chronograf assertion |
| Large/media/path/personal payload injected | Reject or project safe fixed-size status only; no RP2040 media transfer |
| K3 existing source stops; RI data remains available | K3 still goes \`RAW → STALE → N/A\` according to original Pulse/Console rules; RI cannot resurrect it |
| Legacy PULSE/1 and PULSE/IMU-1 frames, skins/flash journal and K4 host status | Identical function, performance and CRC regressions; no new reader |
| Physical optional integration test | Separate approved pilot, opt-in, bounded resource/latency measurement, rollback by disabling only new consumer |

For any future physical pilot, distinguish source benchmark from consumer
benchmark. The observed multiplex Pulse Bridge process around 0.3% CPU /
3.8 MiB RSS (single 2026-11-22 sample) is **not a budget guarantee**;
new parsing/verification overhead requires instrumented measurement.

## 5. Open questions for RI / owners

- Which **authorized read-only transport and ACL** would deliver ref
  metadata without granting media access or running \`EvidenceStore.verify\`
  on every live update? The PR #4 projection functions do not themselves
  provide a deployed Pulse subscription.
- Does the intended RI source have an independently verifiable **robot
  binding and source ID mapping** to Console's IMU session/source? The
  current candidate explicitly says **not verified**.
- Which separately owned service provides genuine **dropped observation**
  metrics, source receipt freshness and validated same-boot clock domain?
  None are implied by RI PR #4's fields.
- When, if ever, would a qualified **History/Chronograf receipt** be
  available for K2? Pulse will not create one.
- Keep PR #4, Pulse #12 and Pulse #13 separate and Draft until their
  independent gates pass. **No automatic merge, contract promotion or
  physical flash** from this coordination note.

## Handoff back to Reality Ingress

See [PULSE-TO-REALITY-INGRESS-RI-INTEGRATION-01.md](PULSE-TO-REALITY-INGRESS-RI-INTEGRATION-01.md) for a short return note with decisions, candidate contract, open gates and PR references.
