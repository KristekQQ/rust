//! Owned cut snapshots run entirely on a bounded CPU worker; no scene/GPU access.
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::thread::JoinHandle;
use webgpu_wasm::rim_game::{CutWork, PreparedCut};

const CAPACITY: usize = 2;
struct Job {
    generation: u64,
    work: CutWork,
}
struct Reply {
    generation: u64,
    result: Result<PreparedCut, String>,
    #[cfg(test)]
    execution_thread: std::thread::ThreadId,
}
pub(crate) struct CutWorker {
    sender: Option<SyncSender<Job>>,
    replies: Receiver<Reply>,
    thread: Option<JoinHandle<()>>,
    generation: u64,
    inflight: usize,
}
impl CutWorker {
    pub fn new() -> Result<Self, String> {
        let (sender, jobs) = mpsc::sync_channel::<Job>(CAPACITY);
        let (results, replies) = mpsc::sync_channel(CAPACITY);
        let thread = std::thread::Builder::new()
            .name("rim-cut".into())
            .spawn(move || {
                while let Ok(job) = jobs.recv() {
                    let reply = Reply {
                        generation: job.generation,
                        result: job.work.execute(),
                        #[cfg(test)]
                        execution_thread: std::thread::current().id(),
                    };
                    if results.send(reply).is_err() {
                        break;
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        log::info!("Rim cut worker started; max {CAPACITY} inflight jobs");
        Ok(Self {
            sender: Some(sender),
            replies,
            thread: Some(thread),
            generation: 0,
            inflight: 0,
        })
    }
    pub fn available(&self) -> bool {
        self.inflight < CAPACITY
    }
    pub fn submit(&mut self, work: CutWork) -> Result<(), String> {
        if !self.available() {
            return Err("Rim cut worker capacity reached".into());
        }
        self.sender
            .as_ref()
            .ok_or("Rim cut worker stopped")?
            .try_send(Job {
                generation: self.generation,
                work,
            })
            .map_err(|e| e.to_string())?;
        self.inflight += 1;
        Ok(())
    }
    /// Poll never waits for geometry. Old encounters cannot publish into a new one,
    /// even when a fresh Game reuses the same cut tokens and scene/model IDs.
    pub fn poll(&mut self) -> Result<Option<PreparedCut>, String> {
        loop {
            match self.replies.try_recv() {
                Ok(reply) => {
                    #[cfg(test)]
                    {
                        assert_ne!(reply.execution_thread, std::thread::current().id());
                        assert_eq!(
                            reply.execution_thread,
                            self.thread.as_ref().unwrap().thread().id()
                        );
                    }
                    self.inflight -= 1;
                    if reply.generation == self.generation {
                        return reply.result.map(Some);
                    }
                }
                Err(TryRecvError::Empty) => return Ok(None),
                Err(TryRecvError::Disconnected) => return Err("Rim cut worker disconnected".into()),
            }
        }
    }
    pub fn invalidate(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }
}
impl Drop for CutWorker {
    fn drop(&mut self) {
        // Only teardown joins. At most CAPACITY replies can exist, so the worker
        // can finish without waiting for a receiver drain after closing input.
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
