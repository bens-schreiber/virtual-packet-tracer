use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, SystemTime};

pub trait Tickable {
    fn tick(&mut self);
}

/// Schedules events to occur at a time interval. Call `tick` to update the timer.
pub struct TickTimer<T: Eq + Hash + Clone> {
    map: HashMap<T, (SystemTime, Duration, bool)>, // (time_ready, interval_in_seconds, persist)
}

impl<T: Eq + Hash + Clone> TickTimer<T> {
    pub fn new() -> Self {
        TickTimer {
            map: HashMap::new(),
        }
    }

    /// Adds a key to the timer IFF it doesn't already exist.
    /// * `key` - The key to add to the timer.
    /// * `interval` - The interval in seconds to wait before the key is ready.
    /// * `persist` - If the key should persist after it is ready.
    pub fn schedule(&mut self, key: T, interval_in_seconds: u64, persist: bool) {
        if None == self.map.get(&key) {
            let time_to_ready = SystemTime::now() + Duration::new(interval_in_seconds, 0);
            self.map.insert(
                key,
                (
                    time_to_ready,
                    Duration::new(interval_in_seconds, 0),
                    persist,
                ),
            );
        }
    }

    /// Returns a list of keys that are ready to be processed.
    pub fn ready(&self) -> Vec<T> {
        let mut ready = vec![];
        for (key, (time_ready, _, _)) in self.map.iter() {
            if *time_ready <= SystemTime::now() {
                ready.push(key.clone());
            }
        }

        ready
    }
}

impl<T: Eq + Hash + Clone> Tickable for TickTimer<T> {
    fn tick(&mut self) {
        self.map
            .retain(|_, (time_until, _, persist)| *time_until > SystemTime::now() || *persist);

        for (_, (time_ready, interval_in_seconds, persist)) in self.map.iter_mut() {
            if *time_ready < SystemTime::now() {
                continue;
            }
            if !*persist {
                continue;
            }
            *time_ready = SystemTime::now() + *interval_in_seconds;
        }
    }
}
