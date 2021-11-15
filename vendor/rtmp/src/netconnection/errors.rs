use {
    crate::amf0::errors::{Amf0ReadError, Amf0WriteError},
    failure::{Backtrace, Fail},
    std::fmt,
};

#[derive(Debug)]
pub struct NetConnectionError {
    pub value: NetConnectionErrorValue,
}
#[derive(Debug, Fail)]
pub enum NetConnectionErrorValue {
    #[fail(display = "amf0 write error: {}\n", _0)]
    Amf0WriteError(Amf0WriteError),
    #[fail(display = "amf0 read error: {}\n", _0)]
    Amf0ReadError(Amf0ReadError),
}

impl From<Amf0WriteError> for NetConnectionError {
    fn from(error: Amf0WriteError) -> Self {
        NetConnectionError {
            value: NetConnectionErrorValue::Amf0WriteError(error),
        }
    }
}

impl From<Amf0ReadError> for NetConnectionError {
    fn from(error: Amf0ReadError) -> Self {
        NetConnectionError {
            value: NetConnectionErrorValue::Amf0ReadError(error),
        }
    }
}

impl fmt::Display for NetConnectionError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.value, f)
    }
}

impl Fail for NetConnectionError {
    fn cause(&self) -> Option<&dyn Fail> {
        self.value.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.value.backtrace()
    }
}
