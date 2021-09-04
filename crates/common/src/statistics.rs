use std::time::{Duration, Instant, SystemTime, SystemTimeError};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

const MAX_VEC_LENGTH: usize = 30;

#[derive(Clone, Serialize, Deserialize)]
pub struct Statistics {
    total_sent_bytes: u64,
    total_read_bytes: u64,
    sent_bytes: Vec<(SystemTime, u64)>,
    read_bytes: Vec<(SystemTime, u64)>,
    pings: Vec<Duration>
}

struct InstantWrapper {}

pub fn serialize<S>(instant: &Instant, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let duration = instant.elapsed();
    duration.serialize(serializer)
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<Instant, D::Error>
where
    D: Deserializer<'de>,
{
    let duration = Duration::deserialize(deserializer)?;
    let now = Instant::now();
    let instant = now.checked_sub(duration).ok_or_else(|| panic!("Err checked_add"))?;
    Ok(instant)
}

// TODO: Optimize
// TODO: Test if the stats are actually accurate. I have a feeling that the average calculation is not right.
impl Statistics {
    pub fn new() -> Self {
        Statistics {
            total_sent_bytes: 0,
            total_read_bytes: 0,
            sent_bytes: vec![],
            read_bytes: vec![],
            pings: vec![],
        }
    }

    pub fn sent_bytes(&mut self, size: u64) {
        self.total_sent_bytes += size;
        self.sent_bytes.push((SystemTime::now(), size));
        if self.sent_bytes.len() > MAX_VEC_LENGTH {
            self.sent_bytes.swap_remove(0);
        }
    }

    pub fn received_bytes(&mut self, size: u64) {
        self.total_read_bytes += size;
        self.read_bytes.push((SystemTime::now(), size));
        if self.read_bytes.len() > MAX_VEC_LENGTH {
            self.read_bytes.swap_remove(0);
        }
    }

    pub fn new_ping(&mut self, ping: Duration) {
        self.pings.push(ping);
    }

    /// Get the average received bytes/second since a given duration
    pub fn get_avg_received(&self, dur: Duration) -> Result<u64, SystemTimeError> {
        let mut sum = 0; // Calculate the total received
        for (i, c) in self.read_bytes.iter().rev() {
            // FIXME: This is a horrible workaround
            if i.elapsed()? > dur {
                break;
            }
            sum += c;
        }
        Ok(sum / dur.as_secs())
    }

    /// Get the average sent bytes/second since a given duration
    pub fn get_avg_sent(&self, dur: Duration) -> Result<u64, SystemTimeError> {
        let mut sum = 0; // Calculate the total sent
        for (i, c) in self.sent_bytes.iter().rev() {
            // FIXME: This is a horrible workaround
            if i.elapsed()? > dur { 
                break;
            }
            sum += c;
        }
        Ok(sum / dur.as_secs())
    }

    /// Get the total received data in bytes
    pub fn get_total_received(&self) -> u64 {
        self.total_read_bytes
    }

    /// Get the total sent data in bytes
    pub fn get_total_sent(&self) -> u64 {
        self.total_sent_bytes
    }

    pub fn get_last_ping(&self) -> Option<&Duration> {
        self.pings.last()
    }
}