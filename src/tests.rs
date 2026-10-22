use crate::{
    config::Config,
    engine::{self, Scope, Task},
    ipc::{Client, Snapshot},
};
use niri_ipc::{Action, Output, Request, Response, Window, Workspace, WorkspaceReferenceArg};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    sync::{Arc, Mutex},
    thread,
};

fn fixture() -> Snapshot {
    let output:Output=serde_json::from_value(json!({"name":"eDP-1","make":"Example","model":"Laptop","serial":null,"physical_size":null,"modes":[],"current_mode":0,"is_custom_mode":false,"vrr_supported":false,"vrr_enabled":false,"logical":{"x":0,"y":0,"width":1920,"height":1200,"scale":1.0,"transform":"Normal"}})).unwrap();
    let workspace:Workspace=serde_json::from_value(json!({"id":10,"idx":1,"name":"notes","output":"eDP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":1})).unwrap();
    let window:Window=serde_json::from_value(json!({"id":1,"title":null,"app_id":"org.example.Notes","pid":null,"workspace_id":10,"is_focused":true,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[1,1],"tile_size":[1916.0,1160.0],"window_size":[1914,1158],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[1.0,1.0]},"focus_timestamp":null})).unwrap();
    Snapshot {
        outputs: [("eDP-1".into(), output)].into_iter().collect(),
        workspaces: vec![workspace],
        windows: vec![window],
    }
}
fn config() -> Config {
    Config::parse("app \"notes\" app-id=\"^org[.]example[.]Notes$\"\nprofile \"laptop\" { workspace \"notes\" output=\"laptop\" order=1 { columns \"notes\" width=\"100%\"; }; }").unwrap()
}
fn maps() -> BTreeMap<String, String> {
    [("laptop".into(), "eDP-1".into())].into_iter().collect()
}
#[test]
fn already_correct_has_no_changes() {
    let c = config();
    let s = fixture();
    let (tasks, _) =
        engine::tasks(&c, &c.profile("laptop").unwrap(), &maps(), &s, Scope::All).unwrap();
    assert!(engine::preview(&tasks, &s).is_empty());
}
#[test]
fn absent_output_or_workspace_rejected_before_applying() {
    let c = config();
    let mut s = fixture();
    assert!(
        engine::tasks(
            &c,
            &c.profile("laptop").unwrap(),
            &BTreeMap::new(),
            &s,
            Scope::All
        )
        .is_err()
    );
    s.workspaces.clear();
    assert!(engine::tasks(&c, &c.profile("laptop").unwrap(), &maps(), &s, Scope::All).is_err());
}
#[test]
fn ambiguous_monitor_description_rejected() {
    let mut s = fixture();
    let mut second = s.outputs["eDP-1"].clone();
    second.name = "DP-1".into();
    s.outputs.insert("DP-1".into(), second);
    let mappings = [("laptop".into(), "Example Laptop Unknown".into())]
        .into_iter()
        .collect();
    assert!(engine::resolve_outputs(&s, &mappings).is_err());
}
#[test]
fn new_window_does_not_reclaim_manually_moved_existing_window() {
    let c = config();
    let mut s = fixture();
    s.windows[0].workspace_id = Some(99);
    let mut new = s.windows[0].clone();
    new.id = 2;
    s.windows.push(new);
    let ids = [2].into_iter().collect();
    let (tasks, _) = engine::tasks(
        &c,
        &c.profile("laptop").unwrap(),
        &maps(),
        &s,
        Scope::New(&ids),
    )
    .unwrap();
    assert!(tasks.iter().all(|t| match t {
        Task::WindowWorkspace { window, .. }
        | Task::SeparateColumn { window }
        | Task::ColumnOrder { window, .. }
        | Task::Width { window, .. } => *window == 2,
        _ => false,
    }));
}
#[test]
fn ignores_floating_and_unmatched_windows() {
    let c = config();
    let mut s = fixture();
    s.windows[0].is_floating = true;
    let mut other = s.windows[0].clone();
    other.id = 2;
    other.is_floating = false;
    other.app_id = Some("unmatched".into());
    s.windows.push(other);
    let (_, claimed) =
        engine::tasks(&c, &c.profile("laptop").unwrap(), &maps(), &s, Scope::All).unwrap();
    assert!(claimed.is_empty());
}
#[test]
fn presentation_selects_only_first_browser_for_external() {
    let c = Config::parse(include_str!("../examples/config.kdl")).unwrap();
    let mut s = fixture();
    for (i, name) in ["web", "shell", "presentation"].into_iter().enumerate() {
        let mut w = s.workspaces[0].clone();
        w.id = 11 + i as u64;
        w.idx = 2 + i as u8;
        w.name = Some(name.into());
        s.workspaces.push(w);
    }
    let mut external = s.outputs["eDP-1"].clone();
    external.name = "HDMI-A-1".into();
    s.outputs.insert(external.name.clone(), external);
    for id in [2, 3] {
        let mut w = s.windows[0].clone();
        w.id = id;
        w.app_id = Some("org.example.Browser".into());
        s.windows.push(w);
    }
    let mut mappings = maps();
    mappings.insert("external".into(), "HDMI-A-1".into());
    let (tasks, _) = engine::tasks(
        &c,
        &c.profile("presenting").unwrap(),
        &mappings,
        &s,
        Scope::All,
    )
    .unwrap();
    assert!(tasks.iter().any(|t| matches!(
        t,
        Task::WindowWorkspace {
            window: 2,
            workspace: 13
        }
    )));
    assert!(tasks.iter().any(|t| matches!(
        t,
        Task::WindowWorkspace {
            window: 3,
            workspace: 11
        }
    )));
}
#[test]
fn overlapping_rules_rejected() {
    let c=Config::parse("app \"a\" app-id=\".*\"\nprofile \"p\" {workspace \"notes\" output=\"laptop\" order=1 { column \"a\" width=\"50%\"; column \"a\" width=\"50%\"; }; }").unwrap();
    assert!(
        engine::tasks(
            &c,
            &c.profile("p").unwrap(),
            &maps(),
            &fixture(),
            Scope::All
        )
        .is_err()
    );
}
#[test]
fn socket_apply_is_idempotent_and_restores_focus() {
    let path = std::env::temp_dir().join(format!("niri-layout-test-{}.sock", std::process::id()));
    let listener = UnixListener::bind(&path).unwrap();
    let mut s = fixture();
    let mut other = s.windows[0].clone();
    other.id = 2;
    other.app_id = Some("other".into());
    other.is_focused = false;
    other.layout.tile_size.0 = 400.0;
    other.layout.pos_in_scrolling_layout = Some((2, 1));
    s.windows.push(other);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let log = calls.clone();
    let server = thread::spawn(move || {
        let (stream, _) = listener.accept().unwrap();
        let mut reader = BufReader::new(stream);
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap() == 0 {
                break;
            }
            let req: Request = serde_json::from_str(&line).unwrap();
            let reply: Result<Response, String> = match req {
                Request::Outputs => Ok(Response::Outputs(s.outputs.clone())),
                Request::Workspaces => Ok(Response::Workspaces(s.workspaces.clone())),
                Request::Windows => Ok(Response::Windows(s.windows.clone())),
                Request::Action(a) => {
                    log.lock().unwrap().push(a.clone());
                    match a {
                        Action::FocusWindow { id } => {
                            for w in &mut s.windows {
                                w.is_focused = w.id == id
                            }
                        }
                        Action::SetColumnWidth { .. } => {
                            s.windows
                                .iter_mut()
                                .find(|w| w.is_focused)
                                .unwrap()
                                .layout
                                .tile_size
                                .0 = 1916.0
                        }
                        Action::MoveWindowToWorkspace {
                            window_id: Some(id),
                            reference: WorkspaceReferenceArg::Id(ws),
                            ..
                        } => {
                            s.windows
                                .iter_mut()
                                .find(|w| w.id == id)
                                .unwrap()
                                .workspace_id = Some(ws)
                        }
                        _ => panic!("unexpected action {a:?}"),
                    }
                    Ok(Response::Handled)
                }
                _ => panic!("unexpected query"),
            };
            serde_json::to_writer(reader.get_mut(), &reply).unwrap();
            reader.get_mut().write_all(b"\n").unwrap();
        }
    });
    let mut client = Client::connect(&path).unwrap();
    let tasks = vec![Task::Width {
        window: 2,
        percent: 100.0,
        pixels: 1916.0,
    }];
    assert_eq!(engine::apply(&mut client, &tasks).unwrap(), 1);
    assert_eq!(engine::apply(&mut client, &tasks).unwrap(), 0);
    assert!(client.snapshot().unwrap().windows[0].is_focused);
    assert_eq!(calls.lock().unwrap().len(), 3);
    drop(client);
    server.join().unwrap();
    std::fs::remove_file(path).unwrap();
}
#[test]
fn order_only_has_no_window_operations() {
    let c = config();
    let (tasks, _) = engine::tasks(
        &c,
        &c.profile("laptop").unwrap(),
        &maps(),
        &fixture(),
        Scope::OrderOnly,
    )
    .unwrap();
    assert!(
        tasks
            .iter()
            .all(|t| matches!(t, Task::WorkspaceOrder { .. }))
    );
    let empty = BTreeSet::new();
    assert!(
        engine::tasks(
            &c,
            &c.profile("laptop").unwrap(),
            &maps(),
            &fixture(),
            Scope::New(&empty)
        )
        .unwrap()
        .0
        .is_empty()
    );
}

