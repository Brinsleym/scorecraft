# Control protocol

> **Last reviewed:** 2026-10-31 · **Last updated:** 2026-11-08 · **Change:** minor (`ui.pointer` takes a `button`, to right-click; earlier: M3, first methods live in the desktop app) · **Target:** enScore 3.7 (2026)

How agents and scripts drive the running ScoreCraft app. Same design as PhotoCraft's control
channel: JSON lines over an authenticated loopback TCP socket, with the engine's commands as the
main vocabulary.

## Starting it

```sh
scorecraft --control            # prints  control: 127.0.0.1:<port> token=<64 hex chars>
scorecraft --control-port 7400 --control-file control.txt score.pdf
                                # fixed port; also writes the line to a file (release Windows
                                # builds have no console)
scorecraft-cli mcp --bridge     # MCP server that forwards to the running app
scorecraft-cli serve            # headless JSON server (no window)
```

- Binds `127.0.0.1` only. The first line of every connection must be
  `{"auth":"<token>"}`; the 256-bit token is random per launch unless `SCORECRAFT_CONTROL_TOKEN`
  is set.
- A connection has 10 s to authenticate. The token comes from the OS random source.
- One request per line, one reply per line: `{"id":1,"method":"…","params":{…}}` →
  `{"id":1,"result":…}` or `{"id":1,"error":{"code":"…","message":"…"}}`.

## Methods

**Live (M3):** `command`, `command_batch`, `command_list`, `ui.state`, `ui.set`, `ui.key`,
`ui.pointer` and `ui.screenshot`, in the desktop app (`apps/scorecraft/src/control.rs`, transport
in `scorecraft-automation::control`). The rest of the table is the plan.

- `ui.state` returns `{"ui": <shell state>, "reader": <view.state, or null with no score>}`.
- `ui.set` paths today: `theme` (`"light"|"dark"|"system"`), `fullScreen`, `chrome`, `keepAwake` (bools).
- `ui.key` and `ui.pointer` inject real input events into the next frame, so they take the same
  path as a keyboard, pedal or finger, and answer with `ui.state` after that frame.
  `modifiers` is a list of `ctrl`, `alt`, `shift`, `cmd`. Pointer coordinates are window points.
  A pedal is a key: `ui.key {"key":"PageDown"}`. Turn keys pass the pedal's bounce filter, so
  repeats of the same key less than 80 ms apart are dropped: to turn several pages quickly, use
  `command view.turn.next` (or `view.page.goto`) instead.
- `ui.screenshot` returns `{"width","height","png"}`: the window's real pixels, captured even
  when the window is minimised. It waits (up to 15 s) until every visible page has finished
  rendering, so it never shows a placeholder or a blurry stand-in. Without a window, `view.snapshot` (an
  engine command) composes the reader on the CPU instead.

| Method | Params | Result |
|---|---|---|
| `command` | `{"id":"view.turn.next","params":{}}` | the command's JSON result |
| `command_batch` | `{"steps":[{"id":…,"params":…}],"stop_on_error":true}` | per-step results |
| `command_list` | `{"filter":"annotate."}` | ids, labels, params docs, enabled state |
| `ui.state` | `{}` | the serde UI state (`ui-egui/src/state.rs`): open reader, page, layout, tool, panels |
| `ui.set` | `{"path":"theme","value":"dark"}` | new state |
| `ui.pointer` | `{"phase":"down|move|up","x":…,"y":…,"button":"primary|secondary"}` (`button` defaults to primary; secondary right-clicks, opening context menus; pen `kind`, `pressure`, `tilt` later) | state after the frame |
| `ui.key` | `{"key":"ArrowRight","modifiers":[]}` | state after the frame |
| `ui.pedal` | `{"profile":"default","button":"right"}` | ok (simulates a mapped pedal) |
| `ui.midi` | `{"bytes":[176,64,127]}` | ok (simulates incoming MIDI) |
| `ui.screenshot` | `{}` | base64 PNG of the window |
| `ui.dialog.set` / `ui.dialog.confirm` | `{"field":…,"value":…}` | dialog state |
| `jobs.list` / `jobs.cancel` | `{}` / `{"job":id}` | background jobs (render, import, OCR, sync) |
| `prefs.get` / `prefs.set` | `{"key":"turn.halfPage.split"}` | one preference at a time |

## Engine commands

All feature-map commands are available through `command`. Their params docs follow the same
mini-schema as PhotoCraft's (`{"name":type=default}`), for example:

```text
view.layout.set        {"mode":"single|twoPage|scroll"="single"}
view.turn.next         {"half":bool?}
annotate.stroke        {"page":int,"layer":id?,"tool":"pen|highlighter|whiteout"="pen","colour":"#rrggbbaa"?,"width":pt=1.5,"points":[[x,y,pressure?,t?]]}
list.entry.add         {"list":id,"score":id,"pageRange":[int,int]?,"tempo":bpm?}
score.export.pdf       {"score":id,"layers":[id]?,"flatten":bool=true,"out":path}
```

Commands that touch the filesystem take paths only inside the configured workspace roots
(below); anything else is refused at the automation boundary.

## Transport limits

- Request line ≤ 8 MiB, batch ≤ 1,000 steps, ≤ 8 concurrent connections, per-command timeout.
- Screenshots and exports larger than the limit return an error with the size.

## Security

- Read and write workspace roots are configured separately (`--control-read-root`,
  `--control-write-root`); defaults are the library directory only.
- Account, sync-credential and network-enabling preferences cannot be changed over the channel.
- See [the automation security model](../book/src/automation/security-model.md).

## Revision history

| Date | Change | Summary |
|---|---|---|
| 2026-11-08 | minor | `ui.pointer` takes `button` (`secondary` right-clicks) |
| 2026-10-31 | minor | M3: command, command_batch, command_list, ui.state, ui.set, ui.key, ui.pointer, ui.screenshot live; `--control-port`, `--control-file`, auth timeout |
| 2026-10-28 | major | First version, adapted from PhotoCraft's control protocol |
