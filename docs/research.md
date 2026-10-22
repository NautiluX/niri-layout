# Existing tools checked — 2026-10-22

Search included the [official ecosystem list](https://github.com/niri-wm/awesome-niri)
and project documentation. This cannot prove that no equivalent exists; no documented
match for the complete requested workflow was found.

| Project | Relevant capability | Difference from this project |
| --- | --- | --- |
| [Piri](https://github.com/Asthestarsfalll/piri) | Rust event-driven plugin host, window rules, workspace widths and weighted window ordering | Closest overlap. Reviewed docs do not describe monitor-selected inherited profiles binding workspace output/order and app column widths together. Could be an upstream destination for parts of this work. |
| [nirilayout](https://github.com/calico32/nirilayout) | Switch output configurations | Handles the display side, not app/workspace layout policies. Also a similar name: reconsider `niri-layout` before publication. |
| [nirinit](https://github.com/amaanq/nirinit) | Save/restore sessions, launch apps, preserve window/workspace placement | Restores saved sessions rather than selecting declarative app arrangements per dock. |
| [oniri](https://github.com/Antiz96/oniri) | Automatic maximization and optional tiling | Narrow automatic sizing behavior, no equivalent profile model. |
| [miri](https://github.com/MintyDoggo/miri) | Alternative workspace tiling layouts | Different scope from placement across monitor profiles. |
| [niri-companion](https://github.com/dybdeskarphet/niri-companion) | Layered config generation, task sessions, IPC utilities | Related toolkit; no documented complete equivalent found. |
| [niri-dynamic-workspaces](https://docs.rs/crate/niri-dynamic-workspaces/latest) | Workspace selection and program placement | Different workflow from dock-selected workspace layouts. |
| [monitoradlo](https://github.com/kumekay/monitoradlo) | GUI editor for Kanshi profiles | Useful complementary display configuration UI. |

Niri itself [recommends Kanshi for connected-monitor-dependent configuration](https://niri-wm.github.io/niri/FAQ.html#how-do-i-change-output-configuration-based-on-connected-monitors).
Kanshi remains responsible for monitor detection and mode/scale configuration.
The companion additionally supports optional relative positions computed from
the actual logical output widths after scale is applied.

Implementation API: [niri-ipc 26.4.0](https://docs.rs/niri-ipc/26.4.0/niri_ipc/).
This version exposes window/workspace events but no OutputsChanged event. Kanshi's
profile exec hook initiates topology changes; niri's events handle new windows.

Decision: build a focused first implementation. Extending Piri remains an option,
but this prototype doesn't require adopting its broader plugin host or policy model.
