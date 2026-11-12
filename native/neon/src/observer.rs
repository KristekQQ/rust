//! Opt-in pose telemetry. Frame input only replaces a fixed latest-value slot;
//! configuration, coordinate encoding and all UDP work stay off the XR thread.
use glam::{EulerRot, Mat4, Quat};
use serde::Deserialize;
use std::{
    io::{self, Read},
    net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const INTERVAL: Duration = Duration::from_nanos(33_333_334);
const POLL: Duration = Duration::from_millis(4);
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    host: Ipv4Addr,
    port: u16,
    token: String,
    #[serde(rename = "scenePort")]
    scene_port: Option<u16>,
}
impl Config {
    pub(crate) fn scene_config(score_path: Option<&Path>) -> Option<(SocketAddrV4, [u8; 16])> {
        let path = score_path?.parent()?.join("neon-observer.json");
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return None,
            Err(error) => {
                log::warn!("Scene observer disabled: {error}");
                return None;
            }
        };
        let result = (|| -> Result<_, String> {
            let mut bytes = Vec::new();
            file.take(4097)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            let (peer, token) = Self::decode(&bytes)?;
            let config: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
            let port = config
                .scene_port
                .or_else(|| peer.port().checked_add(1))
                .filter(|p| *p != 0)
                .ok_or("invalid scene port")?;
            Ok((SocketAddrV4::new(*peer.ip(), port), token))
        })();
        match result {
            Ok(config) => Some(config),
            Err(error) => {
                log::warn!("Scene observer disabled: {error}");
                None
            }
        }
    }
    fn decode(bytes: &[u8]) -> Result<(SocketAddrV4, [u8; 16]), String> {
        if bytes.len() > 4096 {
            return Err("configuration exceeds 4096 bytes".into());
        }
        let config: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        if config.scene_port == Some(0)
            || config.port == 0
            || config.host.is_unspecified()
            || config.host.is_multicast()
            || config.host.is_broadcast()
        {
            return Err("expected a unicast IPv4 host and nonzero port".into());
        }
        if config.token.len() != 32 || !config.token.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err("expected a 32-digit hexadecimal token".into());
        }
        let mut token = [0; 16];
        for (i, byte) in token.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&config.token[i * 2..i * 2 + 2], 16)
                .map_err(|e| e.to_string())?;
        }
        Ok((SocketAddrV4::new(config.host, config.port), token))
    }
}
#[derive(Clone, Copy)]
struct Sample {
    captured: SystemTime,
    valid: bool,
    poses: [Option<Mat4>; 3],
}
#[derive(Default)]
struct Slot {
    generation: u64,
    latest: Option<Sample>,
}
pub(crate) struct Observer {
    slot: Arc<Mutex<Slot>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Observer {
    /// Missing private config is normal. Every other setup error disables telemetry.
    pub(crate) fn load(score_path: Option<&Path>) -> Option<Self> {
        let path = score_path?.parent()?.join("neon-observer.json");
        let file = match std::fs::File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return None,
            Err(e) => {
                log::warn!("Pose observer disabled: {e}");
                return None;
            }
        };
        let mut bytes = Vec::new();
        if let Err(e) = file.take(4097).read_to_end(&mut bytes) {
            log::warn!("Pose observer disabled: {e}");
            return None;
        }
        let (peer, token) = match Config::decode(&bytes) {
            Ok(value) => value,
            Err(e) => {
                log::warn!("Pose observer disabled: {e}");
                return None;
            }
        };
        match Self::start(peer, token) {
            Ok(observer) => Some(observer),
            Err(e) => {
                log::warn!("Pose observer disabled: {e}");
                None
            }
        }
    }
    fn start(peer: SocketAddrV4, token: [u8; 16]) -> io::Result<Self> {
        let slot = Arc::new(Mutex::new(Slot::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (thread_slot, thread_stop) = (Arc::clone(&slot), Arc::clone(&stop));
        let thread = thread::Builder::new()
            .name("neon-observer".into())
            .spawn(move || {
                #[cfg(target_os = "android")]
                // Linux/Android setpriority(who=0) affects only this calling thread.
                unsafe {
                    if libc::setpriority(libc::PRIO_PROCESS, 0, 10) != 0 {
                        log::warn!("Pose observer priority: {}", io::Error::last_os_error());
                    }
                }
                let socket = match UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).and_then(|socket| {
                    socket.set_nonblocking(true)?;
                    Ok(socket)
                }) {
                    Ok(socket) => socket,
                    Err(e) => {
                        log::warn!("Pose observer disabled: {e}");
                        return;
                    }
                };
                run(socket, peer, token, thread_slot, thread_stop);
            })?;
        Ok(Self {
            slot,
            stop,
            thread: Some(thread),
        })
    }
    /// No allocation, network, serialization or blocking lock on the XR path.
    pub(crate) fn publish(&self, dt: f32, head: Option<Mat4>, hands: [Option<Mat4>; 2]) {
        if let Ok(mut slot) = self.slot.try_lock() {
            slot.latest = Some(Sample {
                captured: SystemTime::now(),
                valid: dt.is_finite() && dt > 0. && head.is_some(),
                poses: [head, hands[0], hands[1]],
            });
            slot.generation = slot.generation.wrapping_add(1);
        }
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
fn micros(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_micros().min(u64::MAX as u128) as u64)
}
fn valid_pose(pose: Mat4) -> bool {
    if !pose.is_finite() || pose.w_axis.truncate().abs().max_element() > 100. {
        return false;
    }
    let axes = [
        pose.x_axis.truncate(),
        pose.y_axis.truncate(),
        pose.z_axis.truncate(),
    ];
    pose.x_axis.w.abs() < 1e-4
        && pose.y_axis.w.abs() < 1e-4
        && pose.z_axis.w.abs() < 1e-4
        && (pose.w_axis.w - 1.).abs() < 1e-4
        && axes.iter().all(|a| (a.length_squared() - 1.).abs() < 0.002)
        && axes[0].dot(axes[1]).abs() < 0.001
        && axes[0].dot(axes[2]).abs() < 0.001
        && axes[1].dot(axes[2]).abs() < 0.001
        && (pose.determinant() - 1.).abs() < 0.003
}
fn encode(sample: Sample, token: [u8; 16], session: u64, sequence: u64) -> [u8; 120] {
    let mut out = [0u8; 120];
    out[..4].copy_from_slice(b"NPO1");
    out[4..20].copy_from_slice(&token);
    out[20..28].copy_from_slice(&session.to_le_bytes());
    out[28..36].copy_from_slice(&sequence.to_le_bytes());
    out[36..44].copy_from_slice(&micros(sample.captured).to_le_bytes());
    let mut flags = 0u32;
    if sample.valid && sample.poses[0].is_some_and(valid_pose) {
        for (i, matrix) in sample.poses.into_iter().enumerate() {
            if let Some(matrix) = matrix.filter(|m| valid_pose(*m)) {
                let p = matrix.w_axis.truncate();
                let (y, x, z) = Quat::from_mat4(&matrix).normalize().to_euler(EulerRot::YXZ);
                for (j, value) in [p.x, p.y, p.z, x, y, z].into_iter().enumerate() {
                    let offset = 48 + i * 24 + j * 4;
                    out[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
                flags |= 1 << i;
            }
        }
    }
    out[44..48].copy_from_slice(&flags.to_le_bytes());
    out
}
fn clock_reply(
    request: &[u8],
    source: SocketAddr,
    peer: SocketAddrV4,
    token: [u8; 16],
) -> Option<[u8; 36]> {
    if source.ip() != *peer.ip()
        || request.len() != 28
        || &request[..4] != b"NPP1"
        || request[4..20] != token
    {
        return None;
    }
    let mut response = [0; 36];
    response[..28].copy_from_slice(request);
    response[..4].copy_from_slice(b"NPA1");
    response[28..36].copy_from_slice(&micros(SystemTime::now()).to_le_bytes());
    Some(response)
}
fn run(
    socket: UdpSocket,
    peer: SocketAddrV4,
    token: [u8; 16],
    slot: Arc<Mutex<Slot>>,
    stop: Arc<AtomicBool>,
) {
    let session = micros(SystemTime::now());
    let (mut generation, mut sequence) = (0, 0u64);
    let mut next = Instant::now();
    let mut failed = false;
    let mut request = [0u8; 64];
    while !stop.load(Ordering::Relaxed) {
        // A hostile or busy peer cannot starve pose sending or shutdown.
        for _ in 0..16 {
            match socket.recv_from(&mut request) {
                Ok((n, source)) => {
                    if let Some(reply) = clock_reply(&request[..n], source, peer, token) {
                        let _ = socket.send_to(&reply, source);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    if !failed {
                        log::warn!("Pose observer receive: {e}");
                        failed = true;
                    }
                    break;
                }
            }
        }
        if Instant::now() >= next {
            let latest = slot.try_lock().ok().and_then(|slot| {
                (slot.generation != generation).then_some((slot.generation, slot.latest))
            });
            if let Some((current, Some(sample))) = latest {
                generation = current;
                sequence = sequence.saturating_add(1);
                let packet = encode(sample, token, session, sequence);
                match socket.send_to(&packet, peer) {
                    Ok(_) => failed = false,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) => {
                        if !failed {
                            log::warn!("Pose observer send: {e}");
                            failed = true;
                        }
                    }
                }
                next = Instant::now() + INTERVAL;
            }
        }
        thread::sleep(POLL);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    fn sample() -> Sample {
        Sample {
            captured: UNIX_EPOCH + Duration::from_micros(123456),
            valid: true,
            poses: [Some(Mat4::IDENTITY); 3],
        }
    }
    #[test]
    fn packet_layout_rotation_and_visibility_match_xr() {
        let mut s = sample();
        s.poses[1] = Some(Mat4::from_rotation_translation(
            Quat::from_euler(EulerRot::YXZ, 0.7, -0.2, 0.4),
            Vec3::new(1., 2., 3.),
        ));
        s.poses[2] = None;
        let packet = encode(s, [0xab; 16], 7, 9);
        assert_eq!(&packet[..4], b"NPO1");
        assert_eq!(&packet[4..20], &[0xab; 16]);
        assert_eq!(u64::from_le_bytes(packet[20..28].try_into().unwrap()), 7);
        assert_eq!(u64::from_le_bytes(packet[28..36].try_into().unwrap()), 9);
        assert_eq!(
            u64::from_le_bytes(packet[36..44].try_into().unwrap()),
            123456
        );
        assert_eq!(u32::from_le_bytes(packet[44..48].try_into().unwrap()), 3);
        for (i, expected) in [1., 2., 3., -0.2, 0.7, 0.4].into_iter().enumerate() {
            assert!(
                (f32::from_le_bytes(packet[72 + i * 4..76 + i * 4].try_into().unwrap()) - expected)
                    .abs()
                    < 1e-5
            );
        }
        for head in [None, Some(Mat4::ZERO)] {
            s.poses[0] = head;
            assert_eq!(&encode(s, [0; 16], 1, 1)[44..120], &[0; 76]);
        }
        s = sample();
        s.valid = false;
        assert_eq!(&encode(s, [0; 16], 1, 1)[44..120], &[0; 76]);
        s = sample();
        s.poses[2] = Some(Mat4::from_translation(Vec3::new(f32::NAN, 0., 0.)));
        assert_eq!(&encode(s, [0; 16], 1, 1)[44..48], &3u32.to_le_bytes());
    }
    #[test]
    fn strict_config_and_clock_authentication() {
        assert!(Observer::load(None).is_none());
        let missing = std::env::temp_dir()
            .join(format!(
                "neon-observer-missing-{}-{}",
                std::process::id(),
                micros(SystemTime::now())
            ))
            .join("top5.json");
        assert!(Observer::load(Some(&missing)).is_none());
        assert!(Config::decode(&[b' '; 4097]).is_err());
        let (peer, token) = Config::decode(
            br#"{"host":"127.0.0.1","port":8792,"token":"0123456789abcdef0123456789ABCDEF"}"#,
        )
        .unwrap();
        assert_eq!(
            token,
            [1, 35, 69, 103, 137, 171, 205, 239, 1, 35, 69, 103, 137, 171, 205, 239]
        );
        for bytes in [
            br#"{}"#.as_slice(),
            br#"{"host":"localhost","port":1,"token":"00"}"#.as_slice(),
            br#"{"host":"127.0.0.1","port":0,"token":"0123456789abcdef0123456789abcdef"}"#
                .as_slice(),
        ] {
            assert!(Config::decode(bytes).is_err());
        }
        let mut request = [0; 28];
        request[..4].copy_from_slice(b"NPP1");
        request[4..20].copy_from_slice(&token);
        request[20..28].copy_from_slice(&99u64.to_le_bytes());
        let source = SocketAddr::V4(peer);
        let response = clock_reply(&request, source, peer, token).unwrap();
        assert_eq!(&response[..4], b"NPA1");
        assert_eq!(&response[4..28], &request[4..]);
        assert!(clock_reply(&request, "127.0.0.2:8792".parse().unwrap(), peer, token).is_none());
        request[4] ^= 1;
        assert!(clock_reply(&request, source, peer, token).is_none());
    }
    #[test]
    fn udp_latest_slot_clock_and_nonblocking_frame_path() {
        let receiver = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        receiver
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let peer = match receiver.local_addr().unwrap() {
            SocketAddr::V4(peer) => peer,
            _ => unreachable!(),
        };
        let observer = Observer::start(peer, [7; 16]).unwrap();
        // Contention drops the sample without blocking; the latest slot is bounded.
        let guard = observer.slot.lock().unwrap();
        observer.publish(0.01, Some(Mat4::IDENTITY), [None; 2]);
        assert!(guard.latest.is_none());
        drop(guard);
        for x in 0..100 {
            observer.publish(
                0.01,
                Some(Mat4::from_translation(Vec3::new(x as f32, 0., 0.))),
                [None; 2],
            );
        }
        let mut bytes = [0; 256];
        let deadline = Instant::now() + Duration::from_secs(2);
        let source = loop {
            let (n, source) = receiver.recv_from(&mut bytes).unwrap();
            assert_eq!(n, 120);
            if f32::from_le_bytes(bytes[48..52].try_into().unwrap()) == 99. {
                break source;
            }
            assert!(Instant::now() < deadline);
        };
        let mut request = [0; 28];
        request[..4].copy_from_slice(b"NPP1");
        request[4..20].copy_from_slice(&[7; 16]);
        request[20..28].copy_from_slice(&123u64.to_le_bytes());
        receiver.send_to(&request, source).unwrap();
        loop {
            let (n, _) = receiver.recv_from(&mut bytes).unwrap();
            if &bytes[..4] == b"NPA1" {
                assert_eq!(n, 36);
                assert_eq!(&bytes[20..28], &123u64.to_le_bytes());
                break;
            }
            assert!(Instant::now() < deadline);
        }
        observer.publish(0., Some(Mat4::IDENTITY), [Some(Mat4::IDENTITY); 2]);
        loop {
            let (n, _) = receiver.recv_from(&mut bytes).unwrap();
            if n == 120 {
                assert_eq!(&bytes[44..48], &0u32.to_le_bytes());
                break;
            }
            assert!(Instant::now() < deadline);
        }
        let start = Instant::now();
        drop(observer);
        assert!(start.elapsed() < Duration::from_millis(200));
    }
}
