# AI Usage

Disclosure of AI usage, following ETHGlobal's guidance.

## Tools used

- **Claude Code** (Anthropic) — in use as of 2026-09-26

## Work assisted

- `face-auth-ui/` — design, implementation, and device compatibility work for the
  face-authentication terminal UI (React) (transpiling for Chromium 74 / CSS fallbacks)
- `face-auth-ui/device-shell/` — implementation and build scripts for the terminal's WebView shell APK
- On-device (Hi-CARA) verification procedure (adb reverse / install / testing)

## Human involvement

- Product specification and architecture decisions (host PC CLI control of operating modes,
  on-device engine execution, placement of the ZKP pipeline) were made by the team
- On-device face-authentication behavior and camera behavior were verified by the team
- Generated code was reviewed by the team and validated through builds and device testing

## Notes

- The face-authentication engine (SAFR eSDK) is a commercial engine embedded in the device;
  its materials are not redistributable and are not included in the repository (each developer
  places them locally)
