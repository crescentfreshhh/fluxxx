"""Append-only, tamper-evident evidence log.

Every state transition and radio action writes exactly one entry. Entries form a hash chain (each
carries the SHA-256 of the previous entry) so tampering is detectable. There is no delete API — the
log is the backbone of the engagement report and of accountability. See docs/COMPONENTS.md.
"""
from __future__ import annotations

import hashlib
import json
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone


@dataclass(frozen=True)
class EvidenceEntry:
    seq: int
    at: str
    engagement: str
    action: str
    detail: dict
    prev_hash: str
    this_hash: str = ""

    def compute_hash(self) -> str:
        payload = json.dumps(
            {"seq": self.seq, "at": self.at, "engagement": self.engagement,
             "action": self.action, "detail": self.detail, "prev_hash": self.prev_hash},
            sort_keys=True,
        )
        return hashlib.sha256(payload.encode()).hexdigest()


class EvidenceLog:
    def __init__(self, engagement_name: str):
        self.engagement = engagement_name
        self._entries: list[EvidenceEntry] = []

    def record(self, action: str, **detail) -> EvidenceEntry:
        prev = self._entries[-1].this_hash if self._entries else "0" * 64
        entry = EvidenceEntry(
            seq=len(self._entries),
            at=datetime.now(timezone.utc).isoformat(),
            engagement=self.engagement,
            action=action,
            detail=detail,
            prev_hash=prev,
        )
        entry = EvidenceEntry(**{**asdict(entry), "this_hash": entry.compute_hash()})
        self._entries.append(entry)
        return entry

    def verify_chain(self) -> bool:
        prev = "0" * 64
        for e in self._entries:
            if e.prev_hash != prev or e.this_hash != e.compute_hash():
                return False
            prev = e.this_hash
        return True

    @property
    def entries(self) -> list[EvidenceEntry]:
        return list(self._entries)
