# Worked examples

All app IDs and full display identities here are fictional. Connector names,
scales and dimensions are illustrative. Substitute observations from your own
session; never apply a guessed monitor mode.

The files fit together:

- [config.kdl](config.kdl): companion layouts, four profiles.
- [niri-workspaces.kdl](niri-workspaces.kdl): required workspace declarations.
- [kanshi.conf](kanshi.conf): matching monitor sets and profile hooks.

## Portable: one screen

Notes, web, shell, then an empty presentation workspace, all on the `laptop` role.
Each matched app window gets a full-width column. Workspace ordering is enforced
on workspace events. Example preview:

```sh
niri-layout --config examples/config.kdl plan portable --output laptop=eDP-1
```

## Docked: laptop and external

Notes remain on the laptop. Browsers move to the external display at half width,
and terminals to another external workspace at full width. Moving a workspace
reorders the remaining laptop workspaces, so the inherited presentation workspace
is explicitly changed to index 2.

```sh
niri-layout --config examples/config.kdl plan docked \
    --output laptop=eDP-1 --output external=DP-1
```

## Presenting: one browser on the projector

Only the first browser window by ID goes to the presentation workspace on the
external output. Remaining browser windows stay on the laptop using `skip=1`.
The two ranges are disjoint. The user must verify which window is selected: these
rules cannot distinguish work/private accounts. Closing a browser can change the
selected window on the next profile application.

```sh
niri-layout --config examples/config.kdl plan presenting \
    --output laptop=eDP-1 --output external=HDMI-A-1
```

## External-only: reuse roles

The inherited portable layout is reused, with `laptop` mapped to the only desired
external connector. Role names are labels; this does not require a laptop display.

```sh
niri-layout --config examples/config.kdl plan external-only --output laptop=DP-1
```

## Monitor position and matching

The Kanshi examples assume a 1920-logical-pixel laptop. Set the external x position
to the actual logical laptop width for a flush right-hand placement. For adaptive
left/right placement, use the optional `place-output` rule documented in the
reference and omit a conflicting Kanshi position for that output.

Exact two-screen profiles are above the generic fallback. Kanshi 1.9's
`...output` allows additional matching heads. The generic profile uses `output
"*"` for one required extra output and `...output "* * *"` for remaining
full-description matches; repeating the identical criteria twice is rejected by
Kanshi. Add explicit profiles if multiple external monitors require particular
roles: `first-external` alone intentionally chooses by connector name, not priority.
