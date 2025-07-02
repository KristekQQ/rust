Before committing or pushing changes to this repository:

1. Run `cargo test` and ensure all tests pass.
2. Then run `scripts/pre-push` to update `all_sources.txt`.

If needed you can run `./offline.sh pack-all` to build offline archives. Use `./offline.sh unpack-all` to restore them.
See the README for full instructions.
