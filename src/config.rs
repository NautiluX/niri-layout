use anyhow::{Context, Result, bail, ensure};
use kdl::{KdlDocument, KdlNode};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub struct Config {
    pub apps: BTreeMap<String, Regex>,
    pub profiles: BTreeMap<String, Profile>,
    pub gaps: f64,
    pub horizontal_inset: f64,
    pub placements: Vec<Placement>,
}
#[derive(Clone, Debug)]
pub struct Placement {
    pub output: Regex,
    pub anchor: Regex,
    pub side: String,
    pub offset_y: i32,
}
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub extends: Option<String>,
    pub enforce_order: Option<bool>,
    pub workspaces: Vec<Workspace>,
}
#[derive(Clone, Debug)]
pub struct Workspace {
    pub name: String,
    pub output: String,
    pub order: usize,
    pub columns: Vec<Columns>,
}
#[derive(Clone, Debug)]
pub struct Columns {
    pub app: String,
    pub width: f64,
    pub skip: usize,
    pub limit: usize,
}
fn text(node: &KdlNode, key: impl Into<kdl::NodeKey>) -> Result<String> {
    node.get(key)
        .and_then(|v| v.as_string())
        .map(str::to_owned)
        .with_context(|| format!("{}: missing or non-string argument", node.name().value()))
}
fn positive(node: &KdlNode, key: &str, default: Option<usize>, zero: bool) -> Result<usize> {
    let n = match node.get(key) {
        None => return default.with_context(|| format!("{}: missing {key}", node.name().value())),
        Some(v) => v
            .as_integer()
            .and_then(|i| usize::try_from(i).ok())
            .with_context(|| format!("{key} must be an integer"))?,
    };
    ensure!(zero || n > 0, "{key} must be positive");
    Ok(n)
}
fn shape(node: &KdlNode, args: usize, props: &[&str], children: bool) -> Result<()> {
    ensure!(node.ty().is_none(), "type annotations are unsupported");
    let mut names = BTreeSet::new();
    let mut count = 0;
    for e in node.entries() {
        ensure!(e.ty().is_none(), "type annotations are unsupported");
        if let Some(name) = e.name() {
            ensure!(
                props.contains(&name.value()),
                "{}: unknown property {}",
                node.name().value(),
                name.value()
            );
            ensure!(
                names.insert(name.value()),
                "duplicate property {}",
                name.value()
            );
        } else {
            count += 1;
        }
    }
    ensure!(
        count == args,
        "{} expects {args} positional arguments",
        node.name().value()
    );
    ensure!(
        children || node.children().is_none(),
        "{} cannot have children",
        node.name().value()
    );
    Ok(())
}
impl Config {
    pub fn parse(input: &str) -> Result<Self> {
        let doc: KdlDocument = input.parse().context("invalid KDL configuration")?;
        let mut config = Self {
            apps: BTreeMap::new(),
            profiles: BTreeMap::new(),
            gaps: 2.0,
            horizontal_inset: 0.0,
            placements: vec![],
        };
        let mut scalar_seen = BTreeSet::new();
        for n in doc.nodes() {
            match n.name().value() {
                "gaps" | "horizontal-inset" => {
                    shape(n, 1, &[], false)?;
                    ensure!(
                        scalar_seen.insert(n.name().value()),
                        "duplicate {}",
                        n.name().value()
                    );
                    let value = n
                        .get(0)
                        .and_then(|v| v.as_float().or_else(|| v.as_integer().map(|i| i as f64)))
                        .context("expected numeric inset")?;
                    ensure!(
                        value.is_finite() && value >= 0.0,
                        "insets must be finite and nonnegative"
                    );
                    if n.name().value() == "gaps" {
                        config.gaps = value
                    } else {
                        config.horizontal_inset = value
                    }
                }
                "place-output" => {
                    shape(n, 1, &["anchor", "side", "offset-y"], false)?;
                    let side = text(n, "side")?;
                    ensure!(
                        side == "left" || side == "right",
                        "side must be left or right"
                    );
                    let offset_y = match n.get("offset-y") {
                        None => 0,
                        Some(value) => value
                            .as_integer()
                            .and_then(|n| i32::try_from(n).ok())
                            .context("offset-y must be an i32")?,
                    };
                    config.placements.push(Placement {
                        output: Regex::new(&text(n, 0)?)?,
                        anchor: Regex::new(&text(n, "anchor")?)?,
                        side,
                        offset_y,
                    });
                }
                "app" => {
                    shape(n, 1, &["app-id"], false)?;
                    let name = text(n, 0)?;
                    let re = Regex::new(&text(n, "app-id")?).context("invalid app-id regex")?;
                    ensure!(
                        config.apps.insert(name.clone(), re).is_none(),
                        "duplicate app {name}"
                    );
                }
                "profile" => {
                    shape(n, 1, &["extends", "enforce-order"], true)?;
                    let name = text(n, 0)?;
                    let mut p = Profile::default();
                    if n.get("extends").is_some() {
                        p.extends = Some(text(n, "extends")?)
                    }
                    if let Some(v) = n.get("enforce-order") {
                        p.enforce_order =
                            Some(v.as_bool().context("enforce-order must be a boolean")?)
                    }
                    let mut seen = BTreeSet::new();
                    if let Some(body) = n.children() {
                        for w in body.nodes() {
                            ensure!(
                                w.name().value() == "workspace",
                                "profile {name}: expected workspace"
                            );
                            shape(w, 1, &["output", "order"], true)?;
                            let mut workspace = Workspace {
                                name: text(w, 0)?,
                                output: text(w, "output")?,
                                order: positive(w, "order", None, false)?,
                                columns: vec![],
                            };
                            ensure!(workspace.order <= 255, "workspace order must be <= 255");
                            ensure!(
                                seen.insert(workspace.name.clone()),
                                "duplicate workspace {}",
                                workspace.name
                            );
                            if let Some(body) = w.children() {
                                for c in body.nodes() {
                                    let single = match c.name().value() {
                                        "column" => true,
                                        "columns" => false,
                                        other => bail!("unexpected workspace node {other}"),
                                    };
                                    shape(c, 1, &["width", "skip", "limit"], false)?;
                                    ensure!(
                                        !single || c.get("limit").is_none(),
                                        "use columns for limit"
                                    );
                                    let width_text = text(c, "width")?;
                                    let width: f64 = width_text
                                        .strip_suffix('%')
                                        .context("width must end in %")?
                                        .parse()?;
                                    ensure!(
                                        width.is_finite() && width > 0.0 && width <= 100.0,
                                        "width must be between 0 and 100%"
                                    );
                                    workspace.columns.push(Columns {
                                        app: text(c, 0)?,
                                        width,
                                        skip: positive(c, "skip", Some(0), true)?,
                                        limit: if single {
                                            1
                                        } else {
                                            positive(c, "limit", Some(usize::MAX), false)?
                                        },
                                    });
                                }
                            }
                            p.workspaces.push(workspace);
                        }
                    }
                    ensure!(
                        config.profiles.insert(name.clone(), p).is_none(),
                        "duplicate profile {name}"
                    );
                }
                other => bail!("unknown configuration node {other}"),
            }
        }
        ensure!(!config.profiles.is_empty(), "no profiles defined");
        for name in config.profiles.keys() {
            let p = config.profile(name)?;
            let mut orders = BTreeSet::new();
            for w in p.workspaces {
                ensure!(
                    !w.output.is_empty() && !w.name.is_empty(),
                    "workspace name and output cannot be empty"
                );
                ensure!(
                    orders.insert((w.output.clone(), w.order)),
                    "profile {name}: duplicate order on {}",
                    w.output
                );
                for c in w.columns {
                    ensure!(config.apps.contains_key(&c.app), "unknown app {}", c.app);
                }
            }
        }
        Ok(config)
    }
    pub fn profile(&self, name: &str) -> Result<Profile> {
        fn resolve(config: &Config, name: &str, seen: &mut BTreeSet<String>) -> Result<Profile> {
            ensure!(
                seen.insert(name.into()),
                "profile inheritance cycle at {name}"
            );
            let child = config
                .profiles
                .get(name)
                .with_context(|| format!("unknown profile {name}"))?;
            let mut result = if let Some(parent) = &child.extends {
                resolve(config, parent, seen)?
            } else {
                Profile::default()
            };
            if child.enforce_order.is_some() {
                result.enforce_order = child.enforce_order;
            }
            for w in &child.workspaces {
                result.workspaces.retain(|old| old.name != w.name);
                result.workspaces.push(w.clone());
            }
            result.extends = None;
            result
                .workspaces
                .sort_by_key(|w| (w.output.clone(), w.order));
            Ok(result)
        }
        resolve(self, name, &mut BTreeSet::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inheritance_replaces_workspace() {
        let c = Config::parse("app \"a\" app-id=\"a\"\nprofile \"laptop\" { workspace \"x\" output=\"laptop\" order=1 { columns \"a\" width=\"100%\"; }; }\nprofile \"office\" extends=\"laptop\" { workspace \"x\" output=\"external\" order=1 { columns \"a\" width=\"50%\"; }; }").unwrap();
        let p = c.profile("office").unwrap();
        assert_eq!(p.workspaces.len(), 1);
        assert_eq!(p.workspaces[0].columns[0].width, 50.0);
    }
    #[test]
    fn rejects_bad_configs() {
        for input in [
            "profile \"a\" extends=\"b\"\nprofile \"b\" extends=\"a\"",
            "profile \"a\" typo=1",
            "app \"a\" app-id=\"[\"\nprofile \"a\"",
            "profile \"a\" { workspace \"x\" output=\"l\" order=0; }",
            "profile \"a\" { workspace \"x\" output=\"l\" order=1 { column \"unknown\" width=\"100%\"; }; }",
        ] {
            assert!(Config::parse(input).is_err(), "accepted {input}");
        }
    }
}
