Before committing or pushing changes to this repository:

1. Spusťte `./ci_offline_setup.sh`, který rozbalí `vendor/`, nastaví `RUSTUP_TOOLCHAIN=stable-offline` a spustí `cargo test --offline`. Testy musí projít.
2. Poté spusťte `scripts/pre-push` pro aktualizaci `all_sources.txt`.

Offline builds are supported. Use `./offline.sh pack-all` to archive the vendor
directory and toolchain caches in one step. Run `./offline.sh unpack-all` or
`./ci_offline_setup.sh` to restore everything and run the tests.
See the README for full instructions.
