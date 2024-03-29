use std::error;
use std::fmt::{self, Display, Formatter};
use std::io;
use std::result;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ErrorKind {
    Upstream,
    InvParam,
    InvState,
}

impl Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let output = match *self {
            Self::Upstream => "An error occurred in an upstream function",
            Self::InvParam => "An invalid parameter was encountered",
            Self::InvState => "An invalid state was encountered",
        };
        write!(f, "{}", output)
    }
}

#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    cause: Option<Box<dyn error::Error + Send + Sync + 'static>>,
    context: Option<String>,
}

impl Error {
    pub fn new(kind: ErrorKind) -> Error {
        Error {
            kind,
            cause: None,
            context: None,
        }
    }

    pub fn with_context(kind: ErrorKind, context: &str) -> Error {
        Error {
            kind,
            cause: None,
            context: Some(context.to_owned()),
        }
    }

    pub fn with_cause<E: error::Error + Send + Sync + 'static>(
        kind: ErrorKind,
        cause: Box<E>,
    ) -> Error {
        Error {
            kind,
            cause: Some(cause),
            context: None,
        }
    }

    pub fn with_all<E: error::Error + Send + Sync + 'static>(
        kind: ErrorKind,
        context: &str,
        cause: Box<E>,
    ) -> Error {
        Error {
            kind,
            cause: Some(cause),
            context: Some(context.to_owned()),
        }
    }

    pub fn from_upstream(cause: Error, context: &str) -> Error {
        Error {
            kind: ErrorKind::Upstream,
            cause: Some(Box::new(cause)),
            context: Some(context.to_owned()),
        }
    }

    pub fn from_upstream_error<E: error::Error + Send + Sync + 'static>(
        cause: Box<E>,
        context: &str,
    ) -> Error {
        Error {
            kind: ErrorKind::Upstream,
            cause: Some(cause),
            context: Some(context.to_owned()),
        }
    }

    pub fn kind(&self) -> ErrorKind {
        self.kind
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        match &self.context {
            Some(context) => {
                write!(f, ", context: {}", context)?;
            }
            None => (),
        }
        let mut curr_err: &dyn error::Error = self;

        while let Some(cause) = curr_err.source() {
            write!(f, "\n  caused by: {}", cause)?;
            curr_err = cause;
        }
        Ok(())
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Error::from_upstream_error(Box::new(error), "")
    }
}
impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.cause {
            Some(cause) => Some(&**cause),
            None => None,
        }
    }

    fn cause(&self) -> Option<&dyn error::Error> {
        match &self.cause {
            Some(cause) => Some(&**cause),
            None => None,
        }
    }
}

pub trait ToError<T> {
    fn error(self) -> Result<T>;
    fn upstream_with_context(self, context: &str) -> Result<T>;
    fn error_with_all(self, kind: ErrorKind, context: &str) -> Result<T>;
    fn error_with_kind(self, kind: ErrorKind) -> Result<T>;
}

impl<T, E> ToError<T> for result::Result<T, E>
where
    E: error::Error + Send + Sync + 'static,
{
    fn error(self) -> Result<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(why) => Err(Error::with_cause(ErrorKind::Upstream, Box::new(why))),
        }
    }
    fn error_with_all(self, kind: ErrorKind, context: &str) -> Result<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(why) => Err(Error::with_all(kind, context, Box::new(why))),
        }
    }

    fn error_with_kind(self, kind: ErrorKind) -> Result<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(why) => Err(Error::with_cause(kind, Box::new(why))),
        }
    }
    fn upstream_with_context(self, context: &str) -> Result<T> {
        match self {
            Ok(ok) => Ok(ok),
            Err(why) => Err(Error::with_all(ErrorKind::Upstream, context, Box::new(why))),
        }
    }
}

pub type Result<T> = result::Result<T, Error>;
