"""Local secret vault for the one sensitive datum (a recovered passphrase).

Design intent (see docs/AUTHORIZATION.md §3 and docs/COMPONENTS.md): secrets are stored locally,
encrypted at rest, and referenced elsewhere by opaque handle. There is deliberately NO export/send
method — the absence of an exfiltration path is a feature. reveal() is an explicit, logged operator
action.

This ships as a safe in-memory reference implementation. A production build would back it with an
OS keystore (Windows DPAPI / Credential Manager) or a libsodium-encrypted file.
"""
from __future__ import annotations

import os
import secrets as _secrets


class Vault:
    def __init__(self, engagement_name: str):
        self.engagement = engagement_name
        self._store: dict[str, bytes] = {}

    def put(self, secret: str) -> str:
        """Store a secret, return an opaque handle. The plaintext is not logged or returned."""
        handle = f"vault:{self.engagement}:{_secrets.token_hex(4)}"
        # Reference impl: XOR with a random pad kept only in memory. Replace with DPAPI/libsodium.
        pad = os.urandom(len(secret.encode()))
        self._store[handle] = bytes(a ^ b for a, b in zip(secret.encode(), pad))
        self._pads = getattr(self, "_pads", {})
        self._pads[handle] = pad
        return handle

    def reveal(self, handle: str) -> str:
        """Explicit, should-be-logged operator action to recover a stored secret."""
        pad = self._pads[handle]
        return bytes(a ^ b for a, b in zip(self._store[handle], pad)).decode()

    def purge(self) -> None:
        """Securely drop all secrets at engagement close."""
        self._store.clear()
        getattr(self, "_pads", {}).clear()

    # Intentionally no export(), send(), sync(), or upload() — see module docstring.
