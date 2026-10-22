# Installing prebuilt releases

Download an archive and its `.sha256` file from the [GitHub Releases page](https://github.com/NautiluX/niri-layout/releases/latest).
Use `uname -m` to choose `linux-x86_64` (Intel/AMD 64-bit) or `linux-aarch64`
(ARM64). These are native-tested static musl executables; no Rust installation or
particular glibc version is required. The session wrapper/installer require Bash
and standard Linux utilities. niri and, for automatic profiles, Kanshi must still
be installed separately.

From the download directory, replacing the version/architecture as appropriate:

```sh
sha256sum --check niri-layout-v0.1.0-linux-x86_64.tar.gz.sha256
tar -xzf niri-layout-v0.1.0-linux-x86_64.tar.gz
cd niri-layout-v0.1.0-linux-x86_64
./install.sh
```

By default this installs:

- `~/.local/bin/niri-layout`
- `~/.local/bin/niri-layout-session`
- Documentation and fictional examples under `~/.local/share/niri-layout/`

`PREFIX=/absolute/path ./install.sh` selects another installation prefix. Ensure
its `bin` directory is on the session PATH; a nondefault prefix must also be visible
to the startup wrapper's environment. The installer does not install Kanshi,
modify config files, start processes or require root for a user installation.

Follow [getting started](getting-started.md#3-write-the-three-configurations) to
adapt examples and activate the companion. For upgrades, install the new release
and restart the running companion/wrapper at a convenient time; replacing the
executable does not upgrade an already running process. Configuration is preserved.
To remove the installation, stop its processes/remove its startup entry and delete
the two installed executables and the `share/niri-layout` directory from the chosen
prefix. Config files are separate and remain yours.

A checksum detects corrupted downloads; it is not an independent signature of the
release publisher. Obtain both files from the same trusted release page.

## CI and publishing

`.github/workflows/build.yml` runs on pull requests, main pushes, `v*` tag pushes,
and manual dispatch. Each native Linux architecture job installs the pinned Rust
compiler, tests the musl target, runs the fake-compositor smoke test and config
check, verifies static linkage, packages the archive, and tests installation into
a temporary prefix. CI artifacts are downloadable for 14 days; these are development
builds unless they come from a published version tag.

A pushed version tag must exactly match `v` plus `package.version` in Cargo.toml.
After both architecture jobs pass, the release job downloads/verifies their assets,
creates or resumes a draft release, uploads the archives and checksums, then
publishes it. Tags containing a hyphen are marked as prereleases. Already-published
release assets are never overwritten by reruns. Fixes should receive a new version.

Maintainer procedure, with a chosen version and a clean reviewed checkout:

1. Update Cargo.toml and Cargo.lock together and commit to main.
2. Confirm the main CI run passes.
3. Create an annotated `vVERSION` tag on that commit and push that tag.
4. Inspect the tag workflow and published assets.

Tag publication requires repository write permission. Ordinary build jobs have only
read permission; the separate tag release job has `contents: write`. No personal
configuration, build cache or whole checkout is included in the release archive:
packaging uses an explicit executable allowlist plus tracked docs and examples.
