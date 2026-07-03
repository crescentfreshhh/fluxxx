# GUI Shell (skeleton)

The Windows GUI is a **thin MVVM renderer** over `core/`. It dispatches intents to an
`Orchestrator` and renders its state + evidence-log event stream. It never touches 802.11.

## Production choice

In a production Windows build this layer is **WPF or WinUI 3 (C#/.NET)** — see
[`../../docs/WINDOWS_PLATFORM.md`](../../docs/WINDOWS_PLATFORM.md). The views map 1:1 to the
wireframes in [`../../design/ui-wireframes.md`](../../design/ui-wireframes.md):

| View | ViewModel binds to | Intents dispatched |
|---|---|---|
| DashboardView | engagement status, backend link, state | `stop` |
| EngagementView | Engagement fields | `load_engagement`, `affirm_and_arm` |
| ReconView | `scan()` results (in-scope selectable) | `scan`, select target, `capture_handshake` |
| AttackConsoleView | state machine + event feed | `start_twin`, `deauth`, `submit_passphrase`, `stop` |
| ReportView | report preview | `generate_report`, `reveal`, `purge` |

## Button enablement rule

A control is enabled iff its intent is a **legal transition from the current `AttackState`** (see
[`../../design/state-machine.md`](../../design/state-machine.md)). This keeps "what can I do now"
in exactly one place — the state machine — instead of scattered across the UI.

## Skeleton

For a single-language reference, a PySide6/Qt version of the shell would host each view as a
`QWidget`, subscribe to orchestrator events on a background thread, and marshal updates to the UI
thread. The C# WPF version follows the same contracts. Either way, the shell depends only on the
public methods of `core/orchestrator.Orchestrator` — swapping GUI frameworks touches nothing below
this folder.
