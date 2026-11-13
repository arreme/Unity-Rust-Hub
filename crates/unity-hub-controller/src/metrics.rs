//! Opt-in CPU, memory, and uptime sampling for the Processes page.
//!
//! `sysinfo` is only linked by the controller, so the CLI adapter, the model
//! contract, and the visual layer stay dependency-free.

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use sysinfo::{Pid, ProcessesToUpdate, System};

#[derive(Debug, Clone, PartialEq)]
pub struct ProcessMetrics {
    /// `None` until a second sample exists, because CPU is a delta.
    pub cpu_percent: Option<f32>,
    pub memory_bytes: u64,
    pub uptime: Duration,
}

/// Keeps one `System` across ticks: sysinfo needs the previous sample to
/// compute a CPU percentage. The first sample for a PID therefore reports
/// `cpu_percent: None` rather than a misleading zero.
pub struct Sampler {
    system: System,
    primed: HashSet<u32>,
}

impl Default for Sampler {
    fn default() -> Self {
        Self::new()
    }
}

impl Sampler {
    pub fn new() -> Self {
        Self {
            system: System::new(),
            primed: HashSet::new(),
        }
    }

    pub fn sample(&mut self, pids: &[u32]) -> HashMap<u32, ProcessMetrics> {
        if pids.is_empty() {
            self.primed.clear();
            return HashMap::new();
        }

        let system_pids: Vec<Pid> = pids.iter().copied().map(Pid::from_u32).collect();
        self.system
            .refresh_processes(ProcessesToUpdate::Some(&system_pids), true);

        let mut samples = HashMap::new();
        for pid in pids {
            let Some(process) = self.system.process(Pid::from_u32(*pid)) else {
                continue;
            };
            samples.insert(
                *pid,
                ProcessMetrics {
                    cpu_percent: self.primed.contains(pid).then(|| process.cpu_usage()),
                    memory_bytes: process.memory(),
                    uptime: Duration::from_secs(process.run_time()),
                },
            );
        }

        // Vanished processes must not keep a stale "primed" entry that would
        // report a bogus percentage if the PID is reused later.
        self.primed.retain(|pid| pids.contains(pid));
        self.primed.extend(pids.iter().copied());
        samples
    }

    /// Stops sampling; the next enabled sample starts with a fresh system view.
    pub fn reset(&mut self) {
        self.system = System::new();
        self.primed.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_cpu_sample_is_a_placeholder_and_the_second_is_a_percentage() {
        let mut sampler = Sampler::new();
        let pid = std::process::id();

        let first = sampler.sample(&[pid]);
        let first = first.get(&pid).expect("this process should be visible");
        assert_eq!(first.cpu_percent, None);
        assert!(first.memory_bytes > 0);
        assert!(first.uptime.as_secs() < 60);

        std::thread::sleep(Duration::from_millis(20));
        let second = sampler.sample(&[pid]);
        let second = second
            .get(&pid)
            .expect("this process should still be visible");
        assert!(second.cpu_percent.is_some());
    }

    #[test]
    fn a_vanished_process_disappears_without_failing_the_sample() {
        let mut sampler = Sampler::new();
        // PIDs are small integers on every supported platform; this one is not
        // expected to exist.
        let samples = sampler.sample(&[u32::MAX - 1]);
        assert!(samples.is_empty());
    }

    #[test]
    fn clearing_the_pid_list_resets_the_primed_state() {
        let mut sampler = Sampler::new();
        let pid = std::process::id();
        sampler.sample(&[pid]);
        assert!(sampler.primed.contains(&pid));

        sampler.sample(&[]);
        assert!(sampler.primed.is_empty());
    }
}
