use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const SLOT_SECONDS: u64 = 30;
pub const EPOCH_SLOTS:  u64 = 32;

#[derive(Clone, Copy)]
pub struct SlotClock {
    pub genesis: u64, // unix seconds
}

impl SlotClock {
    pub fn new_aligned_now() -> Self {
        let now = now_secs();
        let genesis = now - (now % SLOT_SECONDS);
        Self { genesis }
    }
    pub fn slot_of(&self, ts: u64) -> u64 { (ts.saturating_sub(self.genesis)) / SLOT_SECONDS }
    pub fn now_slot(&self) -> u64 { self.slot_of(now_secs()) }
    pub fn epoch_of_slot(&self, slot: u64) -> u64 { slot / EPOCH_SLOTS }
    pub fn slot_in_epoch(&self, slot: u64) -> u64 { slot % EPOCH_SLOTS }
    pub fn next_slot_start(&self) -> u64 {
        let now = now_secs();
        let s = self.slot_of(now) + 1;
        self.genesis + s * SLOT_SECONDS
    }
}

pub fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs()
}

pub fn sleep_until(next_unix: u64) -> tokio::time::Sleep {
    let now = now_secs();
    let dur = if next_unix > now { Duration::from_secs(next_unix - now) } else { Duration::from_secs(0) };
    tokio::time::sleep(dur)
}
