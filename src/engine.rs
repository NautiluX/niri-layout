use crate::{
    config::{Config, Profile},
    ipc::{Client, Snapshot},
};
use anyhow::{Context, Result, ensure};
use niri_ipc::{
    Action, ConfiguredPosition, OutputAction, OutputConfigChanged, PositionToSet, Request,
    Response, SizeChange, WorkspaceReferenceArg as Ref,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "operation", rename_all = "kebab-case")]
pub enum Task {
    OutputPosition {
        output: String,
        x: i32,
        y: i32,
    },
    WorkspaceOutput {
        workspace: u64,
        output: String,
    },
    WorkspaceOrder {
        workspace: u64,
        index: usize,
    },
    WindowWorkspace {
        window: u64,
        workspace: u64,
    },
    SeparateColumn {
        window: u64,
    },
    ColumnOrder {
        window: u64,
        index: usize,
    },
    Width {
        window: u64,
        percent: f64,
        pixels: f64,
    },
}
impl Task {
    pub fn needed(&self, s: &Snapshot) -> bool {
        match self {
            Self::OutputPosition { output, x, y } => s
                .outputs
                .get(output)
                .and_then(|o| o.logical)
                .is_some_and(|l| l.x != *x || l.y != *y),
            Self::WorkspaceOutput { workspace, output } => s
                .workspaces
                .iter()
                .any(|w| w.id == *workspace && w.output.as_ref() != Some(output)),
            Self::WorkspaceOrder { workspace, index } => s
                .workspaces
                .iter()
                .any(|w| w.id == *workspace && usize::from(w.idx) != *index),
            Self::WindowWorkspace { window, workspace } => s
                .windows
                .iter()
                .any(|w| w.id == *window && w.workspace_id != Some(*workspace)),
            Self::SeparateColumn { window } => {
                s.windows.iter().find(|w| w.id == *window).is_some_and(|w| {
                    w.layout.pos_in_scrolling_layout.is_some_and(|(col, _)| {
                        s.windows.iter().any(|other| {
                            other.id != w.id
                                && other.workspace_id == w.workspace_id
                                && other
                                    .layout
                                    .pos_in_scrolling_layout
                                    .is_some_and(|(c, _)| c == col)
                        })
                    })
                })
            }
            Self::ColumnOrder { window, index } => {
                s.windows.iter().find(|w| w.id == *window).is_some_and(|w| {
                    w.layout
                        .pos_in_scrolling_layout
                        .is_some_and(|(c, _)| c != *index)
                })
            }
            Self::Width { window, pixels, .. } => s
                .windows
                .iter()
                .find(|w| w.id == *window)
                .is_some_and(|w| (w.layout.tile_size.0 - pixels).abs() > 2.0),
        }
    }
    fn focus_window(&self) -> Option<u64> {
        match self {
            Self::SeparateColumn { window }
            | Self::ColumnOrder { window, .. }
            | Self::Width { window, .. } => Some(*window),
            _ => None,
        }
    }
    fn action(&self) -> Action {
        match self {
            Self::OutputPosition { .. } => unreachable!("output operations use Request::Output"),
            Self::WorkspaceOutput { workspace, output } => Action::MoveWorkspaceToMonitor {
                output: output.clone(),
                reference: Some(Ref::Id(*workspace)),
            },
            Self::WorkspaceOrder { workspace, index } => Action::MoveWorkspaceToIndex {
                index: *index,
                reference: Some(Ref::Id(*workspace)),
            },
            Self::WindowWorkspace { window, workspace } => Action::MoveWindowToWorkspace {
                window_id: Some(*window),
                reference: Ref::Id(*workspace),
                focus: false,
            },
            Self::SeparateColumn { .. } => Action::ExpelWindowFromColumn {},
            Self::ColumnOrder { index, .. } => Action::MoveColumnToIndex { index: *index },
            Self::Width { percent, .. } => Action::SetColumnWidth {
                change: SizeChange::SetProportion(*percent),
            },
        }
    }
}
#[derive(Clone, Copy)]
pub enum Scope<'a> {
    All,
    New(&'a BTreeSet<u64>),
    OrderOnly,
}

