use std::time::{Duration, SystemTime, SystemTimeError};

use serde::{Deserialize, Serialize};

const MAX_VEC_LENGTH: usize = 30;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub enum TransferState {
    Transfering,
    Complete,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct TransferStatistics {
    pub started: SystemTime,
    pub bytes_written: usize,
    /// Can be more than the file size if more than one peer requests the file.
    pub bytes_read: usize,
    pub state: TransferState,
}

impl TransferStatistics {
    pub fn new() -> Self {
        Self {
            started: SystemTime::now(),
            bytes_written: 0,
            bytes_read: 0,
            state: TransferState::Transfering,
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ConnectionStatistics {
    total_sent_bytes: u64,
    total_read_bytes: u64,
    sent_bytes: Vec<(SystemTime, u64)>,
    read_bytes: Vec<(SystemTime, u64)>,
    pings: Vec<Duration>,
}

// TODO: Optimize
// TODO: Test if the stats are actually accurate. I have a feeling that the average calculation is not right.
impl ConnectionStatistics {
    pub fn new() -> Self {
        ConnectionStatistics {
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

    /// Register a new ping data point
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
