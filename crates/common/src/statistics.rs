use std::time::{Duration, Instant, SystemTime, SystemTimeError};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Serialize)]
pub struct Statistics {
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
            sent_bytes: vec![],
            read_bytes: vec![],
            pings: vec![]
        }
    }

    pub fn sent_bytes(&mut self, size: u64) {
        self.sent_bytes.push((SystemTime::now(), size));
    }

    pub fn received_bytes(&mut self, size: u64) {
        self.read_bytes.push((SystemTime::now(), size));
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