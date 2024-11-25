use std::hash::Hash;

/// Converts a time in seconds to a time in ticks.
/// TODO: Ticks should be globally defined somewhere as 30.
#[macro_export]
macro_rules! tseconds {
    ($seconds:expr) => {
        $seconds * 30 // 30 ticks per second
    };
}

pub trait Tickable {
    fn tick(&mut self, tick: u32);
}

pub struct TickTimer<T: Eq + Hash + Clone> {
    map: std::collections::HashMap<T, (u32, u32, bool)>, // (time_until, interval, persist)
}

impl<T: Eq + Hash + Clone> TickTimer<T> {
    pub fn new() -> Self {
        TickTimer {
            map: std::collections::HashMap::new(),
        }
    }

    /// Adds a key to the timer IFF it doesn't already exist.
    /// * `key` - The key to add to the timer.
    /// * `interval` - The interval in ticks to wait before the key is ready.
    /// * `persist` - If the key should persist after it is ready.
    pub fn schedule(&mut self, key: T, interval: u32, persist: bool) {
        if None == self.map.get(&key) {
            self.map.insert(key, (interval, interval, persist));
        }
    }

    /// Returns a list of keys that are ready to be processed.
    pub fn ready(&self) -> Vec<T> {
        let mut ready = vec![];
        for (key, (time_until, _, _)) in self.map.iter() {
            if *time_until == 0 {
                ready.push(key.clone());
            }
        }

        ready
    }
}

impl<T: Eq + Hash + Clone> Tickable for TickTimer<T> {
    fn tick(&mut self, _tick: u32) {
        self.map
            .retain(|_, (time_until, _, persist)| *time_until != 0 || *persist);

        for (_, (time_until, interval, persist)) in self.map.iter_mut() {
            if *time_until == 0 {
                if !*persist {
                    continue;
                }
                *time_until = *interval;
            } else {
                *time_until = time_until.saturating_sub(1);
            }
        }
    }
}
