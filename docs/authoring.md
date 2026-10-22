# Authoring a layout (including with an LLM)

Read [the reference](configuration.md) and use [the complete examples](../examples/README.md).
Generate the syntax this version implements, not speculative niri/plugin features.

## Gather the minimum inputs

Ask for or inspect:

1. Active outputs: connector, make, model, serial, logical dimensions and scale.
2. Relevant app IDs and existing named workspaces. Exclude titles and PIDs from
   shared snapshots unless separately needed and authorized.
3. Desired monitor sets (portable, docked, presentation, external-only), workspace
   destinations/order, app column order/width, and policy for extra monitors.
4. Whether manual changes should persist until the next profile switch; whether
   workspace order should be continuously enforced.

Do not infer output identities, app IDs, account identities or installed programs.
Use conspicuous fictional placeholders when observations are unavailable and label
what must be replaced. Do not embed a real user's desktop dump or configuration
in public examples, tests or documentation.

## Assign responsibilities

| Requirement | Where to configure it |
| --- | --- |
| Monitor set matching, mode and scale | Kanshi |
| Fixed monitor positions | Kanshi, using logical coordinates |
| Adaptive left/right position from actual logical widths | Optional global `place-output` in niri-layout |
| Named workspace existence | niri config |
| Workspace destination/order and app column width | niri-layout profile |
| Start applications / restore browser tabs | Separate session launcher or app settings |
| Bar, lock screen, input, clipboard, wallpaper | Separate desktop configuration |

A layout profile is not a monitor matcher. Connect the two with one Kanshi `exec`
hook after the display profile applies. Start the companion before that hook can
run. Avoid two managers setting conflicting positions.

## Translate requirements into configuration

1. Define one global `app "ALIAS" app-id="REGEX"` per matching rule. Anchor regexes
   when exact matching is intended. Check for overlaps.
2. Define a base profile with named workspaces, output roles and contiguous orders.
3. Use `column` for one window; `columns` for separate columns for all matches.
   Specify widths explicitly. Full width is `"100%"`, not true fullscreen.
4. Add inherited profiles. Restate all columns inside an overridden workspace;
   workspace bodies are replaced, not merged. Recompute orders on both screens
   when workspaces move. Two roles pointing to one screen still share its indices.
5. For an exception selecting one of several identical app IDs, use disjoint
   `skip`/`limit` ranges and explain the window-ID heuristic. Never call it reliable
   browser-profile selection.
6. Decide exact versus fallback output matching. Exact descriptions reject ambiguity;
   `first-*` selectors deliberately accept it and choose by connector name.
7. Write matching Kanshi profiles with explicit role mappings. Quote an entire
   `ROLE=description with spaces` argument in the exec command. Kanshi's glob
   criteria and niri-layout's regex selectors are different languages.
8. Supply all required niri workspace declarations and one startup entry.

Unsupported features must be explained rather than invented: no nested `app`
nodes in columns, title/PID matching, browser-profile selector, tabbed columns,
launch command, automatic named-workspace creation, per-profile relative placement,
config includes, or profile auto-selection inside the companion.

## Deliverables

Provide these as separately labelled files/fragments:

- `niri-layout/config.kdl` with app aliases and all layout profiles.
- `kanshi/config` with monitor matching and a profile application hook.
- niri workspace declarations and the startup entry, to merge with existing config.
- A short role/profile table, assumptions, exact validation commands and rollback steps.

Keep generated machine-specific configuration outside the source checkout, or
under ignored `local/`. Public examples should retain fictional identities.

## Validation sequence

1. Build the binary and run `check` on the generated file. This covers every profile.
2. Validate the merged niri config with `niri validate -c FILE`.
3. For each available topology, run `plan PROFILE --output ...` and review window
   destinations, column selection, output placements and widths. Check cannot
   prove that a monitor is available or a live window matches only one rule.
4. Respect the user's authorization for applying changes. Stop competing managers,
   then apply once or select the profile through the daemon. Never run `--once`
   while the daemon owns the session lock.
5. Verify `status`, a second read-only plan, and the session log. Test one new window
   and a manual move; check other monitor combinations when physically available.
6. Record what was actually tested. Do not claim unplug/replug behavior is verified
   from static parsing alone. Do not push real configuration to a public repository.

For `plan --config FILE` to predict a daemon apply, the daemon must have been
started with that same file. The apply client's `--config` option does not change
the running daemon's configured path.

## Reusable generation prompt

> Read AGENTS.md, docs/configuration.md, docs/authoring.md and the examples in this
> checkout. Generate layout, Kanshi and niri-workspace fragments for the monitor
> identities and app IDs I supply. Keep all real-machine files outside the public
> source tree. Use only implemented syntax, explain inherited replacements and
> ambiguous window selection, run static validation, and provide read-only plan
> commands for each topology. State missing facts explicitly. Apply only within
> the scope I authorize; do not alter unrelated desktop services or publish files.
