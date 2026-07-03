# Authorization, Legality, and Engagement Gating

> This is the most important document in the repository. If you build the tool, the authorization
> gate described here is a **required, non-optional** component — not a checkbox to remove.

## 1. The legal reality

Running an evil twin performs three separately-illegal-when-unauthorized acts:

| Act | What it is | Representative law |
|---|---|---|
| **Deauthentication** | Forcibly disconnecting clients from an AP | Interference with a communications service; in the US the FCC has fined operators for Wi-Fi deauth "jamming" (e.g. Marriott, 2014, $600k) |
| **AP impersonation** | Broadcasting another network's ESSID to lure clients | Wire fraud / unauthorized access precursor |
| **Credential capture** | Phishing a passphrase via captive portal | Unauthorized access (CFAA 18 U.S.C. §1030), Computer Misuse Act 1990 s.1–3, EU 2013/40/EU |

"I was just testing" is not a defense. **Written authorization scoped to the specific network is.**

## 2. What "authorized" means concretely

Before a single frame is transmitted, you must have, in writing, from someone with authority over
the target network:

- **Scope**: the exact ESSID(s) and/or BSSID(s) in scope, and a physical location.
- **Window**: the date/time range testing is permitted.
- **Rules of engagement**: whether client deauth is permitted, whether real users may be affected,
  and the maximum acceptable disruption.
- **Point of contact**: someone to call when something goes wrong (and it will).
- **Handling**: how captured material (handshakes, any entered passphrases) is stored, transmitted,
  and destroyed.

For CTF/lab use, the "authorization" is the range being your own equipment or a sanctioned event.

## 3. Engagement gating in the tool (design requirement)

The tool must not be operable as an ambient "point and shoot" weapon. The design forces an
**engagement** object to exist and be affirmed before any radio operation is allowed. This is the
`Engagement` model in [`src/core/engagement.py`](../src/core/engagement.py).

```
No Engagement loaded ──▶  All attack actions disabled in the GUI.
Engagement loaded    ──▶  Operator must type the in-scope ESSID/BSSID to confirm
                          + check "I have written authorization" + name the auth reference.
Target not in scope  ──▶  Attack actions remain disabled; a scan can list it but not target it.
Outside time window  ──▶  Attack actions disabled.
```

Design choices that make misuse harder without hurting legitimate use:

1. **Scope allow-list, not deny-list.** You can only target a BSSID you explicitly entered into the
   engagement scope. Scanning shows everything; *targeting* is restricted to the allow-list.
2. **Time-boxing.** The engagement carries a start/end timestamp; the orchestrator refuses to arm
   outside it.
3. **Mandatory evidence log.** Every state transition (scan, capture, twin-up, deauth, portal-hit,
   validation) is written to an append-only, timestamped engagement log for the report and for
   accountability. There is no "quiet mode."
4. **No credential exfiltration path.** Captured/entered secrets stay in the local engagement
   vault; there is no built-in feature to send them anywhere. The report references them by
   handle, not value.
5. **Data destruction.** A one-click "purge engagement" wipes captured handshakes and any entered
   secrets after the report is finalized.

These are the same principles that separate a professional testing platform (e.g. how commercial
tools like a WiFi Pineapple frame scope) from malware.

## 4. What this tool deliberately will not do

Per responsible-tooling limits, the design excludes:

- **Mass / opportunistic targeting** — no "attack every AP in range" mode. One in-scope target at
  a time.
- **Detection evasion / anti-forensics** — no MAC-randomization-to-evade, no log suppression, no
  vendor-fingerprint spoofing for the purpose of hiding.
- **Persistence** — nothing survives the engagement; the twin is torn down on stop.
- **Credential exfiltration** — see above.

If your engagement genuinely needs one of these, that is a conversation with the client and their
legal team, handled outside this tool.

## 5. Operator checklist (put this in the GUI's pre-flight screen)

- [ ] I have written authorization covering this exact ESSID/BSSID and location.
- [ ] Current time is inside the authorized window.
- [ ] The client POC knows testing is happening now.
- [ ] Deauth of real clients is explicitly permitted (or disabled in RoE).
- [ ] I understand entered passphrases are real user secrets and will be handled per the DPA.
- [ ] I have a rollback/teardown plan if the twin disrupts production.
