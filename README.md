# Fluxxx — Evil-Twin AP Assessment Toolkit (Design)

**Status:** Design specification and reference skeleton. This repository contains the
**architecture and design** for a Fluxion-style wireless security assessment tool with a
Windows-based GUI front end. It intentionally does **not** ship turnkey deauthentication or
credential-capture exploit payloads.

Fluxxx is intended for **authorized WPA/WPA2-PSK security assessments** — engagements where you
have explicit, written permission to test the target wireless network. It automates the workflow
that a penetration tester performs by hand: enumerate a target ESSID, capture a WPA handshake,
stand up a look-alike ("evil twin") access point with a captive portal, and validate any
passphrase a user supplies against the captured handshake.

> ⚠️ **Read [`docs/AUTHORIZATION.md`](docs/AUTHORIZATION.md) before anything else.** Operating an
> evil twin, deauthenticating clients, or capturing credentials against a network you do not own
> or lack written authorization to test is illegal in most jurisdictions (e.g. US CFAA 18 U.S.C.
> § 1030, UK Computer Misuse Act 1990, EU directive 2013/40/EU). This project is for licensed
> testers, red teams, CTF ranges, and lab education only.

---

## What this repository is

| You get | You do **not** get |
|---|---|
| Full system architecture ([`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)) | A packaged, ready-to-fire attack binary |
| Component-by-component design ([`docs/COMPONENTS.md`](docs/COMPONENTS.md)) | Working 802.11 deauth injection code |
| The Windows platform reality check ([`docs/WINDOWS_PLATFORM.md`](docs/WINDOWS_PLATFORM.md)) | A live credential-harvesting portal wired to a validator |
| An engagement/authorization gating model ([`docs/AUTHORIZATION.md`](docs/AUTHORIZATION.md)) | Detection-evasion / anti-forensics tooling |
| A GUI + orchestration skeleton (`src/`) with the sensitive operations stubbed | |
| Blue-team detection guidance ([`docs/DEFENSE.md`](docs/DEFENSE.md)) | |

The stubbed operations are the parts that turn a design into a weapon. Anyone building this for a
real, authorized engagement will supply those from established tooling (aircrack-ng suite, hostapd,
hcxtools, etc.) under their own legal authority. Keeping them out of this repo keeps the design
useful for learning and planning without being copy-paste operational.

## The core idea (how a Fluxion-style attack works)

An evil-twin passphrase-recovery attack is a **social-engineering** attack, not a cryptographic
one. It never brute-forces the WPA key. Instead:

1. **Recon** — find the target AP's ESSID, BSSID, channel, and connected clients.
2. **Handshake capture** — obtain a WPA 4-way handshake (passively, or by nudging a client to
   reconnect). The handshake lets you *verify* a candidate passphrase offline.
3. **Evil twin** — broadcast an open AP with the *same ESSID* on a nearby channel.
4. **Client migration** — clients are pushed off the real AP (deauth) so they associate with the
   twin. This is the targeted-DoS component and the most legally/ethically loaded step.
5. **Captive portal** — the twin serves a spoofed "router firmware update / re-enter your WiFi
   password" page. This is the phishing component.
6. **Validation** — any passphrase entered is checked against the captured handshake. A correct
   passphrase verifies; a wrong one is rejected and the portal asks again.

The insight worth internalizing for defense: **step 6 is why the attack works at all.** Without a
captured handshake to validate against, the portal is just a guess. This is also why the primary
defense — WPA3/SAE and 802.11w Protected Management Frames — is so effective (see
[`docs/DEFENSE.md`](docs/DEFENSE.md)).

## Repository layout

```
fluxxx/
├── README.md                  ← you are here
├── docs/
│   ├── AUTHORIZATION.md        ← legal + engagement gating (READ FIRST)
│   ├── ARCHITECTURE.md         ← system architecture, data flow, state machine
│   ├── COMPONENTS.md           ← per-module design + responsibilities
│   ├── WINDOWS_PLATFORM.md     ← why Windows is hard; the WSL2/VM/agent split
│   └── DEFENSE.md              ← detection + hardening (the blue-team half)
├── design/
│   ├── state-machine.md        ← engagement lifecycle states + transitions
│   └── ui-wireframes.md        ← GUI screen-by-screen wireframes
└── src/
    ├── gui/                    ← Windows GUI front end (design + skeleton)
    ├── core/                   ← orchestration, engagement/authorization model
    └── services/              ← adapter interfaces to the Linux attack backend (stubbed)
```

## Quick start (for readers)

1. Read [`docs/AUTHORIZATION.md`](docs/AUTHORIZATION.md).
2. Read [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for the big picture.
3. Read [`docs/WINDOWS_PLATFORM.md`](docs/WINDOWS_PLATFORM.md) — it explains why a "Windows GUI
   evil twin" is really a Windows front end driving a Linux radio backend.
4. Browse `src/` for the skeleton.

## License / intent

Provided for **educational and authorized security-testing** purposes only. See
[`docs/AUTHORIZATION.md`](docs/AUTHORIZATION.md). The authors do not condone and are not
responsible for unauthorized use.
