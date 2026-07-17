# Contributing

Thanks for wanting to help with this project. It is a small, purpose-built tool
and is not intended to have an active maintenance roadmap.

## Development setup

Install the Rust toolchain declared in `rust-toolchain.toml`, clone the
repository, and run:

```bash
cargo build --locked
cargo test --locked
```

Linux builds use X11/XWayland so native file drag and drop works.

## Pull requests

Keep changes focused and describe how they were tested. Before submitting, run:

```bash
cargo fmt --all -- --check
cargo test --locked --all-targets
cargo clippy --locked --all-targets -- -D warnings
```

Do not commit project sidecar folders (`*.wdassetproj_data/`), local build
output, or personal Wonderdraft assets. New behavior should include tests where
practical.

By contributing, you agree that your contribution is distributed under the
repository's [MIT License](LICENSE).
