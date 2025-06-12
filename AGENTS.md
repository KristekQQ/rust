Before committing or pushing changes to this repository:

1. Spusťte `./ci_offline_setup.sh`, který rozbalí `vendor/`, nastaví `RUSTUP_TOOLCHAIN=stable-offline` a spustí `cargo test --offline`. Testy musí projít.
2. Poté spusťte `scripts/pre-push` pro aktualizaci `all_sources.txt`.

Offline builds are supported. Use `./pack_toolchain.sh` to create cache parts,
`./offline.sh build-vendor` to vendor dependencies, `./offline.sh join-toolchain`
to reconstruct the cached toolchain and `./offline.sh evendor` to restore the
vendor directory. Run `./ci_offline_setup.sh` to restore the environment and run
tests.
See the README for full instructions.
