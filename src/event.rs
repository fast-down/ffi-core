use std::fmt::Debug;

use fast_down::ProgressEntry;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Event {
    PrefectError(String),
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
            fast_down::Event::Pulling(id) => Event::Pulling(*id),
            fast_down::Event::PullError(id, e) => Event::PullError(*id, format!("{e:?}")),
            fast_down::Event::PullTimeout(id) => Event::PullTimeout(*id),
            fast_down::Event::PullProgress(id, range) => Event::PullProgress(*id, range.clone()),
            fast_down::Event::PushError(id, e) => Event::PushError(*id, format!("{e:?}")),
            fast_down::Event::PushProgress(id, range) => Event::PushProgress(*id, range.clone()),
            fast_down::Event::FlushError(e) => Event::FlushError(format!("{e:?}")),
            fast_down::Event::Finished(id) => Event::Finished(*id),
        }
    }
}
