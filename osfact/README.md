# osfact - os-factory: a device's installable image, for any distro and any device

A Rust crate grown inside lab-tools and kept extractable: it depends on no lpoco crate (lab-tools depends on it,
never the reverse; `tests/boundary.rs`), it has its own version and tag prefix (`osfact-v*`, not yet released),
and its own docs folder (`docs/`). Why it is so, and when it leaves for its own repository: `docs/README.md`.

| folder | what it is |
|---|---|
| `src/` | the crate: `boot` (the boot-format interface; `android_bootimg`, `avb`, `fdt` are its first adapter and its parts), `install` (install.toml read, its commands listed; a plan that flashes is refused), the `osfact` command (`cli.rs`, argenv) |
| `tests/` | the boundary, and the failing walleye test the first build makes pass |
| `tests/fixtures/` | google-walleye's declaration, two distro profiles, and `plans/` (install.toml cases, each refused one named so), the tests' inputs |
| `docs/` | osfact's own docs: its shape, its boundary, its way out |
