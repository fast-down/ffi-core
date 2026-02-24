use fast_down::ProgressEntry;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::IpAddr, path::PathBuf, time::Duration};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum WriteMethod {
    Mmap,
    Std,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Proxy {
    No,
    System,
    Custom(String),
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    pub save_dir: PathBuf,
    pub threads: usize,
    pub proxy: Proxy,
    pub headers: HashMap<String, String>,
    pub min_chunk_size: u64,
    pub write_buffer_size: usize,
    pub write_queue_cap: usize,
    pub retry_gap: Duration,
    pub pull_timeout: Duration,
    pub accept_invalid_certs: bool,
    pub accept_invalid_hostnames: bool,
    pub local_address: Vec<IpAddr>,
    pub max_speculative: usize,
    pub write_method: WriteMethod,
    pub have_been_downloaded_chunk: Vec<ProgressEntry>,
    pub chunk_window: u64,
}
