# Hardware parity: pedals, MIDI, styluses and displays

> **Last reviewed:** 2026-10-28 · **Last updated:** 2026-10-31 · **Change:** patch (M4 part 4: HW-20 and HW-22 partial in the desktop app, HW-21 waits on winit; earlier: minor (HW-22 palm rejection with the platforms' own behaviour; earlier: Windows tablet is the first test platform, pedals in M3; earlier: HW-12 and HW-13 from the Microsoft Store description; earlier: major, first version) · **Target:** enScore 3.7 (2026)

Status and evidence as in [`ui-parity.md`](ui-parity.md). Every **done** row names the device
models and OS versions it was tested on.

**Summary (estimated):** 0% ready.

## Page-turn pedals

| Id | Target | Rows | Status | Tested on |
|---|---|---|---|---|
| HW-1 | Keyboard-emulating Bluetooth pedals (arrow keys, Page Up/Down, Space/Enter) turn pages by default | TURN-4 | missing | |
| HW-2 | Learn mode: press a pedal to bind it; profiles per pedal; long-press as a second action | TURN-4 | missing | |
| HW-3 | Debounce and no double turns on key repeat | TURN-4 | missing | |
| HW-4 | Pedals work while the on-screen keyboard is hidden (mobile) | TURN-4 | missing | |

Test devices to collect: AirTurn (DUO, BT200 series), PageFlip (Firefly, Butterfly), Donner, a generic
USB keyboard pedal. The owner's Windows tablet is the first test platform (pedals land in M3).

## MIDI

| Id | Target | Rows | Status | Tested on |
|---|---|---|---|---|
| HW-10 | MIDI input (USB, BLE MIDI, network RTP-MIDI, Web MIDI) with device picker | MIDI-4 | missing | |
| HW-11 | MIDI learn maps a CC, note or program message to turn/command | TURN-5, MIDI-1 | missing | |
| HW-12 | On score open, send Program Change, Bank Select (CC 0/32) and Song Select, one message or an ordered list | MIDI-2, MIDI-3 | missing | |
| HW-13 | A setlist entry overrides the score's MIDI messages, so one piece calls up different sounds in different setlists | MIDI-5 | missing | |

## Styluses

| Id | Target | Rows | Status | Tested on |
|---|---|---|---|---|
| HW-20 | Pressure and tilt from Apple Pencil, Windows Ink / Surface Pen, Android stylus (S Pen), Wacom on desktop | ANNO-2 | partial | (desktop: Windows Ink pressure through winit's touch force, untested on a device; no tilt in winit 0.30) |
| HW-21 | Eraser end and barrel buttons (barrel or double-tap switches pen and eraser by default) | ANNO-11 | missing | (`annotate.pen.button` is ready; winit 0.30 reports neither the eraser end nor the barrel button) |
| HW-22 | Palm rejection while the pen draws, working with the platform's own: Windows' *Ignore touch input when I'm using my pen* setting (can be off or hidden), Android 13+ palm flags (cancelled touches); touch lock | ANNO-14 | partial | (in the desktop app through `scorecraft-input`'s filter, [spec](spec/ink.md#anno-14-pen-only-and-draw-with-touch-palm-rejection); a pen is told from a finger by its pressure; needs a check on a Windows tablet) |

## Displays and power

| Id | Target | Rows | Status | Tested on |
|---|---|---|---|---|
| HW-30 | Keep the screen awake while reading or in a setlist; release on leave | READ-9 | missing | |
| HW-31 | E-ink devices (Boox, reMarkable-class Android readers): no animation, partial refresh friendly | PLAT-2 | missing | |
| HW-32 | High-DPI and external displays (projector or second screen mirror) | inferred | missing | |

## Cameras and scanners

| Id | Target | Rows | Status | Tested on |
|---|---|---|---|---|
| HW-40 | Device camera capture on Android, iOS and desktop webcams | IMP-1 | missing | |
| HW-41 | eSCL (AirScan) network scanners, flatbed and feeder | IMP-2 | missing | |

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-10-31 | patch | M4 part 4: the pen in the desktop app; HW-20 and HW-22 partial; HW-21 needs pen buttons from winit |
| 2026-10-31 | minor | HW-22: palm rejection on each platform, from the ink spec |
| 2026-10-28 | minor | The owner's Windows tablet is the first pedal and pen test platform; pedals move to M3 |
| 2026-10-28 | minor | HW-12 takes a list of messages; HW-13 per-setlist MIDI (MIDI-5), from the Microsoft Store description |
| 2026-10-28 | major | First version |
