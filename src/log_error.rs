use failure::{Backtrace, Context, Fail};
use std::fmt::{self, Display};

#[derive(Copy, Clone, Eq, PartialEq, Debug, Fail)]
pub enum LogErrorKind {
    #[fail(display = "A required item could not be found")]
    NotFound,
    #[fail(display = "An duplicate item was encountered where it should be unique")]
    Duplicate,
    #[fail(display = "An error occured in an upstream function")]
    Upstream,
    #[fail(display = "An unknown error occurred")]
    Unknown,
    #[fail(display = "The OS type is not supported")]
    InvOSType,
    #[fail(display = "The function has not been implemented yet")]
    NotImpl,
    #[fail(display = "A command IO stream operation failed")]
    CmdIO,
    #[fail(display = "An invalid value was encountered")]
    InvParam,
    #[fail(display = "An invalid state was encountered")]
    InvState,
    #[fail(display = "A required program could not be found")]
    PgmNotFound,
    #[fail(display = "A required feature is not available")]
    FeatureMissing,
    #[fail(display = "A spawned process returned an error code")]
    ExecProcess,
    #[fail(display = "An error occurred calling a WINAPI")]
    WinApi,
    #[fail(display = "Initialization of WMI")]
    WmiInit,
    #[fail(display = "A WMI query failed")]
    WmiQueryFailed,
    #[fail(display = "A Powershell command failed")]
    PSFailed,
    #[fail(display = "You are not authorized to execute this command")]
    AuthError,
    #[fail(display = "Mutual access failed")]
    MutAccess,
    #[fail(display = "No Match")]
    NoMatch,
}

pub struct LogErrCtx {
    kind: LogErrorKind,
    descr: String,
}

impl LogErrCtx {
    pub fn from_remark(kind: LogErrorKind, descr: &str) -> LogErrCtx {
        LogErrCtx {
            kind,
            descr: String::from(descr),
        }
    }
}

impl From<LogErrorKind> for LogErrCtx {
    fn from(kind: LogErrorKind) -> LogErrCtx {
        LogErrCtx {
            kind,
            descr: String::new(),
        }
    }
}

impl Display for LogErrCtx {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.descr.is_empty() {
            write!(f, "Error: {}", self.kind)
        } else {
            write!(f, "Error: {}, {}", self.kind, self.descr)
        }
    }
}

#[derive(Debug)]
pub struct LogError {
    inner: Context<LogErrCtx>,
}

impl Fail for LogError {
    fn name(&self) -> Option<&str> {
        self.inner.name()
    }

    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut res = Display::fmt(&self.inner, f);
        if let Some(fail) = self.inner.cause() {
            write!(f, " - ")?;
            res = Display::fmt(fail, f);
        }
        res
    }
}

impl LogError {
    pub fn kind(&self) -> LogErrorKind {
        self.inner.get_context().kind
    }

    pub fn from_remark(kind: LogErrorKind, remark: &str) -> LogError {
        LogError {
            inner: Context::new(LogErrCtx::from_remark(kind, remark)),
        }
    }
}

impl From<LogErrorKind> for LogError {
    fn from(kind: LogErrorKind) -> LogError {
        LogError {
            inner: Context::new(LogErrCtx::from(kind)),
        }
    }
}

impl From<LogErrCtx> for LogError {
    fn from(log_ctxt: LogErrCtx) -> LogError {
        LogError {
            inner: Context::new(log_ctxt),
        }
    }
}

impl From<Context<LogErrCtx>> for LogError {
    fn from(inner: Context<LogErrCtx>) -> LogError {
        LogError { inner: inner }
    }
}
