# Engagement Lifecycle State Machine

The orchestrator is a strict finite state machine. This document is the authoritative transition
table that [`src/core/orchestrator.py`](../src/core/orchestrator.py) implements.

## States

| State | Meaning | Twin broadcasting? |
|---|---|---|
| `IDLE` | No engagement loaded / not authorized | no |
| `ARMED` | Engagement loaded, authorized, in-scope target selected, in time window | no |
| `RECON` | Actively scanning for target + clients | no |
| `HANDSHAKE_CAPTURED` | A WPA handshake for the target is captured (enables validation) | no |
| `TWIN_ACTIVE` | Evil twin + captive portal broadcasting | yes |
| `MIGRATING` | Client-migration in progress (deauth, RoE-gated); portal live | yes |
| `VALIDATING` | A candidate passphrase is being checked against the handshake | yes |
| `COMPLETE` | Result reached; ready for report + purge | no |
| `ERROR` | Fault; twin torn down, adapter restored | no |

## Transition table

| From | Event | Guard | To |
|---|---|---|---|
| `IDLE` | `load_engagement` | valid engagement file | `IDLE` |
| `IDLE` | `affirm_and_arm` | authorization affirmed **and** target in scope **and** in window | `ARMED` |
| `ARMED` | `scan` | — | `RECON` |
| `RECON` | `capture_handshake` | target selected | `HANDSHAKE_CAPTURED` |
| `RECON` | `scan` | — | `RECON` (refresh) |
| `HANDSHAKE_CAPTURED` | `start_twin` | in window | `TWIN_ACTIVE` |
| `TWIN_ACTIVE` | `deauth` | **`engagement.deauth_permitted`** | `MIGRATING` |
| `TWIN_ACTIVE` | `passphrase_submitted` | handshake present | `VALIDATING` |
| `MIGRATING` | `passphrase_submitted` | handshake present | `VALIDATING` |
| `VALIDATING` | `validation_rejected` | — | `MIGRATING` (re-prompt) |
| `VALIDATING` | `validation_verified` | — | `COMPLETE` |
| *any* | `stop` | — | `COMPLETE` (twin torn down) |
| *any* | `fault` | — | `ERROR` (twin torn down) |
| `COMPLETE` | `generate_report` | — | `COMPLETE` |
| `COMPLETE` | `purge` | report finalized | `IDLE` |
| `ERROR` | `reset` | — | `IDLE` |

## Invariants (asserted at every transition)

1. **No `VALIDATING` without `HANDSHAKE_CAPTURED` having occurred** — the tool never solicits a
   secret it cannot verify.
2. **No `MIGRATING` unless `engagement.deauth_permitted`** — deauth is opt-in per RoE.
3. **`TWIN_ACTIVE`/`MIGRATING` require `engagement.is_within_window(now)`** — leaving the window
   forces `stop`.
4. **Every transition writes one evidence-log entry** — no silent actions.
5. **`stop` is always legal and always tears down the twin and restores the adapter.**
