"""Engagement + authorization model — the gate that must pass before any radio op.

This is the safety core of Fluxxx. No attack action is permitted unless a loaded Engagement
authorizes targeting a specific BSSID within a specific time window, and the operator has affirmed
written authorization. See docs/AUTHORIZATION.md.
"""
from __future__ import annotations

import json
import time
from dataclasses import dataclass, field
from datetime import datetime, timezone


def _now() -> datetime:
    return datetime.now(timezone.utc)


@dataclass(frozen=True)
class Affirmation:
    """Proof the operator re-typed the in-scope ESSID and cited written authorization."""

    typed_essid: str
    auth_reference: str
    affirmed_at: datetime


@dataclass
class Engagement:
    """A scoped, time-boxed authorization to test specific wireless network(s).

    An Engagement is loaded from a file the tester prepares from their statement of work. It is an
    allow-list: only BSSIDs explicitly listed here can ever be targeted.
    """

    name: str
    in_scope_essids: list[str]
    in_scope_bssids: list[str]
    window_start: datetime
    window_end: datetime
    deauth_permitted: bool
    auth_reference_required: bool = True
    _affirmation: Affirmation | None = field(default=None, repr=False)

    # ---- loading -------------------------------------------------------------------------

    @classmethod
    def load(cls, path: str) -> "Engagement":
        with open(path, "r", encoding="utf-8") as fh:
            data = json.load(fh)
        return cls(
            name=data["name"],
            in_scope_essids=[e.strip() for e in data["in_scope_essids"]],
            in_scope_bssids=[b.strip().lower() for b in data["in_scope_bssids"]],
            window_start=datetime.fromisoformat(data["window_start"]),
            window_end=datetime.fromisoformat(data["window_end"]),
            deauth_permitted=bool(data.get("deauth_permitted", False)),
        )

    # ---- gate checks ---------------------------------------------------------------------

    def is_target_in_scope(self, bssid: str) -> bool:
        return bssid.strip().lower() in self.in_scope_bssids

    def is_essid_in_scope(self, essid: str) -> bool:
        return essid.strip() in self.in_scope_essids

    def is_within_window(self, now: datetime | None = None) -> bool:
        now = now or _now()
        return self.window_start <= now <= self.window_end

    @property
    def is_affirmed(self) -> bool:
        return self._affirmation is not None

    def affirm_authorization(self, typed_essid: str, auth_reference: str) -> Affirmation:
        """Operator affirmation required to leave IDLE. Raises if inputs don't match scope."""
        if not self.is_essid_in_scope(typed_essid):
            raise PermissionError(
                f"Typed ESSID {typed_essid!r} is not in engagement scope; cannot affirm."
            )
        if self.auth_reference_required and not auth_reference.strip():
            raise PermissionError("A written-authorization reference is required to affirm.")
        aff = Affirmation(typed_essid.strip(), auth_reference.strip(), _now())
        self._affirmation = aff
        return aff

    def assert_can_target(self, bssid: str, now: datetime | None = None) -> None:
        """Single choke point re-used by GUI *and* backend. Raises PermissionError if not allowed."""
        if not self.is_affirmed:
            raise PermissionError("Authorization has not been affirmed.")
        if not self.is_target_in_scope(bssid):
            raise PermissionError(f"BSSID {bssid} is not in the engagement allow-list.")
        if not self.is_within_window(now):
            raise PermissionError("Current time is outside the authorized engagement window.")


def example_engagement_file() -> dict:
    """Shape of an engagement file (see design/ui-wireframes.md screen 2)."""
    return {
        "name": "ACME-Q3-2026",
        "in_scope_essids": ["ACME-CORP"],
        "in_scope_bssids": ["aa:bb:cc:dd:ee:ff"],
        "window_start": "2026-07-03T09:00:00+00:00",
        "window_end": "2026-07-03T17:00:00+00:00",
        "deauth_permitted": True,
    }
