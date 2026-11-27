# osfact - os-factory: a device's installable image, for any distro and any device

A Rust crate grown inside lab-tools and kept extractable: it depends on no lpoco crate (lab-tools depends on it,
never the reverse; `tests/boundary.rs`), it has its own version and tag prefix (`osfact-v*`, not yet released),
and its own docs folder (`docs/`). Why it is so, and when it leaves for its own repository: `docs/README.md`.

| folder | what it is |
|---|---|
| `src/` | the crate (no code yet) |
| `tests/` | the boundary, and the failing walleye test the first build makes pass |
| `tests/fixtures/` | google-walleye's declaration and two distro profiles, the test's inputs |
| `docs/` | osfact's own docs: its shape, its boundary, its way out |
