use crate::primary_writer::PrimaryWriter;
use crate::writers::{FileLogWriterBuilder, LogWriter};
use crate::{FlexiLoggerError, LogSpecification};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Shuts down the logger when dropped, and allows reconfiguring the logger programmatically.
///
/// A `LoggerHandle` is returned from `Logger::start()` and from `Logger::start_with_specfile()`.
/// When the logger handle is dropped, then it shuts down the Logger!
/// This matters if you use one of `Logger::log_to_file`, `Logger::log_to_writer`, or
/// `Logger::log_to_file_and_writer`. It is then important to keep the logger handle alive
/// until the very end of your program!
///
/// `LoggerHandle` offers methods to modify the log specification programmatically,
/// to flush() the logger explicitly, and even to reconfigure the used `FileLogWriter` --
/// if one is used.
///
/// # Examples
///
/// Since dropping the `LoggerHandle` has no effect if you use
/// `Logger::log_to_stderr` (which is the default) or `Logger::log_to_stdout`.
/// you can then safely ignore the return value of `Logger::start()`:
///
/// ```rust
/// # use flexi_logger::Logger;
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
///     Logger::try_with_str("info")?
///         .start()?;
///     // ...
/// # Ok(())
/// # }
/// ```
///
/// When logging to a file or another writer, keep the `LoggerHandle` alive until the program ends:
///
/// ```rust
/// use flexi_logger::{FileSpec, Logger};
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let _logger = Logger::try_with_str("info")?
///         .log_to_file(FileSpec::default())
///         .start()?;
///
///     // do work
///     Ok(())
/// }
/// ```
///
/// You can use the logger handle to permanently exchange the log specification programmatically,
/// anywhere in your code:
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut logger = flexi_logger::Logger::try_with_str("info")?
///         .start()
///         .unwrap();
///     // ...
///     logger.parse_new_spec("warn");
///     // ...
///     # Ok(())
/// # }
/// ```
///
/// However, when debugging, you often want to modify the log spec only temporarily, for  
/// one or few method calls only; this is easier done with the following method, because
/// it allows switching back to the previous spec:
///
/// ```rust
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// #    let mut logger = flexi_logger::Logger::try_with_str("info")?
/// #        .start()?;
/// logger.parse_and_push_temp_spec("trace");
/// // ...
/// // critical calls
/// // ...
/// logger.pop_temp_spec();
/// // Continue with the log spec you had before.
/// // ...
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct LoggerHandle {
    spec: Arc<RwLock<LogSpecification>>,
    spec_stack: Vec<LogSpecification>,
    primary_writer: Arc<PrimaryWriter>,
    other_writers: Arc<HashMap<String, Box<dyn LogWriter>>>,
}

impl LoggerHandle {
    pub(crate) fn new(
        spec: Arc<RwLock<LogSpecification>>,
        primary_writer: Arc<PrimaryWriter>,
        other_writers: Arc<HashMap<String, Box<dyn LogWriter>>>,
    ) -> Self {
        Self {
            spec,
            spec_stack: Vec::default(),
            primary_writer,
            other_writers,
        }
    }

    #[cfg(feature = "specfile_without_notification")]
    pub(crate) fn current_spec(&self) -> Arc<RwLock<LogSpecification>> {
        Arc::clone(&self.spec)
    }

    //
    pub(crate) fn reconfigure(&self, mut max_level: log::LevelFilter) {
        for w in self.other_writers.as_ref().values() {
            max_level = std::cmp::max(max_level, w.max_log_level());
        }
        log::set_max_level(max_level);
    }

    /// Replaces the active `LogSpecification`.
    #[allow(clippy::missing_panics_doc)]
    pub fn set_new_spec(&mut self, new_spec: LogSpecification) {
        let max_level = new_spec.max_level();
        self.spec.write().unwrap(/* catch and expose error? */).update_from(new_spec);
        self.reconfigure(max_level);
    }

    /// Tries to replace the active `LogSpecification` with the result from parsing the given String.
    ///
    /// # Errors
    ///
    /// [`FlexiLoggerError::Parse`] if the input is malformed.
    pub fn parse_new_spec(&mut self, spec: &str) -> Result<(), FlexiLoggerError> {
        self.set_new_spec(LogSpecification::parse(spec)?);
        Ok(())
    }

    /// Replaces the active `LogSpecification` and pushes the previous one to a Stack.
    #[allow(clippy::missing_panics_doc)]
    pub fn push_temp_spec(&mut self, new_spec: LogSpecification) {
        self.spec_stack
            .push(self.spec.read().unwrap(/* catch and expose error? */).clone());
        self.set_new_spec(new_spec)
    }

    /// Tries to replace the active `LogSpecification` with the result from parsing the given String
    ///  and pushes the previous one to a Stack.
    ///
    /// # Errors
    ///
    /// [`FlexiLoggerError::Parse`] if the input is malformed.
    pub fn parse_and_push_temp_spec<S: AsRef<str>>(
        &mut self,
        new_spec: S,
    ) -> Result<(), FlexiLoggerError> {
        self.spec_stack.push(
            self.spec
                .read()
                .map_err(|_| FlexiLoggerError::Poison)?
                .clone(),
        );
        self.set_new_spec(LogSpecification::parse(new_spec)?);
        Ok(())
    }

    /// Reverts to the previous `LogSpecification`, if any.
    pub fn pop_temp_spec(&mut self) {
        if let Some(previous_spec) = self.spec_stack.pop() {
            self.set_new_spec(previous_spec);
        }
    }

    /// Flush all writers.
    pub fn flush(&self) {
        self.primary_writer.flush().ok();
        for writer in self.other_writers.values() {
            writer.flush().ok();
        }
    }

    /// Replaces parts of the configuration of the file log writer.
    ///
    /// Note that neither the write mode nor the format function can be reset and
    /// that the provided `FileLogWriterBuilder` must have the same values for these as the
    /// currently used `FileLogWriter`.
    ///
    /// # Errors
    ///
    /// `FlexiLoggerError::Reset` if no file log writer is configured,
    ///  or if a reset was tried with a different write mode.
    /// `FlexiLoggerError::Io` if the specified path doesn't work.
    /// `FlexiLoggerError::Poison` if some mutex is poisoned.
    pub fn reset_flw(&self, flwb: &FileLogWriterBuilder) -> Result<(), FlexiLoggerError> {
        if let PrimaryWriter::Multi(ref mw) = &*self.primary_writer {
            mw.reset_file_log_writer(flwb)
        } else {
            Err(FlexiLoggerError::Reset)
        }
    }

    /// Shutdown all participating writers.
    ///
    /// This method is supposed to be called at the very end of your program, if
    ///
    /// - you use some [`Cleanup`](crate::Cleanup) strategy with compression:
    ///   then you want to ensure that a termination of your program
    ///   does not interrput the cleanup-thread when it is compressing a log file,
    ///   which could leave unexpected files in the filesystem
    /// - you use your own writer(s), and they need to clean up resources
    ///
    /// See also [`writers::LogWriter::shutdown`](crate::writers::LogWriter::shutdown).
    pub fn shutdown(&self) {
        self.primary_writer.shutdown();
        for writer in self.other_writers.values() {
            writer.shutdown();
        }
    }

    // Allows checking the logs written so far to the writer
    #[doc(hidden)]
    pub fn validate_logs(&self, expected: &[(&'static str, &'static str, &'static str)]) {
        self.primary_writer.validate_logs(expected)
    }
}

impl Drop for LoggerHandle {
    fn drop(&mut self) {
        self.primary_writer.shutdown();
        for writer in self.other_writers.values() {
            writer.shutdown();
        }
    }
}
