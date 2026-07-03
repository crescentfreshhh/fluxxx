# System Architecture

## 1. Layered view

Fluxxx is a three-layer system with a hard boundary between the control plane (Windows) and the
radio plane (Linux). The boundary is what makes the design portable and what keeps the sensitive
operations isolated and stubbable.

```
┌───────────────────────────────────────────────────────────────────────────┐
│  PRESENTATION LAYER  (Windows GUI)                                          │
│  - MVVM shell: Dashboard, Engagement setup, Recon, Attack console, Report   │
│  - Renders state; issues intents; never touches 802.11                      │
├───────────────────────────────────────────────────────────────────────────┤
│  ORCHESTRATION LAYER  (core/ — runs on Windows host or the node)            │
│  - Engagement + Authorization model (the gate)                             │
│  - Attack state machine / orchestrator                                     │
│  - Evidence log + report generator                                        │
│  - Secret vault (local, no exfil path)                                     │
├───────────────────────────────────────────────────────────────────────────┤
│  RADIO / SERVICE LAYER  (services/ — runs on Linux backend)                 │
│  - RadioBackend interface  ← STUBBED in this repo                          │
│    · scan()              (recon)                                          │
│    · capture_handshake() (passive/assisted)                              │
│    · start_twin()        (evil AP + captive portal)                      │
│    · deauth()            (client migration — gated, stubbed)             │
│    · validate_passphrase() (against captured handshake)                  │
│  - Concrete impls wrap aircrack-ng / hostapd / dnsmasq / hcxtools         │
└───────────────────────────────────────────────────────────────────────────┘
```

## 2. Control-channel design (GUI ⇄ backend)

When the backend is remote (WSL2, VM, or on-site node) the GUI talks to it over a **local control
channel**:

- **Transport**: gRPC over a loopback/LAN socket, or a WebSocket, with **mutual TLS**. The backend
  refuses connections without the engagement-scoped client cert. This prevents a stray process from
  driving the radio.
- **Messages are intents, not commands**: `StartScan`, `ArmTwin{engagement_id, target_bssid}`,
  `Stop`, `GetStatus`. The backend re-validates every intent against the engagement scope it holds
  — the GUI is not trusted to have done the check.
- **Events stream back**: discovered APs, handshake-captured, client-count, portal-hit,
  validation-result, errors. The GUI is a live renderer of this event stream.

Re-validation on the backend is deliberate: even if someone bypasses the GUI gate, the backend
still refuses to target a BSSID that isn't in the loaded engagement's allow-list and won't arm
outside the time window.

## 3. Attack state machine

The orchestrator is a strict state machine. Illegal transitions are rejected. This is both a
safety property and what makes the GUI's button-enablement trivial (buttons map to legal
transitions from the current state).

```
        ┌────────────┐
        │   IDLE     │  no engagement
        └─────┬──────┘
              │ load engagement + affirm authorization
              ▼
        ┌────────────┐
        │  ARMED     │  engagement in scope + in window; target selected
        └─────┬──────┘
              │ scan()
              ▼
        ┌────────────┐   capture_handshake()      ┌──────────────────┐
        │  RECON     │ ─────────────────────────▶ │ HANDSHAKE_CAPTURED│
        └─────┬──────┘                            └────────┬─────────┘
              │                                            │ start_twin()
              │◀── (no handshake yet: portal can run but   ▼
              │     cannot validate — design refuses to    ┌──────────────┐
              │     "capture" secrets it can't verify)     │  TWIN_ACTIVE │
              │                                            └──────┬───────┘
              │                                                   │ deauth() [gated by RoE]
              │                                                   ▼
              │                                            ┌──────────────┐
              │                                            │  MIGRATING   │  portal live
              │                                            └──────┬───────┘
              │                                                   │ passphrase entered
              │                                                   ▼
              │                                            ┌──────────────┐
              │                                 reject ◀── │  VALIDATING  │ ──▶ verified
              │                                  (loop)    └──────┬───────┘
              │                                                   ▼
              │                                            ┌──────────────┐
              └───────────────── stop() ◀───────────────── │  COMPLETE    │ ──▶ report + purge
                                                           └──────────────┘
```

Notable design decisions encoded in the machine:

- **You cannot enter `VALIDATING` without a captured handshake.** The tool refuses to solicit a
  secret it has no way to verify — this both makes it a real assessment tool (a wrong passphrase is
  rejected, so the result is trustworthy) and removes the "blind phishing" mode.
- **`deauth()` is a gated edge**, not automatic. It only fires if the engagement's rules of
  engagement permit client deauth, and each burst is logged.
- **`stop()` is reachable from every state** and always tears down the twin, restores the adapter,
  and can trigger the purge.

## 4. Data flow for the one meaningful secret

The only genuinely sensitive datum is a passphrase a user enters into the portal. Its lifecycle:

```
portal form ──▶ backend validate_passphrase(candidate, handshake)
                        │
              ┌─────────┴──────────┐
         verified                rejected
              │                     │
              ▼                     ▼
   store handle in vault      portal re-prompts
   (value encrypted at rest)  (value discarded)
              │
              ▼
   report references it by HANDLE, never prints the value
              │
              ▼
   purge on engagement close (secure delete)
```

There is intentionally **no network sink** for this value. The vault is local; the report cites
"passphrase recovered: yes/no" plus a handle the tester can reveal manually if the engagement
requires disclosing it to the client.

## 5. Component map

See [`COMPONENTS.md`](COMPONENTS.md) for per-module detail. Summary:

| Module | Layer | Responsibility | Ships in repo? |
|---|---|---|---|
| `gui/` | Presentation | MVVM shell, screens, intent dispatch | Skeleton + wireframes |
| `core/engagement.py` | Orchestration | Engagement + authorization gate | Implemented |
| `core/orchestrator.py` | Orchestration | State machine | Implemented (drives stubbed backend) |
| `core/evidence.py` | Orchestration | Append-only evidence log | Implemented |
| `core/vault.py` | Orchestration | Local encrypted secret store | Interface + safe stub |
| `core/report.py` | Orchestration | Engagement report generator | Implemented |
| `services/radio_backend.py` | Radio | The `RadioBackend` interface | Interface + **stub only** |

## 6. Why the sensitive parts are stubbed (and what fills them)

The `RadioBackend` stub raises `NotImplementedError` with a pointer to the standard tool that
performs each function. A real, authorized build wires them to:

| Backend method | Real implementation |
|---|---|
| `scan()` | `airodump-ng` on the monitor interface |
| `capture_handshake()` | `airodump-ng` + optional single assisted reconnect |
| `start_twin()` | `hostapd` (AP) + `dnsmasq` (DHCP/DNS) + a captive-portal web root |
| `deauth()` | `aireplay-ng` — the gated, deliberately-omitted-operationally part |
| `validate_passphrase()` | `aircrack-ng`/`cowpatty` against the captured `.cap` |

Keeping these as an interface means the design is complete and testable (you can unit-test the
orchestrator against a fake backend) without the repository being a runnable attack.
