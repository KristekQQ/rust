//! Nonblocking, sampled pass timestamps and bounded CPU timing windows.
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
};

const SAMPLE_EVERY: u64 = 30;
const MAX_LABELS: usize = 64;
const QUERY_COUNT: u32 = MAX_LABELS as u32 * 2;
const WINDOW: usize = 60;
const BUFFER_BYTES: u64 = QUERY_COUNT as u64 * 8;

thread_local! {
    static CURRENT: RefCell<Option<Rc<RefCell<Collector>>>> = const { RefCell::new(None) };
}

struct Slot {
    queries: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    busy: bool,
}
struct GpuSample {
    frame: u64,
    passes: BTreeMap<String, f64>,
}
struct CpuHistory {
    pending_ms: f64,
    touched: bool,
    samples: VecDeque<f64>,
}
struct Collector {
    frame: u64,
    slots: Vec<Slot>,
    active: Option<usize>,
    labels: Vec<String>,
    latest: Option<GpuSample>,
    error: Option<&'static str>,
    skipped: u64,
    cpu: BTreeMap<&'static str, CpuHistory>,
}

/// Owned query handle: no collector borrow survives into a render pass.
pub struct PassStamp {
    queries: wgpu::QuerySet,
    first: u32,
}
impl PassStamp {
    pub fn writes(&self) -> wgpu::RenderPassTimestampWrites<'_> {
        wgpu::RenderPassTimestampWrites {
            query_set: &self.queries,
            beginning_of_pass_write_index: Some(self.first),
            end_of_pass_write_index: Some(self.first + 1),
        }
    }
}

pub fn initialize(device: &wgpu::Device) {
    let mut slots = Vec::new();
    if device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
        for _ in 0..3 {
            slots.push(Slot {
                queries: device.create_query_set(&wgpu::QuerySetDescriptor {
                    label: Some("sampled pass timestamps"),
                    ty: wgpu::QueryType::Timestamp,
                    count: QUERY_COUNT,
                }),
                resolve: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("timestamp resolve"),
                    size: BUFFER_BYTES,
                    usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                    mapped_at_creation: false,
                }),
                read: device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("timestamp async readback"),
                    size: BUFFER_BYTES,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                busy: false,
            });
        }
    }
    CURRENT.with(|current| {
        *current.borrow_mut() = Some(Rc::new(RefCell::new(Collector {
            frame: 0,
            slots,
            active: None,
            labels: Vec::new(),
            latest: None,
            error: None,
            skipped: 0,
            cpu: BTreeMap::new(),
        })))
    });
}

pub fn reset() {
    // Pending callbacks own the old collector and only release its old buffers.
    CURRENT.with(|current| *current.borrow_mut() = None);
}

pub fn begin_frame() {
    CURRENT.with(|current| {
        let Some(collector) = current.borrow().as_ref().cloned() else {
            return;
        };
        let mut c = collector.borrow_mut();
        // An abandoned frame has no pending readback and may release its slot.
        if let Some(index) = c.active.take() {
            c.slots[index].busy = false;
        }
        c.frame += 1;
        for history in c.cpu.values_mut() {
            history.pending_ms = 0.;
            history.touched = false;
        }
        c.labels.clear();
        if !c.slots.is_empty() && (c.frame == 1 || c.frame % SAMPLE_EVERY == 0) {
            if let Some(index) = c.slots.iter().position(|slot| !slot.busy) {
                c.slots[index].busy = true;
                c.active = Some(index);
            } else {
                c.skipped += 1;
            }
        }
    });
}

pub fn stamp(label: &str) -> Option<PassStamp> {
    CURRENT.with(|current| {
        let collector = current.borrow().as_ref().cloned()?;
        let mut c = collector.borrow_mut();
        let index = c.active?;
        if c.labels.len() >= MAX_LABELS {
            return None;
        }
        let first = c.labels.len() as u32 * 2;
        c.labels.push(label.chars().take(96).collect());
        Some(PassStamp {
            queries: c.slots[index].queries.clone(),
            first,
        })
    })
}

