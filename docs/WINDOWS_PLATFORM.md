# The Windows Platform Reality

You asked for a **Windows-based GUI**. This is a great choice for the operator experience and a
hard one for the radio work, and understanding *why* is half the design. This document is the
single most important technical decision in the project.

## The core problem: Windows can't do the radio part natively

Fluxion is a Linux (bash) tool for a reason. The evil-twin workflow needs three low-level 802.11
capabilities:

| Capability | Needed for | Windows native support |
|---|---|---|
| **Monitor mode** | Sniffing raw 802.11 frames (recon, handshake capture) | ❌ Not exposed by the Windows WLAN/NDIS stack for consumer adapters |
| **Packet injection** | Sending deauthentication frames | ❌ No supported public API |
| **AP / master mode with full control** | Standing up the evil twin + captive portal DHCP/DNS | ⚠️ "Mobile Hotspot" exists but you cannot control ESSID/BSSID/timing at the frame level |

The old NDIS-based approach (Microsoft's Network Monitor + the AirPcap adapters from Riverbed/CACE)
could do monitor-mode capture on Windows, but AirPcap is discontinued, expensive, and never
supported injection. So: **you cannot build a self-contained Windows evil twin against arbitrary
Wi-Fi adapters.** Any design that claims to is either wrong or relies on a very specific driver.

## The design answer: split the tool into a Windows front end + a Linux radio backend

This is the whole architecture in one sentence. The Windows GUI is a **control plane**; the actual
802.11 work runs on a Linux **radio backend** that talks to a monitor-mode-capable adapter. There
are three concrete ways to realize the backend, and the tool should support all three behind one
adapter interface:

### Option A — WSL2 + USB passthrough (single laptop) — recommended default
- Windows 11 GUI runs natively.
- WSL2 runs the Linux backend (aircrack-ng, hostapd, dnsmasq).
- A supported **external USB Wi-Fi adapter** (Atheros AR9271, Ralink RT3070/RT5372, MediaTek
  MT7612U — the usual monitor-mode-capable chipsets) is passed into WSL2 with **`usbipd-win`**.
- Pros: one machine, GUI and radio together. Cons: WSL2 USB/WLAN passthrough is finicky; the
  onboard Wi-Fi card generally **won't** work — you need the external adapter.

### Option B — Windows GUI + Linux VM
- The Linux backend runs in Hyper-V/VMware/VirtualBox with the USB adapter passed through.
- Cleaner isolation than WSL2, slightly more overhead. Good for a lab.

### Option C — Windows GUI + remote Linux radio node (recommended for real engagements)
- The backend runs on a small dedicated Linux device on-site (Raspberry Pi, or a
  purpose-built board) with the right adapter.
- The Windows laptop is purely the operator console, talking to the node over a local control
  channel (see the RPC design in [`ARCHITECTURE.md`](ARCHITECTURE.md)).
- Pros: mirrors how professionals actually work (drop node + remote console), keeps the noisy radio
  gear separate from the operator's machine, best range flexibility. Cons: two devices.

```
        ┌─────────────────────────────┐         control channel          ┌───────────────────────────┐
        │      WINDOWS (GUI host)      │   (local gRPC/WebSocket, mTLS)    │    LINUX RADIO BACKEND     │
        │                             │ ───────────────────────────────▶ │  (WSL2 / VM / on-site Pi)  │
        │  WinUI 3 / WPF front end     │ ◀─────────────────────────────── │                           │
        │  Engagement + auth model     │        events / status / logs    │  monitor-mode adapter      │
        │  Report generator            │                                  │  aircrack-ng, hostapd,     │
        │                             │                                  │  dnsmasq, hcxtools         │
        └─────────────────────────────┘                                  └───────────────────────────┘
```

The key architectural payoff: **the GUI never speaks 802.11.** It sends high-level intents ("scan",
"arm twin for engagement X's in-scope BSSID", "stop") and renders state. All the platform-specific,
sensitive radio work lives behind the `RadioBackend` interface (see
[`src/services/radio_backend.py`](../src/services/radio_backend.py)), and this repo ships that
interface **stubbed**.

## Windows GUI technology choice

For a modern Windows GUI, in rough order of recommendation:

| Stack | When to pick it | Notes |
|---|---|---|
| **WinUI 3 / Windows App SDK (C#)** | You want the current native Windows look, packaging as MSIX | Best long-term Microsoft-supported path |
| **WPF (C#/.NET)** | You want maturity, huge ecosystem, MVVM tooling | Rock-solid, still excellent for tooling apps |
| **.NET MAUI** | You might want the console to also run on macOS later | More churn |
| **Python + PySide6/Qt** | You want the GUI and the orchestration in one language, cross-platform | This skeleton uses Python for the orchestration layer so the whole thing reads in one language; a production build would likely use WPF/WinUI for the shell and keep Python (or Go) for the node |

This skeleton is written in **Python** for the orchestration/core so the design reads end-to-end in
one language, with the GUI layer described as a thin MVVM shell. Swap the shell for WPF/WinUI in a
real build without touching `core/` or `services/`.

## Bill of materials (what an operator actually needs)

- A Windows 11 laptop.
- **A monitor-mode + injection capable USB Wi-Fi adapter.** This is non-negotiable and is the item
  people forget. Common known-good chipsets: Atheros AR9271 (2.4GHz only), MediaTek MT7612U
  (dual-band), Ralink RT5372. Check current driver/monitor-mode status before buying.
- For Option C: a Raspberry Pi 4/5 or equivalent to be the radio node.
- `usbipd-win` (Option A), or a hypervisor (Option B).
