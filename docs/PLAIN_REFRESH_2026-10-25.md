# Plain refresh without public release parameters

Owned isolated checkout: rim-publish/rust, branch codex/core-render-optimization,
base088fdb34 plus preserved follow-ups. User requests ordinary reload at the
normal8443 URL; public `v=` parameters are not required.

The server already sent `Cache-Control: no-store, no-cache, must-revalidate`,
Pragma/Expires and suppressed conditional304 responses. This policy remains.
Existing HTML/index links containing `v` now receive a temporary canonical redirect
that removes only that marker and preserves backend/profile/view options. JS,
WASM and other resource query parameters remain untouched. The Rome cut worker
now explicitly requests its WASM with `cache:'no-store'`, matching the main SDK.
No render/simulation change or Rust rebuild was needed.

Avoided broad Clear-Site-Data cache clearing: browser implementations may also
evict compiled shader caches; ordinary HTTP freshness is sufficient here.
Reference: [MDN Clear-Site-Data](https://developer.mozilla.org/en-US/docs/Web/HTTP/Reference/Headers/Clear-Site-Data).
No cookies, localStorage or saved game preferences were cleared.

Validation:
- Two actual HTTP server tests passed: canonical options, changed body after
  conditional requests, no-store headers and compressed/identity delivery.
- Four production browser cases passed: normal reload with browser cache enabled
  and real Rome cut/log bootstrap, each on WebGPU and WebGL2. Each reload fetched
  26 relevant runtime resources with zero cache hits; worker initialized, expected
  build/backend persisted and localStorage marker survived. Old public `v` links
  became canonical automatically.
- Astra read-only review found no blocker. Canonicalization applies to GET
  navigation; HEAD retains its earlier behavior. The claim concerns HTTP loading,
  not disabling browser-internal shader/compiled-code caches.

Accepted app703a135f71 reuses WASM
c4301d8142f89cda9472095b128a458372101d646a7b9ed1980e84a5569b526a.
Restarted the owned official npm serve:quest server with unchanged TLS files;
new PID43950, tool session47351, normal HTTPS8443. Official packaging regenerated
identical accepted payload identity. All219 live files matched accepted bytes
with no-store, even with stale ETag/future If-Modified-Since request headers.
Canonical HTTPS navigation verified. Previous7c37709d6c package retained in
output/plain-refresh-2026-10-25; all22 examples and Three8878 preserved.
Evidence: browser-report.json and delivery.json in that task output directory.
No user-tab automation, physical Quest acceptance, commit, push or CI run.
