# Vendored crates

`nih_plug` is nih-plug at `de421011f41a6d10fc8c7a6084e4f4dee0143683`
(<https://github.com/robbert-vdh/nih-plug>, ISC license), trimmed to the
`nih_plug` and `nih_plug_derive` crates. The root `Cargo.toml` patches the git
dependency to this copy. Changes on top of upstream are separate commits that
touch only `vendor/`; `git log -- vendor/nih_plug` lists them.