pub fn resolve_outputs(
    s: &Snapshot,
    mappings: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for (role, selector) in mappings {
        let first_regex = selector
            .strip_prefix("first-regex:")
            .map(regex::Regex::new)
            .transpose()?;
        let mut matches: Vec<_> = s
            .outputs
            .values()
            .filter(|o| o.logical.is_some())
            .filter(|o| {
                if selector == "first-external" {
                    return !o.name.starts_with("eDP") && !o.name.starts_with("LVDS");
                }
                if selector == "first-output" {
                    return true;
                }
                let description = format!(
                    "{} {} {}",
                    o.make,
                    o.model,
                    o.serial.as_deref().unwrap_or("Unknown")
                );
                if let Some(re) = &first_regex {
                    return re.is_match(&o.name) || re.is_match(&description);
                }
                o.name == *selector || description.eq_ignore_ascii_case(selector)
            })
            .collect();
        matches.sort_by_key(|o| &o.name);
        let choose_first =
            first_regex.is_some() || selector == "first-output" || selector == "first-external";
        ensure!(
            !matches.is_empty() && (choose_first || matches.len() == 1),
            "output role {role}: expected {} active output matching {selector:?}, found {}",
            if choose_first { "at least one" } else { "one" },
            matches.len()
        );
        result.insert(role.clone(), matches[0].name.clone());
    }
    Ok(result)
}

pub fn tasks(
    config: &Config,
    profile: &Profile,
    mappings: &BTreeMap<String, String>,
    s: &Snapshot,
    scope: Scope<'_>,
) -> Result<(Vec<Task>, BTreeSet<u64>)> {
    let outputs = resolve_outputs(s, mappings)?;
    let mut moves = vec![];
    if matches!(scope, Scope::All) {
        let mut placed = BTreeSet::new();
        for rule in &config.placements {
            let anchors: Vec<_> = s
                .outputs
                .values()
                .filter(|o| o.logical.is_some() && rule.anchor.is_match(&o.name))
                .collect();
            ensure!(anchors.len() <= 1, "ambiguous output placement anchor");
            for output in s.outputs.values().filter(|o| {
                o.logical.is_some()
                    && rule.output.is_match(&format!(
                        "{} {} {}",
                        o.make,
                        o.model,
                        o.serial.as_deref().unwrap_or("Unknown")
                    ))
            }) {
                ensure!(
                    placed.insert(output.name.clone()),
                    "overlapping placement rules for {}",
                    output.name
                );
                let (x, y) = if let Some(anchor) = anchors.first() {
                    ensure!(
                        anchor.name != output.name,
                        "output cannot be its own anchor"
                    );
                    let a = anchor.logical.unwrap();
                    let o = output.logical.unwrap();
                    let x = if rule.side == "left" {
                        i64::from(a.x) - i64::from(o.width)
                    } else {
                        i64::from(a.x) + i64::from(a.width)
                    };
                    (
                        i32::try_from(x)?,
                        a.y.checked_add(rule.offset_y)
                            .context("output y overflow")?,
                    )
                } else {
                    (0, 0)
                };
                moves.push(Task::OutputPosition {
                    output: output.name.clone(),
                    x,
                    y,
                });
            }
        }
    }
    let mut orders = vec![];
    let mut places = vec![];
    let mut splits = vec![];
    let mut columns = vec![];
    let mut sizes = vec![];
    let mut claimed = BTreeSet::new();
    let mut slots = BTreeSet::new();
    for w in &profile.workspaces {
        let out = outputs
            .get(&w.output)
            .with_context(|| format!("missing output mapping for {}", w.output))?;
        ensure!(
            slots.insert((out, w.order)),
            "multiple workspaces request index {} on {out}",
            w.order
        );
        let workspace = s
            .workspaces
            .iter()
            .find(|ws| ws.name.as_ref() == Some(&w.name))
            .with_context(|| format!("declare workspace {:?} in niri config first", w.name))?;
        if matches!(scope, Scope::All) {
            moves.push(Task::WorkspaceOutput {
                workspace: workspace.id,
                output: out.clone(),
            });
        }
        if matches!(scope, Scope::All | Scope::OrderOnly) {
            orders.push(Task::WorkspaceOrder {
                workspace: workspace.id,
                index: w.order,
            });
        }
        let logical = s.outputs[out].logical.context("output is disabled")?;
        let mut index = 0;
        for c in &w.columns {
            let re = &config.apps[&c.app];
            let mut windows: Vec<_> = s
                .windows
                .iter()
                .filter(|win| {
                    !win.is_floating && win.app_id.as_deref().is_some_and(|id| re.is_match(id))
                })
                .collect();
            windows.sort_by_key(|w| w.id);
            for window in windows.into_iter().skip(c.skip).take(c.limit) {
                ensure!(
                    claimed.insert(window.id),
                    "window {} matches more than one column rule",
                    window.id
                );
                index += 1;
                if matches!(scope, Scope::OrderOnly)
                    || matches!(scope,Scope::New(ids) if !ids.contains(&window.id))
                {
                    continue;
                }
                places.push(Task::WindowWorkspace {
                    window: window.id,
                    workspace: workspace.id,
                });
                splits.push(Task::SeparateColumn { window: window.id });
                columns.push(Task::ColumnOrder {
                    window: window.id,
                    index,
                });
                let pixels = (f64::from(logical.width) - config.horizontal_inset - config.gaps)
                    * c.width
                    / 100.0
                    - config.gaps;
                ensure!(pixels > 0.0, "insets exceed output width");
                sizes.push(Task::Width {
                    window: window.id,
                    percent: c.width,
                    pixels,
                });
            }
        }
    }
    orders.sort_by_key(|t| match t {
        Task::WorkspaceOrder { index, .. } => *index,
        _ => 0,
    });
    moves.extend(orders);
    moves.extend(places);
    moves.extend(splits);
    moves.extend(columns);
    moves.extend(sizes);
    Ok((moves, claimed))
}

