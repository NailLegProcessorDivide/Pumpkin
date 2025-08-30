use crate::SHOULD_STOP;
use crossbeam::atomic::AtomicCell;
use std::{
    cell::RefCell,
    collections::VecDeque,
    num::NonZeroU32,
    sync::atomic::Ordering,
    thread::sleep,
    time::{Duration, Instant},
};

pub struct Ticker {
    last_tick: Instant,
    target_tick_rate: AtomicCell<Option<NonZeroU32>>,
    performance_history: RefCell<VecDeque<u128>>,
    history_duration: usize,
}

impl Ticker {
    #[must_use]
    pub fn new(target_freq: Option<NonZeroU32>) -> Self {
        Self {
            last_tick: Instant::now(),
            target_tick_rate: AtomicCell::new(target_freq),
            performance_history: RefCell::new(VecDeque::new()),
            history_duration: 100,
        }
    }

    /// IMPORTANT: Run this in a new thread/tokio task.
    pub async fn run<F>(&mut self, mut run_fn: F)
    where
        F: FnMut(),
    {
        let mut last_tick_time = Instant::now();
        while !SHOULD_STOP.load(Ordering::Relaxed) {
            let tick_start_time = Instant::now();
            if let Some(target_rate) = self.target_tick_rate.load() {
                let period = 1.0 / u32::from(target_rate) as f64;
                // Add period to last tick to account for skew more effectively
                last_tick_time += Duration::from_secs_f64(period);
                // If the server is running slow, report lag and jump the target time forward
                // The server will run slower over time than it should but theres nothing we can do
                if tick_start_time > last_tick_time + Duration::from_millis(200) {
                    log::warn!(
                        "Server running slow, {}ms behind target tick rate, skipping ticks",
                        (tick_start_time - last_tick_time).as_millis()
                    );
                    last_tick_time = tick_start_time;
                } else if tick_start_time < last_tick_time {
                    sleep(last_tick_time - tick_start_time);
                }
            }
            let start_time = Instant::now();
            run_fn();
            let end_time = Instant::now();
            {
                let mut perf_history = self.performance_history.borrow_mut();
                if perf_history.len() > 100 {
                    perf_history.pop_front();
                }
                perf_history.push_back((end_time - start_time).as_micros());
            }
        }
        log::debug!("Ticker stopped");
    }
}
