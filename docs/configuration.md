# Configuration reference

This describes the implemented grammar in `src/config.rs`, output matching and
planning in `src/engine.rs`, and commands in `src/main.rs`. These are the source of
truth when changing behavior. Configuration is KDL v2: booleans are `#true` and
`#false`. Quoted strings use KDL escaping; for regex dots, `[.]` avoids backslash
escaping. Regexes use Rust's `regex` crate, so look-around/backreferences are not
supported. Regex matches are unanchored unless you supply `^` and `$`.

Unknown nodes/properties, duplicate properties and typed values are rejected.
There is no `include` directive or environment-variable interpolation in this
configuration language. At least one profile must exist.

## Global nodes

| Node | Arguments/properties | Default and constraints |
| --- | --- | --- |
| `gaps NUMBER` | One number, no properties/children | `2`; finite and nonnegative; at most once |
| `horizontal-inset NUMBER` | One number, no properties/children | `0`; finite and nonnegative; at most once |
| `app "ALIAS" app-id="REGEX"` | One alias and required regex; no children | Unique alias; no title, PID or browser-profile matching |
| `profile "NAME"` | Optional `extends="PARENT"`, `enforce-order=#true` or `#false`; workspace children | Unique name; see inheritance below |
| `place-output "REGEX" anchor="REGEX" side="left" offset-y=0` | Required output regex, anchor and side; optional offset-y; no children | Optional repeatable rule; details below |

`gaps` and `horizontal-inset` describe the existing niri layout; they do not change
niri settings. Insets are the combined left/right space reserved outside the
working area. Width comparison estimates:

```text
(output logical width - horizontal-inset - gaps) * percent / 100 - gaps
```

The result must be positive. The executor tolerates two logical pixels of
rounding difference. Per-workspace gap overrides, client minimum widths and unusual
panels can make this estimate inaccurate. Applying a width sends niri a percentage,
not the estimated pixel count. `100%` is full column width, not fullscreen.

## Profiles and inheritance

```kdl
profile "base" enforce-order=#true {
    workspace "web" output="main" order=1 {
        columns "browser" width="100%"
    }
}
profile "docked" extends="base" enforce-order=#false {
    workspace "web" output="external" order=1 {
        columns "browser" width="50%"
    }
}
```

A child **replaces the entire workspace definition** with the same name. Other
workspaces are inherited. An omitted `enforce-order` inherits the parent value;
without a parent it behaves as false. Parents can be defined later in the file.
Missing parents and inheritance cycles fail validation. There is no workspace
removal directive. An empty workspace declaration replaces its columns with none.

All profile applications arrange workspace order once. `enforce-order=#true` also
rechecks order on workspace events. It does not continuously enforce window
placement or column widths. Explicit `apply`/`reapply` reapplies the full profile;
subsequent new-window events operate only on unmanaged matching windows.

## Workspaces

```kdl
workspace "web" output="main" order=1 {
    column "chat" width="50%"
    columns "browser" width="50%" limit=2
}
```

| Field | Meaning |
| --- | --- |
| Name | Required nonempty name, predeclared in niri; unique within the profile |
| `output` | Required nonempty role resolved by `--output ROLE=SELECTOR` |
| `order` | Required integer `1..255`; absolute workspace index on the destination output |
| Children | Zero or more `column` / `columns` nodes, in desired column order |

Two resolved workspaces cannot request the same index on the same output.
Uniqueness within a role is checked statically; two roles mapping to the same
physical output are checked at planning time. Contiguous indices starting at one
are recommended: niri can clamp out-of-range moves, so a gap in requested indices
may not be realizable with the currently existing workspaces.

Moving a workspace moves its contents, including unmatched apps on it. The
companion does not close windows. Matching tiled windows elsewhere in the session
can also be moved into the configured workspace; matching is not scoped to their
current workspace. Floating windows are excluded from app selection.

## Columns and selection

| Node/property | Semantics |
| --- | --- |
| `column "APP"` | Select one window matching the named app alias |
| `columns "APP"` | Select all matching windows, each in its own column |
| `width="PERCENT%"` | Required finite percentage greater than 0 and at most 100; decimals allowed |
| `skip=N` | Optional nonnegative integer; default 0 |
| `limit=N` | Optional positive integer on `columns` only; default unlimited |

