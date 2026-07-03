# Component Design

Per-module responsibilities, interfaces, and the reasoning behind each. Modules in `core/` are the
implemented orchestration layer; `services/` is the stubbed radio boundary; `gui/` is the shell.

## core/engagement.py — the authorization gate

The `Engagement` is the object that must exist before anything happens. It encodes scope, time
window, and rules of engagement, and answers one question for the orchestrator: **"is targeting
`bssid` permitted right now?"**

Key methods:
- `Engagement.load(path)` — load a signed engagement file the tester prepared.
- `is_target_in_scope(bssid) -> bool` — allow-list check.
- `is_within_window(now) -> bool` — time-box check.
- `deauth_permitted -> bool` — RoE flag.
- `affirm_authorization(typed_essid, auth_reference)` — the operator must re-type the in-scope
  ESSID and cite the written-authorization reference; returns an `Affirmation` the orchestrator
  requires to leave `IDLE`.

Design intent: the gate is data-driven and explicit. There is no way to "just target this AP I see"
— it has to be in the engagement first.

## core/orchestrator.py — the state machine

Owns the `AttackState` enum and the legal transition table. Every attack action is a transition
that:
1. checks the transition is legal from the current state,
2. re-checks the engagement gate,
3. calls the (stubbed) `RadioBackend`,
4. writes an evidence-log entry,
5. emits an event to the GUI.

The GUI enables exactly the buttons whose transitions are currently legal. This keeps "what can I
do now" in one place.

## core/evidence.py — append-only evidence log

Every meaningful action is recorded with a timestamp, the engagement id, the state transition, and
a hash chain (each entry includes the hash of the previous — tamper-evidence). This is what makes
the tool a professional instrument: the log is the backbone of the deliverable report and of
after-the-fact accountability. There is deliberately no "delete entry" API.

## core/vault.py — local secret store

Holds the one sensitive datum (a recovered passphrase) encrypted at rest, keyed by an
engagement-scoped key. Exposes `put(handle, secret)` / `reveal(handle)` / `purge()`. There is **no
`export` / `send` method** — the absence is the feature. `reveal()` is an explicit, logged
operator action.

## core/report.py — engagement report

Generates the tester's deliverable: engagement metadata, timeline (from the evidence log), what was
attempted, whether a handshake was captured, whether a passphrase was recovered (yes/no + handle,
not value), and remediation guidance pulled from [`DEFENSE.md`](DEFENSE.md). Output is Markdown/PDF.

## services/radio_backend.py — the RadioBackend interface (STUBBED)

The seam between the portable orchestration and the platform-specific, sensitive radio work. It is
an abstract interface; the shipped `StubRadioBackend` raises `NotImplementedError` with a note
naming the standard tool that performs each operation. This lets you:
- unit-test the whole orchestrator against a `FakeRadioBackend`,
- understand exactly what a real build would wire in,
- keep the repository non-operational.

Methods: `scan`, `capture_handshake`, `start_twin`, `deauth`, `validate_passphrase`, `stop`. See
the file for signatures and the tool mapping.

## gui/ — the Windows shell (MVVM)

Screens (see [`../design/ui-wireframes.md`](../design/ui-wireframes.md)):
1. **Dashboard** — engagement status, backend connection, big red STOP.
2. **Engagement** — load/create engagement, scope, window, RoE, the affirmation form.
3. **Recon** — live AP/client list from `scan()`; select an in-scope target.
4. **Attack console** — the state machine as a visual pipeline; step controls; live event feed.
5. **Report** — generate/preview/export; purge.

The GUI is a thin renderer over `core/`. In a production Windows build this is WPF/WinUI (C#) over
the same orchestration contracts; here it's described as a PySide6/Qt skeleton so the whole system
reads in one language.
