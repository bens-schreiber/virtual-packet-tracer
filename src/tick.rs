use std::collections::HashMap;
use std::hash::Hash;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

/// Provides an interface to freeze and manipulate time.
pub struct TimeProvider {
    frozen: Option<SystemTime>,
    offset: Duration,
    last_unfrozen: SystemTime,
    last_frozen: Option<SystemTime>,
}

impl TimeProvider {
    /// A singleton instance of TimeProvider.
    // Really, I just don't want to pass the instance around everywhere and I'm definitely not going to make a DI container.
    pub fn instance() -> &'static Mutex<Self> {
        static INSTANCE: OnceLock<Mutex<TimeProvider>> = OnceLock::new();
        INSTANCE.get_or_init(|| Mutex::new(TimeProvider::new()))
    }

    pub fn new() -> Self {
        Self {
            frozen: None,
            offset: Duration::ZERO,
            last_unfrozen: SystemTime::now(),
            last_frozen: None,
        }
    }

    /// Freezes the current time.
    /// This will cause the TimeProvider to return the same time until `unfreeze` is called.
    pub fn freeze(&mut self) {
        if self.frozen.is_some() {
            panic!("TimeProvider is already frozen");
        }

        self.frozen = Some(self.now());
        self.last_frozen = self.frozen;
    }

    pub fn unfreeze(&mut self) {
        if self.frozen.is_none() {
            panic!("TimeProvider is not frozen");
        }

        if let Some(frozen_time) = self.frozen {
            self.offset += frozen_time
                .duration_since(self.last_unfrozen)
                .expect("Time went backwards");
        }

        self.frozen = None;
        self.last_unfrozen = SystemTime::now();
    }

    /// Advances the current frozen time by the given duration.
    pub fn advance(&mut self, duration: Duration) {
        match self.frozen {
            Some(frozen_time) => {
                self.frozen = Some(frozen_time + duration);
            }
            None => {
                panic!("TimeProvider is not frozen");
            }
        }
    }

    /// Returns the time. Not accurate to SystemTime::now(), considers frozen time and offset.
    pub fn now(&self) -> SystemTime {
        match self.frozen {
            Some(frozen_time) => frozen_time,
            None => SystemTime::now() + self.offset,
        }
    }

    pub fn last_frozen(&self) -> Option<SystemTime> {
        self.last_frozen
    }
}

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
        let now = {
            let tp = TimeProvider::instance().lock().unwrap();
            tp.now()
        };

        if None == self.map.get(&key) {
            let time_to_ready = now + Duration::new(interval_in_seconds, 0);
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
        let now = {
            let tp = TimeProvider::instance().lock().unwrap();
            tp.now()
        };

        self.map
            .iter()
            .filter(|(_, (time_ready, _, _))| *time_ready <= now)
            .map(|(k, _)| k.clone())
            .collect()
    }
}

impl<T: Eq + Hash + Clone> Tickable for TickTimer<T> {
    fn tick(&mut self) {
        let now = {
            let tp = TimeProvider::instance().lock().unwrap();
            tp.now()
        };

        self.map
            .retain(|_, (time_ready, _, persist)| *time_ready > now || *persist);

        for (_, (time_ready, interval_in_seconds, persist)) in self.map.iter_mut() {
            if *time_ready > now || !*persist {
                continue;
            }
            *time_ready = now + *interval_in_seconds;
        }
    }
}
