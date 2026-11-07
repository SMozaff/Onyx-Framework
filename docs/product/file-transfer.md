# File transfer (owner decisions, 2026-11-07)

Status: decisions recorded. Implementation not started. Open questions listed at the end.

## Scope

1. **Internet transfer (in scope).** Files sent through the cloud. Each attempt is limited to **100 MB**.
2. **LAN transfer (in scope).** Direct transfer to other Onyx-equipped systems on the same local network.
3. **Everything else is a stub.** Shown in the UI as **disabled** items with placeholder labels. This includes the BLE and Wi-Fi Direct P2P module (see below).

## Route selection (inside a session)

- The user can switch between **LAN** and **Internet** at any time in the session. The switch is instant.
- Every platform must offer the same routes: web UI, Android, and desktop.

## LAN behaviour

- The app recognises every other Onyx-equipped system on the local network and lists them.
- The user selects a recipient and sends the file.
- **Recipient not reachable on LAN:**
  - The app warns the sender.
  - The sender may keep the file **pending on the server** for the recipient, or switch to **Internet** immediately.
  - Pending files are visible to the recipient, who receives a notification ("you have a package on your domestic LAN; download when you are in the office").
- **Internet route after a switch:** the file is delivered immediately to the recipient. The recipient is notified. **Delivery notifications** are required for the sender.

## Emergency P2P module (separate, deferred)

- Peer-to-peer transfer over **BLE** and **Wi-Fi Direct**, for use with no stable internet or LAN.
- Designed as a **separate module** with its own screen and section, because it is not a routine action.
- Not implemented now. Placeholder only.
- Goals: the most secure and reliable transfer in emergencies.

## Open questions (owner input needed)

1. **LAN recognition:** how does the app know another system is an Onyx system on the LAN? Options include a broadcast announcement signed with the organisation key, or a server-side registry. Which?
2. **Pending retention:** how long may a pending file stay on the server, and what is the storage limit per user?
3. **Notifications channel:** in-app only, or also push (mobile) and email? Which for each platform?
4. **Web UI:** a browser cannot discover LAN peers directly. Should web show only the Internet route and a disabled LAN item?
5. **Documentation home for the emergency module:** the "spiral organisation chart" is not present in this repository (see `docs/audits/2026-10-25/ONYX_Audit_Report.md`, GAP-06). Should this module be documented in `docs/product/` as shown here?
