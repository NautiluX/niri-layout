# Working on niri-layout

## Start here

Read README.md, docs/configuration.md and docs/authoring.md before generating or
changing layout configuration. examples/README.md explains the coordinated example
files. The implemented parser (src/config.rs), planner/executor (src/engine.rs),
daemon (src/daemon.rs) and CLI (src/main.rs) are authoritative if docs disagree.

## Configuration work

- Use only implemented KDL v2 nodes and properties. Do not invent profile matching,
  tabbed columns, browser-account identification, app launching or workspace creation.
- Kanshi selects profiles; role mappings connect its hooks to layout profiles.
- A child profile replaces a whole workspace definition, including its columns.
- Validate static configuration with `niri-layout --config FILE check`, then inspect
  `plan` against available hardware. Only apply within the user's authorized scope.
- Preserve unrelated desktop configuration and provide rollback instructions when
  changing startup or competing layout managers. Do not infer that every Python
  helper on a machine belongs to this project's layout responsibility.

## Public repository boundary

This repository must not contain personal deployment/configuration files, monitor
serials, usernames or absolute home paths, app/account inventories, live desktop
snapshots, session logs, backups or credentials. Use fictional identities in examples
and fixtures. Keep real configurations outside the checkout or in ignored `local/`.
`deploy/` and `docs/deployment.md` are also ignored legacy local paths; do not force-add
them. Ignore rules do not protect already tracked files: inspect the Git index too.
Before publication, review staged changes and scan publishable content for
machine-specific values. Do not commit/push unless authorized by the user.

## Verification

```sh
cargo build --release --locked
cargo test --locked
python3 tests/daemon_smoke.py
./target/release/niri-layout --config examples/config.kdl check
```

The tests need local Unix sockets but must never connect to a real desktop.
Changing examples may require updating synthetic fixtures that reference them.
Keep documentation examples executable where possible and verify links locally.
For docs-only edits, run the relevant config/example checks; do not change live
configuration merely to validate documentation.
