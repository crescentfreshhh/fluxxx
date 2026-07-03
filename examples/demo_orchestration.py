"""End-to-end demo of the orchestration layer against a FAKE backend.

Proves the safety gate + state machine work without any real radio operation. Run:

    python examples/demo_orchestration.py

This exercises: authorization affirmation, scope allow-listing, the full state machine, the
tamper-evident evidence log, the no-exfil vault, and report generation — all against a simulated
backend. No 802.11 frames are transmitted; the real RadioBackend is stubbed (raises).
"""
import os
import sys
from datetime import datetime, timedelta, timezone

sys.path.insert(0, os.path.join(os.path.dirname(__file__), "..", "src"))

from core.engagement import Engagement                      # noqa: E402
from core.orchestrator import Orchestrator, AttackState     # noqa: E402
from core.vault import Vault                                # noqa: E402
from core import report                                     # noqa: E402
from services.radio_backend import RadioBackend, AccessPoint, ValidationResult  # noqa: E402


class FakeRadioBackend(RadioBackend):
    """Simulated backend for testing the orchestrator. Transmits nothing."""

    def __init__(self, correct="hunter2-correct-horse"):
        self._correct = correct
        self.vault = Vault("ACME-Q3-2026")

    def scan(self):
        return [AccessPoint("ACME-CORP", "aa:bb:cc:dd:ee:ff", 6, -42, "WPA2", 3),
                AccessPoint("CoffeeShop", "12:34:56:78:9a:bc", 36, -70, "WPA2", 1)]

    def capture_handshake(self, bssid):
        return True  # pretend we captured it

    def start_twin(self, essid, bssid, channel):
        print(f"    [fake] twin up: {essid} (open) ch{channel}")

    def deauth(self, bssid, client=None):
        print(f"    [fake] deauth {client or 'broadcast'} off {bssid}")

    def validate_passphrase(self, candidate):
        if candidate == self._correct:
            return ValidationResult(True, self.vault.put(candidate))
        return ValidationResult(False)

    def stop(self):
        print("    [fake] twin torn down, adapter restored")


def main():
    now = datetime.now(timezone.utc)
    eng = Engagement(
        name="ACME-Q3-2026",
        in_scope_essids=["ACME-CORP"],
        in_scope_bssids=["aa:bb:cc:dd:ee:ff"],
        window_start=now - timedelta(hours=1),
        window_end=now + timedelta(hours=1),
        deauth_permitted=True,
    )
    backend = FakeRadioBackend()
    orch = Orchestrator(eng, backend)

    print("1. Try to arm an OUT-OF-SCOPE target (must fail):")
    try:
        orch.affirm_and_arm("ACME-CORP", "SOW-2026-0142",
                            "12:34:56:78:9a:bc", "CoffeeShop", 36)
    except PermissionError as e:
        print(f"    blocked as designed: {e}")

    print("2. Arm the IN-SCOPE target:")
    orch.affirm_and_arm("ACME-CORP", "SOW-2026-0142", "aa:bb:cc:dd:ee:ff", "ACME-CORP", 6)
    print(f"    state = {orch.state.name}")

    print("3. Recon + handshake capture:")
    orch.scan()
    orch.capture_handshake()
    print(f"    state = {orch.state.name}")

    print("4. Stand up twin + deauth:")
    orch.start_twin()
    orch.deauth()
    print(f"    state = {orch.state.name}")

    print("5. Wrong passphrase (rejected, re-prompt):")
    r1 = orch.submit_passphrase("wrong-guess")
    print(f"    verified={r1.verified}  state={orch.state.name}")

    print("6. Correct passphrase (verified):")
    r2 = orch.submit_passphrase("hunter2-correct-horse")
    print(f"    verified={r2.verified}  handle={r2.handle}  state={orch.state.name}")

    assert orch.state == AttackState.COMPLETE
    assert orch.log.verify_chain(), "evidence chain must verify"

    print("\n7. Report (passphrase cited by handle, never value):\n")
    print(report.generate(eng, orch.log, handshake_captured=True,
                          passphrase_handle=r2.handle))


if __name__ == "__main__":
    main()
