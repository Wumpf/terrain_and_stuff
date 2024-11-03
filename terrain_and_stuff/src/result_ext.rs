pub trait ResultExt<T> {
    fn ok_or_log(self, context: &str) -> Option<T>;
}

impl<T, E: std::error::Error> ResultExt<T> for Result<T, E> {
    fn ok_or_log(self, context: &str) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(err) => {
                log::error!("{context}, {}", err);
                None
            }
        }
    }
}
