use std::time::{Duration, Instant};

#[derive(Clone)]
pub struct Statistics {
    sent_bytes: Vec<(Instant, u64)>,
    read_bytes: Vec<(Instant, u64)>,
    pings: Vec<Duration>
}

// TODO: Optimize
// TODO: Test if the stats are actually accurate. I have a feeling that the average calculation is not right.
impl Statistics {
    pub fn new() -> Self {
        Statistics {
            sent_bytes: vec![],
            read_bytes: vec![],
            pings: vec![]
        }
    }

    pub fn sent_bytes(&mut self, size: u64) {
        self.sent_bytes.push((Instant::now(), size));
    }

    pub fn received_bytes(&mut self, size: u64) {
        self.read_bytes.push((Instant::now(), size));
    }

    pub fn new_ping(&mut self, ping: Duration) {
        self.pings.push(ping);
    }

    /// Get the average received bytes/second since a given duration
    pub fn get_avg_received(&self, dur: Duration) -> u64 {
        let mut sum = 0; // Calculate the total received
        for (i, c) in self.read_bytes.iter().rev() {
            if i.elapsed() > dur {
                break;
            }
            sum += c;
        }
        sum / dur.as_secs()
    }

    /// Get the average sent bytes/second since a given duration
    pub fn get_avg_sent(&self, dur: Duration) -> u64 {
        let mut sum = 0; // Calculate the total sent
        for (i, c) in self.sent_bytes.iter().rev() {
            if i.elapsed() > dur {
                break;
            }
            sum += c;
        }
        sum / dur.as_secs()
    }

    /// Get the total received data in bytes
    pub fn get_total_received(&self) -> u64 {
        let mut sum = 0;
        for (_, c) in self.read_bytes.iter() {
            sum += c;
        }
        sum
    }

    /// Get the total sent data in bytes
    pub fn get_total_sent(&self) -> u64 {
        let mut sum = 0;
        for (_, c) in self.sent_bytes.iter() {
            sum += c;
        }
        sum
    }

    pub fn get_last_ping(&self) -> Option<&Duration> {
        self.pings.last()
    }
}