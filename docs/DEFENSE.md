# Defense: Detecting and Defeating Evil-Twin Attacks

The point of running an authorized evil-twin assessment is to produce *this*: concrete remediation
the client can act on. This document is the blue-team half of the tool and feeds directly into the
generated report.

## Why the attack works (and therefore how to kill it)

The attack chain has four dependencies. Break any one and it fails:

| Dependency | Defense that removes it |
|---|---|
| Clients will deauth off the real AP | **802.11w Protected Management Frames (PMF)** — cryptographically protects deauth/disassoc frames so forged ones are ignored |
| Clients will associate with a same-ESSID open twin | **WPA3-SAE / WPA2-Enterprise (802.1X)** — mutual auth; a rogue open AP can't complete association |
| A user will type the passphrase into a portal | **User training** + the fact that legitimate WPA networks never ask for the password via a web page |
| The tester can validate the passphrase offline | **WPA3-SAE** is resistant to offline dictionary/handshake validation by design (Dragonfly handshake) |

**Headline recommendation for almost every report: deploy WPA3-SAE with PMF required.** It removes
three of the four dependencies at once.

## Detection (what a defender should monitor)

1. **Duplicate ESSID on multiple BSSIDs / channels.** A WIDS/WIPS (Wireless Intrusion Detection)
   flags a second AP broadcasting your ESSID with an unexpected BSSID.
2. **Deauth floods.** A spike in deauthentication/disassociation frames is the classic signature of
   the client-migration step. PMF makes these ineffective *and* the flood itself is detectable.
3. **Unexpected open AP with your name.** Your corporate ESSID should never appear as an open
   network. WIPS rules can alarm on this.
4. **Rogue AP by BSSID/vendor OUI.** Maintain an allow-list of your real AP BSSIDs; anything else
   broadcasting your ESSID is hostile.
5. **Captive-portal / DNS anomalies.** Clients suddenly hitting a local DNS that answers everything
   with one IP (the portal) is a tell on managed endpoints.

Tooling: enterprise WLAN controllers (Cisco, Aruba, Meraki) have built-in rogue-AP and WIPS
detection; open-source options include Kismet (rogue AP + deauth detection) and nzyme.

## Hardening checklist (goes into the report)

- [ ] **WPA3-SAE** (or WPA2/WPA3 transition mode as a stepping stone).
- [ ] **PMF (802.11w) set to *required*.**
- [ ] For enterprise: **802.1X (WPA2/3-Enterprise)** with server-cert validation enforced on
      clients (so a rogue RADIUS/portal can't impersonate).
- [ ] **WIPS/WIDS** enabled with rogue-AP and deauth-flood rules; alerting wired to the SOC.
- [ ] **BSSID allow-list** of legitimate APs.
- [ ] **User awareness**: your Wi-Fi will never ask for its password via a web page.
- [ ] Disable client auto-join to open networks with saved-network names.
- [ ] Consider **802.11r/k/v** roaming so legitimate roaming doesn't look anomalous (reduces false
      positives, improving detection signal-to-noise).

## What a "passed" assessment looks like

If the target is WPA3-SAE + PMF-required, the assessment should *fail to progress*: the twin can be
stood up but clients won't migrate on forged deauth, and any captured material can't be validated
offline. That failure is the win — and the report should say so explicitly.
