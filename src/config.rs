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
    pub write_method: WriteMethod,
    /// 使用哪些地址来发送请求
    pub local_address: Vec<IpAddr>,
    /// 投机线程数
    pub max_speculative: usize,
    /// 已经下载过的部分
    pub have_been_downloaded_chunk: Vec<ProgressEntry>,
    /// 过滤掉 have_been_downloaded_chunk 中小于 chunk_window 的部分
    pub chunk_window: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            save_dir: PathBuf::new(),
            threads: 32,
            proxy: Proxy::System,
            headers: HashMap::new(),
            min_chunk_size: 8 * 1024 * 1024,
            write_buffer_size: 16 * 1024 * 1024,
            write_queue_cap: 10240,
            retry_gap: Duration::from_millis(500),
            pull_timeout: Duration::from_secs(5),
            accept_invalid_certs: false,
            accept_invalid_hostnames: false,
            local_address: Vec::new(),
            max_speculative: 3,
            write_method: WriteMethod::Mmap,
            have_been_downloaded_chunk: Vec::new(),
            chunk_window: 8 * 1024,
        }
    }
}