Candidates for each rule are all nonfloating windows matching the alias regex,
sorted by window ID. `skip` is applied first, then `limit`. Rules do **not** consume
windows from a shared pool: disjoint ranges must be specified explicitly. Selecting
the same live window through multiple rules fails before application. A rule with
no matching windows is allowed. Unknown app aliases fail static validation.

Window IDs are deterministic identifiers for a current session, **not** a supported
creation-time or browser-account API. `skip=1` means the second matching window by
ID, not a particular browser profile. Selection can change after windows close.

Selected windows occupy separate columns at the beginning of each workspace.
Unmatched windows remain, but can shift as selected columns move. A shared column
is split when needed. There is no tabbed/stacked-column syntax; `columns` does not
mean several windows stacked into one column.

## Output selectors

Pass one mapping per role; duplicate role arguments are rejected. Extra mappings
are still resolved even if the profile does not use them, so omit unnecessary ones.

| Selector | Behavior |
| --- | --- |
| `DP-1` | Exact connector name (case-sensitive) |
| `Example Displays DeskPanel DEMO-001` | Full `make model serial` description, case-insensitive; exactly one active match required |
| `first-output` | First active output by lexicographic connector order |
| `first-external` | First active output whose connector does not start with `eDP` or `LVDS` |
| `first-regex:REGEX` | First active output whose connector **or** full description matches the regex |

Missing serials are represented by `Unknown`. Disabled outputs do not match.
Exact selectors reject ambiguity; `first-*` intentionally choose the first match.
This is connector ordering, not the focused output or preferred physical monitor.
Role names are arbitrary labels, not special built-in laptop/external behavior.
Kanshi uses its own glob criteria; these selectors are not Kanshi syntax.

## Optional relative placement

```kdl
place-output "^Example Displays DeskPanel DEMO-001$" anchor="^eDP-1$" side="left" offset-y=-100
```

The positional regex matches active outputs' **full descriptions only**. The
anchor regex matches active **connector names only**. `side` is exactly `left`
or `right`; `offset-y` defaults to 0 and must fit a signed 32-bit integer.

- Left: output x = anchor x minus the selected output's logical width.
- Right: output x = anchor x plus the anchor's logical width.
- Both: output y = anchor y plus offset-y.
- No matching anchor: matching outputs are positioned at `(0, 0)`.
- Multiple anchors, self-anchoring, overlapping placement rules and coordinate
  overflow are rejected before application. A rule matching no output is a no-op.

These rules are global and run on full profile applications only. Calculations
use the initial snapshot; do not chain anchors that are also being repositioned.
The planner does not resolve output overlaps or choose among multiple monitors
matching a rule. Use precise patterns. Let Kanshi apply scale first, and omit
conflicting fixed positions for these outputs from Kanshi.

## CLI and lifecycle

| Command | Reads config / desktop | Changes desktop |
| --- | --- | --- |
| `check` | Config only; validates every profile | No |
| `plan PROFILE --output ...` | Local config and current niri snapshot | No |
| `daemon` | Own config path; niri events; waits for selection | When a profile is selected or matching windows appear |
| `apply PROFILE --output ...` | Sends selection to daemon; daemon rereads its config | Yes |
| `apply PROFILE --output ... --once` | Own config; no daemon allowed | Yes, once |
| `reapply` | Reads daemon's selection, sends it again | Yes |
| `status` | Daemon's selection and managed-window count | No |

Global options: `--config PATH` defaults to `$XDG_CONFIG_HOME/niri-layout/config.kdl`
or `~/.config/niri-layout/config.kdl`; `--socket PATH` defaults to `$NIRI_SOCKET`.
Only check works without a session socket. The daemon, not the apply client,
determines the config path for daemon-based apply/reapply. There is no config
watcher: edits take effect on the next apply/reapply.

One daemon/one-shot executor is permitted per niri socket. Control socket and lock
files are adjacent to it. Selection is in memory, not restored from disk. A niri
event-stream disconnect terminates the daemon. Restart it and let Kanshi select a
profile again. See the [troubleshooting guide](troubleshooting.md) for focus and
partial-failure behavior.
