use std::fmt::Debug;

use fast_down::{ProgressEntry, UrlInfo};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum Event {
    PrefetchError(String),
    Prefetch(UrlInfo),
    Pulling(usize),
    PullError(usize, String),
    PullTimeout(usize),
    PullProgress(usize, ProgressEntry),
    PushError(usize, String),
    PushProgress(usize, ProgressEntry),
    FlushError(String),
    Finished(usize),
    End { is_cancelled: bool },
}

impl<RE: Debug, WE: Debug> From<&fast_down::Event<RE, WE>> for Event {
    fn from(event: &fast_down::Event<RE, WE>) -> Self {
        match event {
            fast_down::Event::Pulling(id) => Self::Pulling(*id),
            fast_down::Event::PullError(id, e) => Self::PullError(*id, format!("{e:?}")),
            fast_down::Event::PullTimeout(id) => Self::PullTimeout(*id),
            fast_down::Event::PullProgress(id, range) => Self::PullProgress(*id, range.clone()),
            fast_down::Event::PushError(id, e) => Self::PushError(*id, format!("{e:?}")),
            fast_down::Event::PushProgress(id, range) => Self::PushProgress(*id, range.clone()),
            fast_down::Event::FlushError(e) => Self::FlushError(format!("{e:?}")),
            fast_down::Event::Finished(id) => Self::Finished(*id),
        }
    }
}