// A preview includes downstream tasks whose prerequisites will alter their state.
pub fn preview(tasks: &[Task], state: &Snapshot) -> Vec<Task> {
    let structural = tasks
        .iter()
        .any(|t| !matches!(t, Task::Width { .. }) && t.needed(state));
    tasks
        .iter()
        .filter(|t| {
            t.needed(state)
                || (structural
                    && matches!(
                        t,
                        Task::ColumnOrder { .. }
                            | Task::WorkspaceOrder { .. }
                            | Task::SeparateColumn { .. }
                            | Task::Width { .. }
                    ))
        })
        .cloned()
        .collect()
}

pub fn apply(client: &mut Client, tasks: &[Task]) -> Result<usize> {
    let initial = client.snapshot()?;
    let original_window = initial.windows.iter().find(|w| w.is_focused).map(|w| w.id);
    let original_workspace = initial
        .workspaces
        .iter()
        .find(|w| w.is_focused)
        .map(|w| w.id);
    let mut focused_by_us = None;
    let mut count = 0;
    let mut state = initial;
    let result = (|| -> Result<()> {
        for task in tasks {
            if !task.needed(&state) {
                continue;
            }
            if let Some(id) = task.focus_window() {
                // Floating dialogs and windows closed since planning are never manipulated.
                if !state.windows.iter().any(|w| w.id == id && !w.is_floating) {
                    continue;
                }
                if let Some(last) = focused_by_us {
                    ensure!(
                        state.windows.iter().any(|w| w.id == last && w.is_focused),
                        "focus changed while applying; stopped to preserve your focus"
                    );
                }
                ensure!(
                    original_window.is_some(),
                    "a panel or empty workspace has focus; retry after focusing a window"
                );
                if !state.windows.iter().any(|w| w.id == id && w.is_focused) {
                    client.action(Action::FocusWindow { id })?;
                }
                focused_by_us = Some(id);
            }
            if let Task::OutputPosition { output, x, y } = task {
                let response = client.request(Request::Output {
                    output: output.clone(),
                    action: OutputAction::Position {
                        position: PositionToSet::Specific(ConfiguredPosition { x: *x, y: *y }),
                    },
                })?;
                ensure!(
                    matches!(
                        response,
                        Response::OutputConfigChanged(OutputConfigChanged::Applied)
                    ),
                    "output disappeared during placement"
                );
            } else {
                client
                    .action(task.action())
                    .with_context(|| format!("applying {task:?}"))?;
            }
            count += 1;
            // Refresh after mutations, not once for every already-satisfied task.
            state = client.snapshot()?;
        }
        Ok(())
    })();
    // Restore even after a rejected operation, but never override a user's new focus.
    if let Some(last) = focused_by_us {
        let s = client.snapshot()?;
        if s.windows.iter().any(|w| w.id == last && w.is_focused) {
            if let Some(id) = original_window.filter(|id| s.windows.iter().any(|w| w.id == *id)) {
                client.action(Action::FocusWindow { id })?;
            } else if let Some(id) = original_workspace {
                client.action(Action::FocusWorkspace {
                    reference: Ref::Id(id),
                })?;
            }
        }
    }
    result?;
    Ok(count)
}
