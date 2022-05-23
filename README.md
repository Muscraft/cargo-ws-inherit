# cargo-ws-inherit

## About
This is a tool to assist in the move to workspace-inheritance!
* RFC: [#2906](https://github.com/rust-lang/rfcs/blob/master/text/2906-cargo-workspace-deduplicate.md)
* Tracking Issue: [#8415](https://github.com/rust-lang/cargo/issues/8415)
* [Status](https://github.com/rust-lang/cargo/issues/8415#issuecomment-1112618913)
* [Example Port](https://github.com/clap-rs/clap/pull/3719)

Currently, it is very limited it in its capabilities with hope that it will be
ready in time for stabilization.

- [ ] Share Duplicate dependencies
  - [x] Path Dependencies
  - [x] Git dependencies
  - [x] Optional dependencies
  - [x] Version checking
  - [x] `default-features`
  - [ ] `rename`
  - [ ] Multi-source dependencies
- [ ] Share package keys
- [ ] Adding `cargo-features = ["workspace-inheritance"]` to manifests
- [ ] Cargo subcommand support
- [ ] Manual mode

## License

Licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)
  at your option.