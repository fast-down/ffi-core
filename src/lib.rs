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

use tokio_util::sync::CancellationToken;

pub type Tx = crossfire::Tx<crossfire::spsc::List<Event>>;
pub type Rx = crossfire::AsyncRx<crossfire::spsc::List<Event>>;

#[must_use]
pub fn create_channel() -> (Tx, Rx) {
    crossfire::spsc::unbounded_async()
}

#[must_use]
pub fn create_cancellation_token() -> CancellationToken {
    CancellationToken::new()
}
