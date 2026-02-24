use std::fmt::Debug;

pub trait Err2Str<T> {
    fn err2str(self) -> Result<T, String>;
}

impl<T, E: Debug> Err2Str<T> for Result<T, E> {
    fn err2str(self) -> Result<T, String> {
        self.map_err(|e| format!("{:?}", e))
    }
}
