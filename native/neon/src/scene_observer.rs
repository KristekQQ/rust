//! Latest full-scene mailbox. Only this low-priority worker serializes or uses TCP.
use crate::observer::Config;
use serde_json::Value;
use std::{
    io::{self, Read, Write},
    net::{SocketAddrV4, TcpStream},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use webgpu_wasm::scene::{
    publication::{RenderPublication, RenderPublisher},
    wire::WirePublisher,
    SceneManager,
};
const INTERVAL: Duration = Duration::from_millis(50);
const MAX_META: usize = 64 * 1024;
const MAX_WIRE: usize = 32 * 1024 * 1024;
const DELTA_HEADER: usize = 24;
const DELTA_BLOCK: usize = 64;
static SESSION: AtomicU64 = AtomicU64::new(0);
struct Sample {
    publication: Arc<RenderPublication>,
    state: Value,
    sampled_at: u64,
}
pub(crate) struct SceneObserver {
    slot: Arc<Mutex<Option<Sample>>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    next: Instant,
    publisher: RenderPublisher,
}
impl SceneObserver {
    pub(crate) fn load(path: Option<&Path>) -> Option<Self> {
        let (peer, token) = Config::scene_config(path)?;
        match Self::start(peer, token) {
            Ok(observer) => Some(observer),
            Err(error) => {
                log::warn!("Scene observer disabled: {error}");
                None
            }
        }
    }
    fn start(peer: SocketAddrV4, token: [u8; 16]) -> io::Result<Self> {
        let slot = Arc::new(Mutex::new(None));
        let stop = Arc::new(AtomicBool::new(false));
        let (worker_slot, worker_stop) = (Arc::clone(&slot), Arc::clone(&stop));
        let thread = thread::Builder::new()
            .name("neon-scene-observer".into())
            .spawn(move || {
                #[cfg(target_os = "android")]
                unsafe {
                    libc::setpriority(libc::PRIO_PROCESS, 0, 10);
                }
                run(peer, token, worker_slot, worker_stop);
            })?;
        Ok(Self {
            slot,
            stop,
            thread: Some(thread),
            next: Instant::now(),
            publisher: RenderPublisher::default(),
        })
    }
    /// Opt-in complete capture at <=20Hz, on simulation only; geometry stays shared.
    pub(crate) fn publish_scene(&mut self, scene: &mut SceneManager, state: &Value) {
        if Instant::now() < self.next {
            return;
        }
        if let Ok(mut slot) = self.slot.try_lock() {
            *slot = Some(Sample {
                publication: Arc::new(self.publisher.capture(scene)),
                state: state.clone(),
                sampled_at: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    .min(u64::MAX as u128) as u64,
            });
            self.next = Instant::now() + INTERVAL;
        }
    }
}
impl Drop for SceneObserver {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
/// The base advances only after ACK. A failed connection drops this entire state.
#[derive(Default)]
struct DeltaSender {
    previous: Vec<u8>,
    sequence: u32,
}
impl DeltaSender {
    fn encode(&self, raw: &[u8]) -> io::Result<Vec<u8>> {
        if raw.len() > MAX_WIRE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "scene raw frame exceeds cap",
            ));
        }
        let sequence = self.sequence.checked_add(1).ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "scene delta sequence exhausted")
        })?;
        let mut packet = Vec::with_capacity(DELTA_HEADER);
        packet.extend_from_slice(b"NDP1");
        packet.extend_from_slice(&[0; 4]);
        packet.extend_from_slice(&sequence.to_le_bytes());
        packet.extend_from_slice(&0u32.to_le_bytes());
        packet.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        packet.extend_from_slice(&0u32.to_le_bytes());
        if self.sequence != 0 {
            let mut runs = 0u32;
            let mut too_large = false;
            let mut offset = 0;
            while offset < raw.len() {
                let end = (offset + DELTA_BLOCK).min(raw.len());
                if self.previous.get(offset..end) == Some(&raw[offset..end]) {
                    offset = end;
                    continue;
                }
                let start = offset;
                offset = end;
                while offset < raw.len() {
                    let end = (offset + DELTA_BLOCK).min(raw.len());
                    if self.previous.get(offset..end) == Some(&raw[offset..end]) {
                        break;
                    }
                    offset = end;
                }
                // Stop constructing a patch as soon as it cannot beat a full frame.
                if packet.len() + 8 + offset - start >= DELTA_HEADER + raw.len() {
                    too_large = true;
                    break;
                }
                packet.extend_from_slice(&(start as u32).to_le_bytes());
                packet.extend_from_slice(&((offset - start) as u32).to_le_bytes());
                packet.extend_from_slice(&raw[start..offset]);
                runs += 1;
            }
            if !too_large && offset == raw.len() && packet.len() < DELTA_HEADER + raw.len() {
                packet[4] = 1;
                packet[12..16].copy_from_slice(&self.sequence.to_le_bytes());
                packet[20..24].copy_from_slice(&runs.to_le_bytes());
                return Ok(packet);
            }
        }
        packet.truncate(DELTA_HEADER);
        packet.extend_from_slice(raw);
        Ok(packet)
    }
    fn acknowledge(&mut self, raw: Vec<u8>) {
        self.sequence += 1;
        self.previous = raw;
    }
}
fn send_frame(stream: &mut TcpStream, meta: &[u8], wire: &[u8]) -> io::Result<()> {
    if meta.len() > MAX_META || wire.len() > MAX_WIRE + DELTA_HEADER {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "scene frame exceeds cap",
        ));
    }
    stream.write_all(&(meta.len() as u32).to_le_bytes())?;
    stream.write_all(&(wire.len() as u32).to_le_bytes())?;
    stream.write_all(meta)?;
    stream.write_all(wire)?;
    let mut ack = [0];
    stream.read_exact(&mut ack)?;
    if ack != [1] {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid scene ACK",
        ));
    }
    Ok(())
}
fn connect(peer: SocketAddrV4, token: [u8; 16]) -> io::Result<TcpStream> {
    let mut stream = TcpStream::connect_timeout(&peer.into(), Duration::from_millis(500))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_nodelay(true)?;
    stream.write_all(b"NSC1")?;
    stream.write_all(&token)?;
    Ok(stream)
}
fn run(
    peer: SocketAddrV4,
    token: [u8; 16],
    slot: Arc<Mutex<Option<Sample>>>,
    stop: Arc<AtomicBool>,
) {
    let mut connection = None;
    let mut publisher = WirePublisher::default();
    let mut delta = DeltaSender::default();
    let mut stats_at = Instant::now();
    let (mut frames, mut raw_bytes, mut encoded_bytes, mut encode_time) =
        (0u64, 0u64, 0u64, Duration::ZERO);
    let mut session = String::new();
    let mut next_send = Instant::now();
    while !stop.load(Ordering::Relaxed) {
        if Instant::now() < next_send {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        let sample = slot.lock().unwrap_or_else(|e| e.into_inner()).take();
        let Some(sample) = sample else {
            thread::sleep(Duration::from_millis(5));
            continue;
        };
        if connection.is_none() {
            match connect(peer, token) {
                Ok(stream) => {
                    connection = Some(stream);
                    publisher = WirePublisher::default();
                    delta = DeltaSender::default();
                    session = format!(
                        "{}-{}-{}",
                        std::process::id(),
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_nanos(),
                        SESSION.fetch_add(1, Ordering::Relaxed)
                    );
                }
                Err(_) => {
                    next_send = Instant::now() + Duration::from_secs(1);
                    continue;
                }
            }
        }
        let cycle_start = Instant::now();
        let result = (|| -> Result<(), String> {
            let meta = serde_json::to_vec(&serde_json::json!({"session": session, "sampledAt": sample.sampled_at, "state": sample.state})).map_err(|e| e.to_string())?;
            let wire = publisher.capture_publication(&sample.publication)?;
            let raw_len = wire.len();
            let encode_start = Instant::now();
            let encoded = delta.encode(&wire).map_err(|e| e.to_string())?;
            encode_time += encode_start.elapsed();
            send_frame(connection.as_mut().unwrap(), &meta, &encoded).map_err(|e| e.to_string())?;
            delta.acknowledge(wire);
            frames += 1;
            raw_bytes += raw_len as u64;
            encoded_bytes += encoded.len() as u64;
            Ok(())
        })();
        next_send = cycle_start + INTERVAL;
        if stats_at.elapsed() >= Duration::from_secs(5) {
            log::info!("Scene observer transport: frames={frames} raw_bytes={raw_bytes} encoded_bytes={encoded_bytes} encode_us={} window_ms={}", encode_time.as_micros(), stats_at.elapsed().as_millis());
            stats_at = Instant::now();
            (frames, raw_bytes, encoded_bytes, encode_time) = (0, 0, 0, Duration::ZERO);
        }
        if let Err(error) = result {
            log::warn!("Scene observer reconnect: {error}");
            connection = None;
            next_send = Instant::now() + Duration::from_secs(1);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, TcpListener};
    fn decode(packet: &[u8], previous: &[u8], base: u32) -> Vec<u8> {
        let number = |at| u32::from_le_bytes(packet[at..at + 4].try_into().unwrap());
        assert_eq!(&packet[..4], b"NDP1");
        assert_eq!(&packet[5..8], &[0; 3]);
        assert_eq!(number(8), base + 1);
        let length = number(16) as usize;
        if packet[4] == 0 {
            assert_eq!(number(12), 0);
            assert_eq!(number(20), 0);
            assert_eq!(packet.len(), DELTA_HEADER + length);
            return packet[DELTA_HEADER..].to_vec();
        }
        assert_eq!(packet[4], 1);
        assert_eq!(number(12), base);
        let mut output = previous.to_vec();
        output.resize(length, 0);
        let (mut cursor, mut end) = (DELTA_HEADER, 0);
        for _ in 0..number(20) {
            let offset = number(cursor) as usize;
            let len = number(cursor + 4) as usize;
            assert!(offset >= end && len > 0 && offset + len <= length);
            cursor += 8;
            output[offset..offset + len].copy_from_slice(&packet[cursor..cursor + len]);
            cursor += len;
            end = offset + len;
        }
        assert_eq!(cursor, packet.len());
        output
    }
    #[test]
    fn byte_delta_roundtrip_full_identical_changed_grow_shrink() {
        let mut sender = DeltaSender::default();
        let mut raw = vec![7; 1024];
        for step in 0..6 {
            match step {
                2 => raw[130] = 8,
                3 => raw.resize(1159, 9),
                4 => raw.truncate(129),
                5 => raw.fill(55),
                _ => (),
            }
            let packet = sender.encode(&raw).unwrap();
            assert_eq!(decode(&packet, &sender.previous, sender.sequence), raw);
            if step == 0 || step == 5 {
                assert_eq!(packet[4], 0);
            } else {
                assert_eq!(packet[4], 1);
            }
            if step == 1 || step == 4 {
                assert_eq!(packet.len(), DELTA_HEADER);
            }
            if step == 2 {
                assert_eq!(packet.len(), DELTA_HEADER + 8 + DELTA_BLOCK);
                assert_eq!(&packet[24..32], &[128, 0, 0, 0, 64, 0, 0, 0]);
            }
            sender.acknowledge(raw.clone());
        }
    }
    #[test]
    fn byte_delta_empty_and_limits() {
        let mut sender = DeltaSender::default();
        for raw in [vec![], vec![1; 65], vec![], vec![]] {
            let packet = sender.encode(&raw).unwrap();
            assert_eq!(decode(&packet, &sender.previous, sender.sequence), raw);
            sender.acknowledge(raw);
        }
        assert!(sender.encode(&vec![0; MAX_WIRE + 1]).is_err());
        sender.sequence = u32::MAX;
        assert!(sender.encode(&[]).is_err());
    }
    #[test]
    fn invalid_ack_does_not_advance_and_reconnect_resets() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        let mut sender = DeltaSender::default();
        sender.acknowledge(vec![7; 512]);
        let raw = vec![7; 512];
        let packet = sender.encode(&raw).unwrap();
        assert_eq!(packet[4], 1);
        let worker = thread::spawn(move || {
            let mut bytes = vec![0; 8 + DELTA_HEADER];
            peer.read_exact(&mut bytes).unwrap();
            peer.write_all(&[0]).unwrap();
        });
        assert!(send_frame(&mut stream, &[], &packet).is_err());
        assert_eq!(sender.sequence, 1);
        assert_eq!(sender.previous, raw);
        worker.join().unwrap();
        sender = DeltaSender::default();
        let packet = sender.encode(&raw).unwrap();
        assert_eq!(packet[4], 0);
        assert_eq!(decode(&packet, &[], 0), raw);
    }
    #[test]
    fn worker_authenticates_and_sends_full_snapshot() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let peer = match listener.local_addr().unwrap() {
            std::net::SocketAddr::V4(v) => v,
            _ => unreachable!(),
        };
        let mut observer = SceneObserver::start(peer, [7; 16]).unwrap();
        let mut scene = SceneManager::default();
        scene.add_cube(glam::Vec3::ONE);
        observer.publish_scene(&mut scene, &serde_json::json!({"phase":"playing"}));
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut auth = [0; 20];
        stream.read_exact(&mut auth).unwrap();
        assert_eq!(&auth[..4], b"NSC1");
        assert_eq!(&auth[4..], &[7; 16]);
        let mut lengths = [0; 8];
        stream.read_exact(&mut lengths).unwrap();
        let meta_len = u32::from_le_bytes(lengths[..4].try_into().unwrap()) as usize;
        let wire_len = u32::from_le_bytes(lengths[4..].try_into().unwrap()) as usize;
        assert!(meta_len <= MAX_META && wire_len <= MAX_WIRE + DELTA_HEADER);
        let mut meta = vec![0; meta_len];
        stream.read_exact(&mut meta).unwrap();
        let meta: Value = serde_json::from_slice(&meta).unwrap();
        assert_eq!(meta["state"]["phase"], "playing");
        assert!(!meta["session"].as_str().unwrap().is_empty());
        assert!(meta["sampledAt"].as_u64().unwrap() > 0);
        let mut wire = vec![0; wire_len];
        stream.read_exact(&mut wire).unwrap();
        assert_eq!(&wire[..4], b"NDP1");
        assert_eq!(wire[4], 0);
        let wire = &wire[DELTA_HEADER..];
        assert_eq!(&wire[..8], b"NWR1\x04\x00\x00\x00");
        assert_eq!(u64::from_le_bytes(wire[8..16].try_into().unwrap()), 1);
        let mut mirror = SceneManager::default();
        mirror.apply_wire_publication(wire, &[]).unwrap();
        stream.write_all(&[1]).unwrap();
        drop(observer);
    }
    #[test]
    fn frame_limits_and_ack() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut sender = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut receiver, _) = listener.accept().unwrap();
        assert_eq!(
            send_frame(&mut sender, &vec![0; MAX_META + 1], &[])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            send_frame(&mut sender, &[], &vec![0; MAX_WIRE + DELTA_HEADER + 1])
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        let worker = thread::spawn(move || {
            let mut header = [0; 8];
            receiver.read_exact(&mut header).unwrap();
            assert_eq!(header, [2, 0, 0, 0, 3, 0, 0, 0]);
            let mut body = [0; 5];
            receiver.read_exact(&mut body).unwrap();
            assert_eq!(&body, b"{}NWR");
            receiver.write_all(&[1]).unwrap();
        });
        send_frame(&mut sender, b"{}", b"NWR").unwrap();
        worker.join().unwrap();
    }
}
