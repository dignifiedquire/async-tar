use std::{error, fmt};

#[cfg(feature = "runtime-smol")]
use futures_lite::io::{self, Error};
#[cfg(feature = "runtime-tokio")]
use tokio::io::{self, Error};

#[derive(Debug)]
pub(crate) struct TarError {
    desc: String,
    io: io::Error,
}

impl TarError {
    pub(crate) fn new(desc: &str, err: Error) -> TarError {
        TarError {
            desc: desc.to_string(),
            io: err,
        }
    }
}

impl error::Error for TarError {
    fn description(&self) -> &str {
        &self.desc
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.io)
    }
}

impl fmt::Display for TarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.desc.fmt(f)
    }
}

impl From<TarError> for Error {
    fn from(t: TarError) -> Error {
        Error::new(t.io.kind(), t)
    }
}
