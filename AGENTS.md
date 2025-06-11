Before committing or pushing changes to this repository, run `scripts/pre-push` to update `all_sources.txt`.

Offline builds are supported. Use `./pack_toolchain.sh` to create cache parts,
`./join_toolchain.sh` to reconstruct the cached toolchain and
`./ci_offline_setup.sh` to restore the environment and run tests.
See the README for full instructions.
