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

## Decisions on open questions (2026-11-07)

1. **LAN recognition:** server-side registry (owner choice). Each Onyx system registers with the server; the app lists peers on the same network from that registry. Trade-off: needs the server to be reachable to discover peers, so LAN discovery needs one server contact per session.
2. **Pending retention:** 1 week. The app warns the recipient and sender before deletion. Pending files are deleted at the end of that week.
3. **Notifications:** push notifications on every platform (web, Android, desktop). Email and SMS are future modules: list them as options in the Settings screen, disabled, and document them as future modules. Not implemented now.
4. **Web UI:** Internet route only. LAN item shown disabled.
5. **Emergency module:** separate module with its own screen and section. Document it as a future module.

## Future modules (placeholders only)

- Email and SMS notification channels (shown disabled in Settings).
- BLE and Wi-Fi Direct emergency P2P transfer (own screen and section).

## Open item

- The "spiral organisation chart" is not in this repository, its git history, or the project documents I searched. Only the 2026-10-25 audit mentions it, and it flags it as unverified. Location of the existing doc needs owner input.