pub fn end_frame(device: &wgpu::Device, queue: &wgpu::Queue) {
    let submission = CURRENT.with(|current| {
        let collector = current.borrow().as_ref().cloned()?;
        let mut c = collector.borrow_mut();
        for history in c.cpu.values_mut().filter(|history| history.touched) {
            if history.samples.len() == WINDOW {
                history.samples.pop_front();
            }
            history.samples.push_back(history.pending_ms);
            history.touched = false;
        }
        let index = c.active.take()?;
        let labels = std::mem::take(&mut c.labels);
        if labels.is_empty() {
            c.slots[index].busy = false;
            return None;
        }
        let frame = c.frame;
        let slot = &c.slots[index];
        let handles = (
            slot.queries.clone(),
            slot.resolve.clone(),
            slot.read.clone(),
        );
        drop(c);
        Some((collector, index, labels, frame, handles))
    });
    let Some((collector, index, labels, frame, (queries, resolve, read))) = submission else {
        return;
    };
    let count = labels.len() as u32 * 2;
    let bytes = count as u64 * 8;
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("sampled timestamp readback"),
    });
    encoder.resolve_query_set(&queries, 0..count, &resolve, 0);
    encoder.copy_buffer_to_buffer(&resolve, 0, &read, 0, bytes);
    queue.submit(Some(encoder.finish()));
    let period_ms = queue.get_timestamp_period() as f64 / 1_000_000.;
    let mapped = read.clone();
    read.slice(0..bytes)
        .map_async(wgpu::MapMode::Read, move |result| {
            let sample = if result.is_ok() {
                mapped
                    .slice(0..bytes)
                    .get_mapped_range()
                    .ok()
                    .and_then(|range| {
                        let mut passes = BTreeMap::<String, f64>::new();
                        for (label, pair) in
                            labels.into_iter().zip(range.as_chunks::<16>().0.iter())
                        {
                            let begin = u64::from_le_bytes(pair[..8].try_into().ok()?);
                            let end = u64::from_le_bytes(pair[8..].try_into().ok()?);
                            let ticks = end.checked_sub(begin)?;
                            *passes.entry(label).or_default() += ticks as f64 * period_ms;
                        }
                        Some(GpuSample { frame, passes })
                    })
            } else {
                None
            };
            mapped.unmap();
            let mut c = collector.borrow_mut();
            c.slots[index].busy = false;
            if let Some(sample) = sample {
                if c.latest
                    .as_ref()
                    .is_none_or(|latest| sample.frame >= latest.frame)
                {
                    c.latest = Some(sample);
                    c.error = None;
                }
            } else {
                c.error = Some("Timestamp readback unavailable");
            }
        });
}

fn now() -> Option<f64> {
    web_sys::window()?
        .performance()
        .map(|performance| performance.now())
}

pub struct CpuScope {
    collector: Option<Rc<RefCell<Collector>>>,
    label: &'static str,
    start: Option<f64>,
    frame: u64,
}
pub fn cpu_scope(label: &'static str) -> CpuScope {
    let collector = CURRENT.with(|current| current.borrow().as_ref().cloned());
    let frame = collector
        .as_ref()
        .map_or(0, |collector| collector.borrow().frame);
    let start = collector.as_ref().and_then(|_| now());
    CpuScope {
        collector,
        label,
        start,
        frame,
    }
}
impl Drop for CpuScope {
    fn drop(&mut self) {
        let Some(collector) = &self.collector else {
            return;
        };
        let (Some(start), Some(end)) = (self.start, now()) else {
            return;
        };
        let mut c = collector.borrow_mut();
        if c.frame == self.frame && (c.cpu.contains_key(self.label) || c.cpu.len() < MAX_LABELS) {
            let history = c.cpu.entry(self.label).or_insert_with(|| CpuHistory {
                pending_ms: 0.,
                touched: false,
                samples: VecDeque::with_capacity(WINDOW),
            });
            history.pending_ms += (end - start).max(0.);
            history.touched = true;
        }
    }
}

pub fn stats() -> serde_json::Value {
    CURRENT.with(|current| {
        let current = current.borrow();
        let Some(collector) = current.as_ref() else {
            return serde_json::json!({"enabled":false,"frame":null,"cpu":{},"gpu":{"supported":false,"available":false,"passesMs":null,"sampleFrame":null,"ageFrames":null}});
        };
        let c = collector.borrow();
        let cpu: BTreeMap<_, _> = c.cpu.iter().filter(|(_, history)| !history.samples.is_empty()).map(|(label, history)| {
            let history = &history.samples;
            let mut sorted: Vec<_> = history.iter().copied().collect();
            sorted.sort_by(f64::total_cmp);
            let p95 = sorted[((sorted.len() as f64 * 0.95).ceil() as usize).saturating_sub(1)];
            (label, serde_json::json!({"lastMs":history.back(),"meanMs":history.iter().sum::<f64>() / history.len() as f64,"p95Ms":p95,"samples":history.len()}))
        }).collect();
        serde_json::json!({
            "enabled":true,"frame":c.frame,"cpu":cpu,
            "gpu":{
                "supported":!c.slots.is_empty(),"available":c.latest.is_some(),
                "sampleEveryFrames":SAMPLE_EVERY,"skippedSamples":c.skipped,
                "sampleFrame":c.latest.as_ref().map(|sample| sample.frame),
                "ageFrames":c.latest.as_ref().map(|sample| c.frame.saturating_sub(sample.frame)),
                "passesMs":c.latest.as_ref().map(|sample| &sample.passes),"error":c.error,
            }
        })
    })
}
