# Getting started

## 1. Inspect your session

Run these inside niri:

```sh
niri msg --json outputs
niri msg --json windows
niri msg --json workspaces
```

Record active output connectors, make/model/serial, logical dimensions and scale;
app IDs; and named workspaces. Window output contains titles and process IDs:
share only the fields needed to describe a layout, not an unredacted desktop dump.
Do not infer browser account/profile identity from a generic browser app ID.

The companion targets niri 26.04. The wildcard integration example uses Kanshi 1.9.
A newer installed version is not proof of compatibility; validate against your session.

## 2. Build and install the executable

From the checkout:

```sh
cargo build --release --locked
install -Dm755 target/release/niri-layout "$HOME/.local/bin/niri-layout"
install -Dm755 scripts/start-session "$HOME/.local/bin/niri-layout-session"
```

Install Kanshi separately if you want automatic monitor switching. Ensure
`~/.local/bin` is on your session PATH. The provided startup wrapper adds it to its
own PATH so Kanshi's exec hooks can find the companion.

## 3. Write the three configurations

Start from [the annotated examples](../examples/README.md). Review and adapt them;
do not copy over an existing configuration without merging it.

| File | Purpose |
| --- | --- |
| `$XDG_CONFIG_HOME/niri/config.kdl` | Declare named workspaces and start the session wrapper |
| `$XDG_CONFIG_HOME/niri-layout/config.kdl` | App selectors, workspace/column layouts and relative placement |
| `$XDG_CONFIG_HOME/kanshi/config` | Select a layout profile for each connected monitor set; set modes/scales |

`XDG_CONFIG_HOME` defaults to `~/.config`. niri can itself use a custom config path;
merge declarations into the file your session actually loads.

Every workspace referenced in any layout profile must already exist as a named
workspace in niri, even if it is currently empty. The companion does not create or
rename workspaces. Its config does not launch applications.

## 4. Validate and preview

```sh
niri-layout --config /path/to/layout.kdl check
niri validate -c /path/to/merged-niri-config.kdl
niri-layout --config /path/to/layout.kdl plan docked \
    --output laptop=eDP-1 --output external=DP-1
```

Use observed connectors, not these example values. `check` parses all profiles,
resolves inheritance, and validates static constraints without contacting niri.
`plan` additionally checks connected outputs, existing named workspaces and live
window matches. It does not move, resize or focus anything.

A preview is a conservative task list; structural changes can make some listed
operations unnecessary. An empty array means the current state satisfies the
layout within the width comparison tolerance. `check` cannot validate Kanshi
syntax or the actual connected hardware. Kanshi has no `--check` option in the
version used here; verify its configuration and startup log during activation.

## 5. Try one profile

Stop any competing automatic window-layout manager before applying changes.
To test without a daemon:

```sh
niri-layout --config /path/to/layout.kdl apply docked \
    --output laptop=eDP-1 --output external=DP-1 --once
```

This changes the desktop. Use it only after reviewing the preview. One-shot mode
cannot run concurrently with the companion daemon and does not handle future
windows. To test the daemon instead, start it in one terminal:

```sh
niri-layout --config /path/to/layout.kdl daemon
```

Then, in another:

```sh
niri-layout apply docked --output laptop=eDP-1 --output external=DP-1
niri-layout status
```

**The daemon owns its config path.** `--config` on an `apply` command sent to an
already running daemon does not override that path. Start the daemon with the
intended file. Each apply rereads that file. For the bundled startup wrapper, use
the default XDG path; the wrapper takes no configuration arguments.

## 6. Enable automatic switching

Back up the files being edited and note how to restart the old layout manager.
Merge the adapted Kanshi example into its config. Remove or disable only competing
layout startup commands; preserve unrelated app startup and desktop services.

Add this to niri after placing the wrapper on the session PATH:

```kdl
spawn-at-startup "niri-layout-session"
```

If niri cannot find it on PATH, use its actual absolute installed path. Do not use
`~` or `$HOME` inside that KDL string and assume a shell will expand it.

The wrapper starts the daemon, waits for its control socket, starts Kanshi, and
stops both if either exits. It logs to
`$XDG_STATE_HOME/niri-layout/session.log` (default `~/.local/state/...`). It does
not automatically restart failed processes. Start the wrapper once per session;
launch it manually for the current session, because reloading niri config does
not rerun `spawn-at-startup` entries.

Check `niri-layout status` and the log. Exercise each monitor combination when
available, including unplugging the external monitor. An optional shortcut is:

```kdl
binds {
    Mod+Alt+A { spawn "niri-layout" "reapply"; }
}
```

Merge the binding into your existing `binds` block. `reapply` explicitly resets
the selected apps to the requested layout, including manually moved windows.

## Rollback

Stop the wrapper and its companion/Kanshi children, restore the backed-up niri
and Kanshi configs, reload niri, and restart the previous layout manager if used.
Stopping the new processes alone does not undo already applied workspace moves.
Avoid running two output managers or two layout managers simultaneously.