#[test]
fn relative_position_tracks_actual_width() {
    let mut c = config();
    c.placements = Config::parse("place-output \"^.* DeskPanel .*\" anchor=\"^eDP\" side=\"left\" offset-y=-100\nprofile \"p\"").unwrap().placements;
    let mut s = fixture();
    let mut external = s.outputs["eDP-1"].clone();
    external.name = "DP-3".into();
    external.model = "DeskPanel".into();
    for width in [1920, 2560] {
        external.logical.as_mut().unwrap().width = width;
        s.outputs.insert("DP-3".into(), external.clone());
        let (tasks, _) =
            engine::tasks(&c, &c.profile("laptop").unwrap(), &maps(), &s, Scope::All).unwrap();
        assert!(tasks.iter().any(|t|matches!(t,Task::OutputPosition{output,x,y} if output=="DP-3" && *x==-(width as i32) && *y == -100)));
    }
}
#[test]
fn first_external_and_model_selectors_are_deterministic() {
    let mut s = fixture();
    for name in ["DP-3", "HDMI-A-1"] {
        let mut o = s.outputs["eDP-1"].clone();
        o.name = name.into();
        o.model = "External".into();
        s.outputs.insert(name.into(), o);
    }
    for selector in ["first-external", "first-regex:^.* External .*"] {
        let result = engine::resolve_outputs(
            &s,
            &[("main".into(), selector.into())].into_iter().collect(),
        )
        .unwrap();
        assert_eq!(result["main"], "DP-3");
    }
    assert!(
        engine::resolve_outputs(
            &fixture(),
            &[("main".into(), "first-external".into())]
                .into_iter()
                .collect()
        )
        .is_err()
    );
}
