use shogi::{MoveError, SfenError};
use std::sync::mpsc::{RecvError, SendError};
use std::{error, fmt, io};

#[derive(Debug)]
pub enum Error {
    Usi(usi::Error),
    Sfen(SfenError),
    Move(MoveError),
    Io(io::Error),
    Channel(Box<dyn error::Error + Send + Sync>),
    EngineNotResponded,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Usi(ref e) => write!(f, "{e}"),
            Error::Sfen(ref e) => write!(f, "{e}"),
            Error::Move(ref e) => write!(f, "{e}"),
            Error::Io(ref e) => write!(f, "{e}"),
            Error::Channel(ref e) => write!(f, "{e}"),
            Error::EngineNotResponded => write!(f, "the engine did not return 'readyok' command"),
        }
    }
}

impl error::Error for Error {
    fn cause(&self) -> Option<&dyn error::Error> {
        match *self {
            Error::Usi(ref e) => Some(e),
            Error::Sfen(ref e) => Some(e),
            Error::Move(ref e) => Some(e),
            Error::Io(ref e) => Some(e),
            Error::Channel(_) => None,
            Error::EngineNotResponded => None,
        }
    }
}

impl<T> From<SendError<T>> for Error
where
    T: Send + Sync + 'static,
{
    fn from(err: SendError<T>) -> Error {
        Error::Channel(Box::new(err))
    }
}

impl From<RecvError> for Error {
    fn from(err: RecvError) -> Error {
        Error::Channel(Box::new(err))
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::Io(err)
    }
}

impl From<usi::Error> for Error {
    fn from(err: usi::Error) -> Error {
        Error::Usi(err)
    }
}

impl From<SfenError> for Error {
    fn from(err: SfenError) -> Error {
        Error::Sfen(err)
    }
}

impl From<MoveError> for Error {
    fn from(err: MoveError) -> Error {
        Error::Move(err)
    }
}
