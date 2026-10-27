# Independent scoped review

The existing Astra consultant independently read the current shader/variant changes and the completed `neutral-final-over` and `neutral-final-three` reports on 2026-10-26. It did not run GPU tests, edit source, or verify a physical device.

- No formula/layout blocker was found in the reviewed no-map omission and integer-power candidate. Direct old/new no-map image checking was missing from the integer-power oracle; it was added to the driver and independently applied to all 88 existing capture pairs (maximum delta 0). Integer powers were subsequently rejected for no pilot gain and restored, rather than retained on source intuition.
- Both final static reports have three alternating 8 s/25 s pairs, profiler OFF, no changed driver inputs and no server errors. `7b0→a54` proves combined native improvement, not an isolated TBN benefit. Its tolerance gate passes but pixels are not exact; 11 is the number of pixels above channel delta 8, not all changed pixels.
- The direct original-Three test is a separate comparison: native 35.0–36.4 FPS versus Three 60. The native result cannot be substituted into the first paired experiment. AA/shader/shadow differences and null cross-engine pixel gate prevent a graphics-parity claim.
- Active CPU comparisons use baseline `292f`, distinct from static `7b0`. At a 60 Hz cap they do not establish FPS gains.

The reviewer required maintained instancing, skinning, actual reflection and production checks before delivery. Those results and deployment status are recorded separately in `DELIVERY.md`; this review is not a substitute for them.
