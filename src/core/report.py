"""Engagement report generator.

Turns the tamper-evident evidence log into the tester's deliverable. Recovered passphrases are
cited by handle, never by value. Remediation is sourced from docs/DEFENSE.md.
"""
from __future__ import annotations

from .engagement import Engagement
from .evidence import EvidenceLog

REMEDIATION = [
    "Deploy WPA3-SAE (or WPA2/WPA3 transition mode as an interim step).",
    "Set 802.11w Protected Management Frames (PMF) to *required* — defeats forged deauth.",
    "For enterprise WLANs, use 802.1X with enforced server-certificate validation.",
    "Enable WIPS/WIDS with rogue-AP and deauth-flood detection wired to the SOC.",
    "Maintain a BSSID allow-list of legitimate APs.",
    "Train users: a legitimate Wi-Fi network never asks for its password via a web page.",
]


def generate(engagement: Engagement, log: EvidenceLog,
             handshake_captured: bool, passphrase_handle: str | None) -> str:
    lines = [
        f"# Wireless Assessment Report — {engagement.name}",
        "",
        "## Scope",
        f"- ESSID(s): {', '.join(engagement.in_scope_essids)}",
        f"- BSSID(s): {', '.join(engagement.in_scope_bssids)}",
        f"- Window: {engagement.window_start.isoformat()} – {engagement.window_end.isoformat()}",
        f"- Deauth permitted: {engagement.deauth_permitted}",
        "",
        "## Result",
        f"- WPA handshake captured: {'yes' if handshake_captured else 'no'}",
        f"- Passphrase recovered: {'yes' if passphrase_handle else 'no'}"
        + (f" (handle: {passphrase_handle}; reveal manually if disclosure is in scope)"
           if passphrase_handle else ""),
        f"- Evidence chain integrity: {'VALID' if log.verify_chain() else 'BROKEN'}",
        "",
        "## Timeline (from evidence log)",
    ]
    for e in log.entries:
        lines.append(f"- `{e.at}`  **{e.action}**  {e.detail}")
    lines += ["", "## Remediation"]
    lines += [f"{i+1}. {r}" for i, r in enumerate(REMEDIATION)]
    lines += ["", "_Passphrase values are stored only in the local vault and are omitted here._"]
    return "\n".join(lines)
