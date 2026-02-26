#![doc = include_str!("../README.md")]

mod config;
mod download;
mod error;
mod event;

pub use config::*;
pub use download::*;
pub use error::*;
pub use event::*;
pub use fast_down;
pub use fast_down::{
    AnyError, BoxPusher, DownloadResult, Event as RawEvent, FileId, InvertIter, Merge,
    ProgressEntry, PullResult, PullStream, Puller, PullerError, Pusher, Total, UrlInfo, WorkerId,
    file, handle, http, invert, mem, mock, multi, reqwest as reqwest_adapter, single, utils,
};
pub use reqwest;
