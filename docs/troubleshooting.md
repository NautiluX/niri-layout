# Troubleshooting and limitations

## A command cannot connect

`check` needs only a config. Other commands need a running niri session and
`NIRI_SOCKET`, or an explicit `--socket PATH`. A daemon must also be running for
`status`, `reapply`, and `apply` without `--once`. Start one daemon per niri session.
A lost event stream terminates it; neither the daemon nor the provided wrapper
automatically reconnects/restarts. After restarting, select a profile again.

## The config passed check, but apply fails

`check` covers syntax and static relationships. Live planning also requires all
output roles to resolve, all workspace names to exist, and all live window matches
to be disjoint. Use `plan` with exactly the intended output mappings. A failed
apply can leave earlier operations applied: there is no atomic transaction or
automatic rollback. Fix the cause, preview again, then reapply.

If a full daemon switch fails after mutation starts, its previous selection is
cleared rather than reverting the desktop through the old rules. Look at `status`.

## Edits do not take effect

The daemon rereads its own config on apply/reapply. It does not watch the config
file. `--config` passed to an apply client does not redirect the daemon. Stop and
restart the daemon with the correct config path if necessary. The bundled startup
wrapper uses the default XDG paths and does not forward command-line arguments.

## An app is missing, selected incorrectly, or moved unexpectedly

Inspect the real app ID. Matching is case-sensitive unless the regex uses `(?i)`.
Floating windows do not participate. `column` selects only one matching window;
`columns` selects all unless limited. Selection is across the whole session, not
just the destination workspace. A workspace move also moves unmatched apps in it.

Window IDs are not account/profile labels. For indistinguishable browser windows,
verify the choice manually. Extra windows beyond a rule's limit are not claimed;
a later explicit apply can select a different set if earlier windows closed.

Already managed matching windows are normally left alone between full profile
applications, including after manual moves. A full apply/reapply intentionally
resets them. New matching windows can shift existing columns as they are inserted.

## Focus changes or the status says it is waiting

Some niri actions require focusing a window. The executor restores the original
focused window where possible and stops if it detects focus changing during its
work. This is best effort: user interaction, windows closing and monitor removal
can race IPC requests. It does not promise preservation of every inactive
workspace's scroll position or focus history.

If windows exist but a panel/empty workspace owns focus during a daemon profile
selection, application is deferred until a window gains focus. New windows that
could not be arranged because of focus can be retried on a subsequent window-focus
event. One-shot mode can report a focus error instead. Avoid rapid interaction
while testing a full rearrangement.

## Repeated width/order changes or visible shuffle

Align global gaps/insets with niri. The width estimate has a two-logical-pixel
tolerance and cannot model every panel, workspace override or app size constraint.
Workspace indices should be contiguous and realizable. A structural preview
includes dependent tasks that may ultimately be skipped; it is not a mutation log.

The companion avoids unnecessary actions, but operations are sequential. Moving
workspaces, splitting a shared column, resizing and focus changes can be visible.
There is no guarantee of an instantaneous transition or measured speedup.

## Wrong monitor profile or physical position

Check Kanshi's matching criteria, ordering, wildcard coverage and exec hook in the
session log (`~/.local/state/niri-layout/session.log` by default). `first-external`
uses connector order, not a preferred monitor list. Add specific profiles/roles
when multiple monitors require distinct placement.

Kanshi positions use logical coordinates after scale/rotation. Relative placement
rules use an initial snapshot; avoid chains of moving anchors. With no anchor,
they place outputs at `(0,0)`. Do not combine a relative rule with a conflicting
absolute Kanshi position. The companion does not resolve overlapping outputs.

Do not force an unavailable mode to fix a cable/dock issue. Inspect the modes
reported by the compositor and test the physical connection separately.
