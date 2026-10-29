# Upload path audit

Pinned project wgpu30.0.1 / wgpu-core30.0.1 / wgpu-hal30.0.1 (Cargo.lock). Registry source hashes: archived measurements/GLES_SOURCE_IDENTITY.json. No backend fork.

`src/render/gltf_gpu.rs` FineGroup::patch compares and packs exact16B translation /112B full records. Original stream merges dirty spans across groups and reserved holes. Diagnostic ranges preserve original record mapping, membership, view/pass ownership and content epochs; bounded4096 ranges fall back to union. Original union remains default.

GLES path supported by actual observations:

- wgpu-hal src/gles/device.rs625–646: emulated MAP_WRITE staging can use a host Vec without a GL buffer; map_buffer772–826 returns host storage.
- wgpu-core src/device/queue.rs662–685: caller data copied to staging and a copy command queued. Per-write tracking/command work remains even for tiny payloads.
- wgpu-hal src/gles/queue.rs437–448: source without raw GL buffer, destination with raw buffer uses host bytes, destination binding and bufferSubData; cleanup454–464 unbinds.
- Submission fence/flush1903–1955 is per submission, not per write.

Warm API observer measured0createBuffer,0bufferData and0copyBufferSubData in sparse2048 both variants. Thus a proposed raw-to-raw copy explanation is not the measured path. More writes reduce bytes but increase command/binding count. This is a plausible mechanism, not a completed causal attribution of every slow frame. No synchronous finish/readback was added to performance loops.

Exact changed-byte diagnostics compare current CPU mirror bytes before each patch. They do not inspect physical GPU memory. On growth, zero-initialized CPU mirror tail is not proof of old retained GPU tail contents. Required whole-record traffic and sent gaps are reported separately. Instance upload bytes are already included in uniformUploadBytes; never sum them. Detailed range timing includes diagnostic byte counting and range registration; it does not isolate all initial packing/comparison.
