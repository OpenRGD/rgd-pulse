# HANDOFF DI RITORNO — Pulse / Chronoscope → Reality Ingress

**Data:** 2026-11-25  
**Destinatario:** [Reality Ingress PR #4](https://github.com/OpenRGD/rgd-reality-ingress/pull/4)  
**Risposta a:** [RI → Pulse](https://github.com/OpenRGD/rgd-reality-ingress/blob/feat/ri-integration-01-pipeline-contract/docs/handoffs/RI-TO-PULSE-CHRONOSCOPE.md)  
**Stato:** review del contratto candidato; nessuna integrazione runtime, nessun merge o flash.

## Decisioni concordate dal lato Pulse

1. **K3 invariato:** IMU fisica → publisher esclusivo Paky-01 → Console (`console.v1/type=imu`, `console.imu.v0.1-candidate`) → `pulse-bridge` → `PULSE/IMU-1` → Chronoscope. Non interporre RI, non duplicare CH340 o stream. K3 `RAW/STALE/N/A` mantiene la propria semantica (~800ms Pico; ~3000ms latest Console).
2. **K4 indipendente:** `PULSE/SYS-1` porta già uptime/RAM del Linux host e health HTTP della Console. PR #12/#13 sono Draft; il multiplex K3/K4 è stato osservato fisicamente per K4, ma nei log disponibili manca la conferma dell'inoltro IMU su K3. RI può offrire in futuro **solo una diagnostica opzionale di evidenza/provenienza**, senza sostituire i dati SYSTEM.
3. **K2 non è History:** nessun `ObservationRef` RI equivale a Chronon, commit History, timestamp sensore verificato o AnchorRef64 di Chronograf. La UI non creerà né certificherà queste autorità.
4. **Preservare tutto il collaudato:** PULSE/1, PULSE/IMU-1, CRC/HELLO/ProbeAck, un solo owner Pico CDC, skin/flash e baseline `release/pulse-0.3c-physical-20261027`. Nessun comando motori, CAN o gestione privilegi hardware.

## Contratto candidato accettato *per studio*, non per implementazione

`ri.consumer-observation-ref.v0.1-candidate` (RI PR #4), read-only:
`observation_id`, `source_id`, `modality`, `reference_frame_id`, `sample_sequence`,
`payload_sha256_hex`, `integrity_status`, `origin_status` e tempo con clock domain/boot.

La presenza di `verified_local_hash` attesta **integrità locale**, non sensore fisico. Il contratto impone
`source_verified=false`, `robot_binding_verified=false`,
`history_committed=false`, `actuation_enabled=false`. La misura temporale della sorgente e il riferimento Chronograf rimangono null/unknown; nessun calcolo automatico di freshness fra boot/clock diversi.

Gli stati visuali futuri restano distinti: `UNVERIFIED REF`, `LOCAL HASH VERIFIED`,
`SOURCE CLAIM/UNKNOWN`, `SIMULATED`, `STALE/UNKNOWN`, `NOT IN HISTORY`. Non riassumerli in `LIVE`.

## Adapter minimo e test

**Decisione:** nessun nuovo codice necessario ora. Dopo policy di lettura/ACL e sorgente autorizzata, eventuale **adapter host-only** può proiettare pochi bit di stato già validati da RI verso un dettaglio opzionale Console/K4. No media, file path, token, SHA integrali, nuova seriale, raccolta audio/video, scritture EvidenceStore, latenza sul loop IMU, né nuovi frame USB motivati solo dalla presenza di RI.

Test obbligatori prima di una PR software separata: schema/flag falsificati → rifiuto; hash `not_verified` ≠ integrità verificata; ref `source_declared_simulated` ≠ misura fisica; cambio boot/clock → età sconosciuta; no fonte/ACL → `UNKNOWN`; regressione fisica K3 publisher-stop → `STALE`, 100/100 synthetic ACK e budget CPU/RAM. L'integrazione futura dovrà essere disattivabile senza alterare Console, firmware o publisher.

## Punti aperti da RI

- Trasporto/ACL read-only effettivamente disponibili per ref autorizzati, senza accesso automatico ai payload.
- Verifica separata del binding robot/source e delle metriche di frame persi (non garantite dal candidato).
- Eventuali ricevute indipendenti History/Chronograf per K2.
- Validazione del pilot e della compatibilità dopo le PR Pulse 0.4A/0.4B.

**Riferimenti:** [Pulse PR #12](https://github.com/OpenRGD/rgd-pulse/pull/12) (K4 Linux);
[Pulse PR #13](https://github.com/OpenRGD/rgd-pulse/pull/13) (multiplex K3/K4);
[RI PR #4](https://github.com/OpenRGD/rgd-reality-ingress/pull/4);
[Pulse 0.3C release](https://github.com/OpenRGD/rgd-pulse/blob/main/docs/RELEASE-PULSE-0.3C.md).

**Nessuna promozione a standard canonico, merge automatico o autorizzazione di attuazione.**
