"""Attack state machine / orchestrator.

Drives the engagement lifecycle over a (stubbed) RadioBackend. Every transition: (1) checks it is
legal from the current state, (2) re-checks the engagement gate, (3) calls the backend, (4) writes
an evidence entry. The authoritative transition table lives in design/state-machine.md.
"""
from __future__ import annotations

from enum import Enum, auto

from .engagement import Engagement
from .evidence import EvidenceLog
import sys, os
sys.path.insert(0, os.path.dirname(os.path.dirname(__file__)))
from services.radio_backend import RadioBackend, ValidationResult  # noqa: E402


class AttackState(Enum):
    IDLE = auto()
    ARMED = auto()
    RECON = auto()
    HANDSHAKE_CAPTURED = auto()
    TWIN_ACTIVE = auto()
    MIGRATING = auto()
    VALIDATING = auto()
    COMPLETE = auto()
    ERROR = auto()


class TransitionError(RuntimeError):
    pass


class Orchestrator:
    def __init__(self, engagement: Engagement, backend: RadioBackend):
        self.engagement = engagement
        self.backend = backend
        self.log = EvidenceLog(engagement.name)
        self.state = AttackState.IDLE
        self.target_bssid: str | None = None
        self.target_essid: str | None = None
        self.target_channel: int | None = None
        self._handshake = False

    # ---- helpers -------------------------------------------------------------------------

    def _require(self, *states: AttackState):
        if self.state not in states:
            raise TransitionError(f"Action illegal in state {self.state.name}.")

    def _to(self, new: AttackState, action: str, **detail):
        self.state = new
        self.log.record(action, state=new.name, **detail)

    # ---- transitions ---------------------------------------------------------------------

    def affirm_and_arm(self, typed_essid: str, auth_reference: str,
                       target_bssid: str, target_essid: str, channel: int):
        self._require(AttackState.IDLE)
        self.engagement.affirm_authorization(typed_essid, auth_reference)
        self.engagement.assert_can_target(target_bssid)  # scope + window check
        self.target_bssid, self.target_essid, self.target_channel = (
            target_bssid, target_essid, channel)
        self._to(AttackState.ARMED, "affirm_and_arm", target=target_bssid, ref=auth_reference)

    def scan(self):
        self._require(AttackState.ARMED, AttackState.RECON)
        aps = self.backend.scan()
        self._to(AttackState.RECON, "scan", found=len(aps))
        return aps

    def capture_handshake(self):
        self._require(AttackState.RECON)
        self.engagement.assert_can_target(self.target_bssid)
        self._handshake = self.backend.capture_handshake(self.target_bssid)
        if self._handshake:
            self._to(AttackState.HANDSHAKE_CAPTURED, "capture_handshake", ok=True)
        return self._handshake

    def start_twin(self):
        self._require(AttackState.HANDSHAKE_CAPTURED)
        self.engagement.assert_can_target(self.target_bssid)  # re-check window
        self.backend.start_twin(self.target_essid, self.target_bssid, self.target_channel)
        self._to(AttackState.TWIN_ACTIVE, "start_twin", essid=self.target_essid)

    def deauth(self, client: str | None = None):
        self._require(AttackState.TWIN_ACTIVE, AttackState.MIGRATING)
        if not self.engagement.deauth_permitted:
            raise PermissionError("Deauth is not permitted by the engagement rules of engagement.")
        self.engagement.assert_can_target(self.target_bssid)
        self.backend.deauth(self.target_bssid, client)
        self._to(AttackState.MIGRATING, "deauth", client=client or "broadcast")

    def submit_passphrase(self, candidate: str) -> ValidationResult:
        self._require(AttackState.TWIN_ACTIVE, AttackState.MIGRATING)
        if not self._handshake:
            # Invariant: never solicit a secret we cannot verify.
            raise TransitionError("Cannot validate without a captured handshake.")
        self.state = AttackState.VALIDATING
        result = self.backend.validate_passphrase(candidate)
        if result.verified:
            self._to(AttackState.COMPLETE, "validation_verified", handle=result.handle)
        else:
            self._to(AttackState.MIGRATING, "validation_rejected")
        return result

    def stop(self):
        self.backend.stop()
        self._to(AttackState.COMPLETE, "stop")

    def fault(self, reason: str):
        try:
            self.backend.stop()
        finally:
            self._to(AttackState.ERROR, "fault", reason=reason)
