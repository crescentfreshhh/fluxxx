"""RadioBackend — the seam between portable orchestration and platform-specific 802.11 work.

This module ships ONLY the abstract interface and a StubRadioBackend that refuses to operate. The
concrete implementation belongs to whoever runs an *authorized* engagement and is wired to standard
tooling on a Linux monitor-mode backend (see docs/ARCHITECTURE.md §6 and docs/WINDOWS_PLATFORM.md).

Deliberately NOT included in this repository:
  * 802.11 deauthentication frame injection
  * a live captive-portal credential-capture web root
  * handshake-based passphrase cracking

Those are the operational, weaponizable parts. Keeping them out lets the orchestrator be fully
designed and unit-tested (against FakeRadioBackend) without this repo being a runnable attack.
"""
from __future__ import annotations

import abc
from dataclasses import dataclass


@dataclass
class AccessPoint:
    essid: str
    bssid: str
    channel: int
    signal_dbm: int
    encryption: str
    client_count: int


@dataclass
class ValidationResult:
    verified: bool
    handle: str | None = None  # vault handle if verified; never the plaintext


# Maps each backend operation to the standard tool a real, authorized build would wrap.
REAL_IMPLEMENTATION = {
    "scan": "airodump-ng on the monitor interface",
    "capture_handshake": "airodump-ng (+ at most one assisted reconnect)",
    "start_twin": "hostapd (AP) + dnsmasq (DHCP/DNS) + captive-portal web root",
    "deauth": "aireplay-ng — gated by engagement RoE; omitted operationally here",
    "validate_passphrase": "aircrack-ng / cowpatty against the captured .cap",
    "stop": "tear down hostapd/dnsmasq, restore managed mode on the adapter",
}


class RadioBackend(abc.ABC):
    """What the orchestrator calls. Concrete impls run on the Linux radio node."""

    @abc.abstractmethod
    def scan(self) -> list[AccessPoint]: ...

    @abc.abstractmethod
    def capture_handshake(self, bssid: str) -> bool:
        """Return True once a WPA handshake for bssid is captured."""

    @abc.abstractmethod
    def start_twin(self, essid: str, bssid: str, channel: int) -> None: ...

    @abc.abstractmethod
    def deauth(self, bssid: str, client: str | None = None) -> None:
        """RoE-gated client migration. Orchestrator only calls this if deauth_permitted."""

    @abc.abstractmethod
    def validate_passphrase(self, candidate: str) -> ValidationResult:
        """Check candidate against the captured handshake. Never returns the plaintext."""

    @abc.abstractmethod
    def stop(self) -> None:
        """Tear down the twin and restore the adapter. Must always succeed."""


class StubRadioBackend(RadioBackend):
    """The only backend shipped here. Every operation refuses and points at the real tool."""

    def _refuse(self, op: str):
        raise NotImplementedError(
            f"RadioBackend.{op} is intentionally not implemented in this design repo. "
            f"An authorized build wires it to: {REAL_IMPLEMENTATION[op]}. See docs/AUTHORIZATION.md."
        )

    def scan(self): self._refuse("scan")
    def capture_handshake(self, bssid): self._refuse("capture_handshake")
    def start_twin(self, essid, bssid, channel): self._refuse("start_twin")
    def deauth(self, bssid, client=None): self._refuse("deauth")
    def validate_passphrase(self, candidate): self._refuse("validate_passphrase")
    def stop(self): pass  # teardown is always safe to call, even when nothing is running
