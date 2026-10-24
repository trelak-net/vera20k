//! Selected ordinary CellClass knowledge state, distinct from the legacy cache
//! projection. Original487630/487690/4876F0,6FB170/6FB470 and578100.
//! Native counter arithmetic wraps; pending is not equivalent to current sight.
use crate::sim::intern::InternedId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct ShroudKnowledge {
    pub counter: i32,
    pub gap_counter: i32,
    pub open: bool,
    pub pending: bool,
    pub local_sources: u32,
    pub allied_sources: u32,
    pub transient_visible: bool,
}
impl Default for ShroudKnowledge {
    fn default() -> Self {
        Self {
            counter: 1,
            gap_counter: 0,
            open: false,
            pending: false,
            local_sources: 0,
            allied_sources: 0,
            transient_visible: false,
        }
    }
}
impl ShroudKnowledge {
    /// Feed the same native-endian bytes as the derived field hashes to the
    /// state's streaming DefaultHasher, batching tiny writes across cells.
    /// No struct padding or simulation fields are changed. The slice length
    /// prefix matches `[ShroudKnowledge]::hash` too.
    pub(crate) fn hash_slice_streaming(values: &[Self], hasher: &mut impl std::hash::Hasher) {
        use std::hash::Hash;
        values.len().hash(hasher);
        const CELL_BYTES: usize = 19;
        let mut buffer = [0u8; CELL_BYTES * 256];
        for chunk in values.chunks(256) {
            for (value, bytes) in chunk.iter().zip(buffer.chunks_exact_mut(CELL_BYTES)) {
                bytes[0..4].copy_from_slice(&value.counter.to_ne_bytes());
                bytes[4..8].copy_from_slice(&value.gap_counter.to_ne_bytes());
                bytes[8] = u8::from(value.open);
                bytes[9] = u8::from(value.pending);
                bytes[10..14].copy_from_slice(&value.local_sources.to_ne_bytes());
                bytes[14..18].copy_from_slice(&value.allied_sources.to_ne_bytes());
                bytes[18] = u8::from(value.transient_visible);
            }
            hasher.write(&buffer[..chunk.len() * CELL_BYTES]);
        }
    }

    pub fn reveal(&mut self) {
        if self.counter == 1 {
            self.counter = 0;
        }
        self.counter = self.counter.wrapping_sub(1);
        if self.counter <= 0 {
            //487630 opens missing18 and returns. Only the already-open branch
            //cancels20: a second gap followed by return must retain pending.
            if self.open {
                self.pending = false;
            } else {
                self.open = true;
            }
        }
    }
    pub fn leave(&mut self) {
        let old = self.counter;
        if self.counter == -1 {
            self.counter = 0;
        }
        self.counter = self.counter.wrapping_add(1).min(self.gap_counter);
        if old <= 0 && self.counter > 0 {
            self.pending = true;
        }
    }
    pub fn fire_unshroud(&mut self) {
        self.open = true;
        if self.counter > 0 {
            self.pending = true;
        }
    }
    pub fn add_gap(&mut self) {
        if self.counter != 1 && self.counter >= 0 {
            self.counter = self.counter.wrapping_add(1);
        }
        self.gap_counter = self.gap_counter.wrapping_add(1);
        if self.counter > 0 {
            self.open = false;
        }
    }
    pub fn remove_gap(&mut self, spy_sat_active: bool) {
        self.gap_counter = self.gap_counter.wrapping_sub(1);
        if spy_sat_active && self.gap_counter <= 0 {
            self.counter = self.counter.wrapping_sub(1);
            if self.counter <= 0 {
                self.open = true;
            }
        }
    }
    pub fn sweep(&mut self) {
        if self.pending {
            self.open = false;
            self.pending = false;
        }
    }
}

/// The existing source producer supplies geometry. Native70AF50 admits one
/// ordinary contribution under latch250 and stores254XYZ/260radius;70B1D0
/// clears the admission latch then releases that stored footprint, retaining
/// the stored geometry. Reconciliation must not
/// turn an unchanged source into another reveal event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct SightAdmission {
    pub owner: InternedId,
    pub origin: (u16, u16, i32),
    pub radius: u16,
    pub cells: Vec<(u16, u16)>,
}

/// Native Foot65C/664 countdown, one logical native client per viewer. The
/// constructor initial value is retained so an unseen/nonallied viewer never
/// borrows another viewer's timer history. Admission release does not reset it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub(crate) struct SightRefreshTimers {
    initial: crate::sim::timer::CdTimer,
    pub(crate) by_viewer: std::collections::BTreeMap<InternedId, crate::sim::timer::CdTimer>,
}
impl SightRefreshTimers {
    pub(crate) fn at_construction(frame: u32) -> Self {
        Self {
            initial: crate::sim::timer::CdTimer::started(frame as i32, 0),
            by_viewer: Default::default(),
        }
    }
    pub(crate) fn timer(&self, viewer: InternedId) -> crate::sim::timer::CdTimer {
        self.by_viewer.get(&viewer).copied().unwrap_or(self.initial)
    }
    pub(crate) fn reload(&mut self, viewer: InternedId, frame: u32) {
        self.by_viewer.insert(
            viewer,
            crate::sim::timer::CdTimer::started(frame as i32, 15),
        );
    }
}

#[cfg(test)]
mod streaming_hash_tests {
    use super::*;
    use std::hash::{Hash, Hasher};

    #[test]
    fn batched_shroud_hash_matches_derived_hash_across_chunk_boundaries() {
        let values: Vec<_> = (0..1025u32)
            .map(|i| ShroudKnowledge {
                counter: i.wrapping_mul(0x87654321) as i32,
                gap_counter: i.wrapping_mul(0xfedcba98) as i32,
                open: i % 2 == 0,
                pending: i % 3 == 0,
                local_sources: i.wrapping_mul(0x12345678),
                allied_sources: u32::MAX - i,
                transient_visible: i % 5 == 0,
            })
            .collect();
        for len in [0, 1, 255, 256, 257, 511, 512, 1025] {
            let mut old = std::collections::hash_map::DefaultHasher::new();
            let mut new = std::collections::hash_map::DefaultHasher::new();
            // Exercise both incoming and outgoing partial SipHash blocks.
            42u8.hash(&mut old);
            42u8.hash(&mut new);
            values[..len].hash(&mut old);
            ShroudKnowledge::hash_slice_streaming(&values[..len], &mut new);
            0x12345678u32.hash(&mut old);
            0x12345678u32.hash(&mut new);
            assert_eq!(old.finish(), new.finish(), "slice length {len}");
        }
    }
}
