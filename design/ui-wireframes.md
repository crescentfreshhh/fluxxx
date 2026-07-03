# GUI Wireframes

Text wireframes for the five Windows GUI screens. In a production build these are WPF/WinUI views;
here they define layout and the intents each control dispatches to `core/`.

## 1. Dashboard

```
┌─ Fluxxx ─────────────────────────────────────────────── [_][□][x] ┐
│  Engagement:  ACME-Q3-2026        Backend:  ● connected (WSL2)     │
│  State:       ARMED               Adapter:  wlan1mon (MT7612U)     │
│                                                                    │
│  ┌── Authorization ──────────────────────────────────────────┐    │
│  │  Scope:   ACME-CORP  (BSSID aa:bb:cc:dd:ee:ff)  IN SCOPE ✓ │    │
│  │  Window:  2026-07-03 09:00 – 17:00   (in window ✓)          │    │
│  │  Deauth:  permitted per RoE ✓                              │    │
│  └───────────────────────────────────────────────────────────┘    │
│                                                                    │
│           [ Go to Attack Console ]      [ ■■  STOP ALL  ■■ ]        │
└────────────────────────────────────────────────────────────────────┘
```

## 2. Engagement setup

```
┌─ Engagement ───────────────────────────────────────────────────────┐
│  [ Load engagement… ]   [ New engagement… ]                        │
│                                                                    │
│  Client / name ......... [ ACME Corp — Q3 WLAN assessment      ]   │
│  In-scope ESSID(s) ..... [ ACME-CORP                           ]   │
│  In-scope BSSID(s) ..... [ aa:bb:cc:dd:ee:ff                   ]   │
│  Window start / end .... [ 2026-07-03 09:00 ] [ 17:00 ]           │
│  Deauth permitted ...... (•) yes  ( ) no                          │
│                                                                    │
│  ┌── Authorization affirmation (required to arm) ─────────────┐    │
│  │  Re-type in-scope ESSID:  [ ______________ ]               │    │
│  │  Written-auth reference:  [ SOW-2026-0142  ]               │    │
│  │  [x] I have written authorization for this exact network.  │    │
│  │                              [ Affirm & Arm ]              │    │
│  └───────────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────────┘
```

## 3. Recon

```
┌─ Recon ────────────────────────────────────────────────────────────┐
│  [ Scan ]   channel-hop: [x]        (results from backend scan())   │
│  ┌───────────────────────────────────────────────────────────┐    │
│  │ ESSID        BSSID              ch  sig  enc     clients    │    │
│  │ ACME-CORP    aa:bb:cc:dd:ee:ff   6  -42  WPA2    3   ◀ SCOPE│    │
│  │ ACME-GUEST   aa:bb:cc:dd:ee:00  11  -55  WPA2    8   (n/s)  │    │
│  │ CoffeeShop   12:34:…            36  -70  WPA2    1   (n/s)  │    │
│  └───────────────────────────────────────────────────────────┘    │
│  Only IN-SCOPE rows are selectable as a target.                    │
│  Selected target: ACME-CORP        [ Capture handshake ]           │
└────────────────────────────────────────────────────────────────────┘
```

## 4. Attack console (the state machine, visualized)

```
┌─ Attack Console ───────────────────────────────────────────────────┐
│  ARMED ─▶ RECON ─▶ [HANDSHAKE ✓] ─▶ TWIN ─▶ MIGRATING ─▶ VALIDATE  │
│                                        ▲ you are here               │
│                                                                    │
│  [ Start twin ]  [ Deauth burst ]*  [ Stop ]      *RoE-gated       │
│                                                                    │
│  ┌── Live events ────────────────────────────────────────────┐    │
│  │ 12:04:11  twin up: ACME-CORP (open) ch6                    │    │
│  │ 12:04:19  client 3c:… migrated to twin                     │    │
│  │ 12:04:40  portal hit from 3c:…                             │    │
│  │ 12:04:52  passphrase submitted → VALIDATING                │    │
│  │ 12:04:53  validation: REJECTED (re-prompt)                 │    │
│  └───────────────────────────────────────────────────────────┘    │
└────────────────────────────────────────────────────────────────────┘
```

## 5. Report

```
┌─ Report ───────────────────────────────────────────────────────────┐
│  Engagement:  ACME-Q3-2026                                         │
│  Handshake captured:  yes                                          │
│  Passphrase recovered: yes  (handle: vault:acme:pmk-01)  [Reveal]  │
│  Timeline: (from tamper-evident evidence log)  …                   │
│  Remediation: WPA3-SAE + PMF required, WIPS rogue-AP rules …       │
│                                                                    │
│  [ Preview ]   [ Export PDF ]        [ ⚠ Purge engagement data ]   │
└────────────────────────────────────────────────────────────────────┘
```
