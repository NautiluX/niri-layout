# niri-layout

Declarative workspace and window layouts for [niri](https://github.com/niri-wm/niri),
selected by [Kanshi](https://gitlab.freedesktop.org/emersion/kanshi) monitor profiles.
A standalone Rust companion using niri IPC, not an in-process compositor plugin.

- Move named workspaces to outputs and assign their order.
- Match apps by app ID and give each selected window a separate column.
- Choose column widths per profile, with workspace-level inheritance.
- Place newly opened windows without continuously undoing manual window moves.
- Preview changes before applying them.

Targets niri 26.04 (`niri-ipc` 26.4.0). This is an early implementation: actions are
sequential, not atomic, and width/order changes can briefly change focus.

## Documentation

- [Getting started](docs/getting-started.md): install, inspect your desktop, validate and activate.
- [Configuration reference](docs/configuration.md): every supported node, default and selector.
- [Layout authoring guide](docs/authoring.md): translate requirements into working profiles; suitable for LLMs.
- [Troubleshooting](docs/troubleshooting.md): matching, focus, sizing and lifecycle limitations.
- [Examples](examples/README.md): laptop, docked, presentation and external-only scenarios.
- [Project research](docs/research.md): related tools considered during development.

## Install

Download a prebuilt Linux x86-64 or ARM64 archive from
[GitHub Releases](https://github.com/NautiluX/niri-layout/releases/latest).
Extract it and run `./install.sh` to install under `~/.local` without Rust.
See [release installation](docs/releases.md) for checksum verification, upgrades
and CI publishing instructions. Install niri and Kanshi separately.

## Build from source

Rust 1.93.1 is the tested compiler. Install Rust/Cargo and Kanshi through your normal
package manager, then:

```sh
cargo build --release --locked
cargo test --locked
python3 tests/daemon_smoke.py
./target/release/niri-layout --config examples/config.kdl check
```

Tests use a fake compositor and require local Unix sockets. Python is used only by
the test harness; the companion itself is Rust. The smoke test expects a release
binary at `target/release/niri-layout`.

## A small example

```kdl
app "browser" app-id="^org[.]example[.]Browser$"
profile "portable" {
    workspace "web" output="main" order=1 {
        columns "browser" width="100%"
    }
}
```

The app ID is fictional. Replace it with the actual value from `niri msg --json
windows`, declare `workspace "web"` in niri's config, and save the layout in
`~/.config/niri-layout/config.kdl` (or the corresponding XDG config directory).

```sh
niri-layout check
niri-layout plan portable --output main=eDP-1
```

`plan` is read-only. After reviewing the preview, start `niri-layout daemon` in
one terminal and run `niri-layout apply portable --output main=eDP-1` in another.
For automatic monitor switching, follow the Kanshi integration guide.

## Scope

Kanshi selects monitor profiles and sets modes/scales. niri-layout arranges
workspaces and app windows, with optional relative output placement. Application
startup, session restoration, launchers, bars and screen locking remain separate.

The examples use fictional app IDs and display identities. Keep real desktop
configuration outside this repository, or in the ignored `local/` directory.
No personal deployment files are required to build or use the project.
