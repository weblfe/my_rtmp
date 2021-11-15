use crate::filter::LogLineFilter;
use crate::flexi_logger::FlexiLogger;
use crate::formats::default_format;
#[cfg(feature = "atty")]
use crate::formats::AdaptiveFormat;
use crate::primary_writer::PrimaryWriter;
use crate::writers::{FileLogWriter, FileLogWriterBuilder, FlWriteMode, LogWriter};
use crate::{
    Cleanup, Criterion, FileSpec, FlexiLoggerError, FormatFunction, LogSpecification, LoggerHandle,
    Naming, DEFAULT_BUFFER_CAPACITY, DEFAULT_FLUSH_INTERVAL,
};
#[cfg(feature = "async")]
use crate::{DEFAULT_MESSAGE_CAPA, DEFAULT_POOL_CAPA};
#[cfg(feature = "specfile")]
use notify::{watcher, DebouncedEvent, RecursiveMode, Watcher};
use std::collections::HashMap;
#[cfg(feature = "specfile_without_notification")]
use std::io::Read;
#[cfg(feature = "specfile_without_notification")]
use std::path::Path;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// The entry-point for using `flexi_logger`.
///
/// A simple example with file logging might look like this:
///
/// ```rust
/// use flexi_logger::{Duplicate, FileSpec, Logger};
///
/// Logger::try_with_str("info, mycrate = debug")
///         .unwrap()
///         .log_to_file(FileSpec::default())
///         .duplicate_to_stderr(Duplicate::Warn)
///         .start()
///         .unwrap();
/// ```
///
///
/// `Logger` is a builder class that allows you to
/// * specify your desired (initial) loglevel-specification
///   * either programmatically as a String ([`Logger::try_with_str`])
///   * or by providing a String in the environment ([`Logger::try_with_env`]),
///   * or by combining both options ([`Logger::try_with_env_or_str`]),
///   * or by building a [`LogSpecification`] programmatically ([`Logger::with`]),
/// * use the desired configuration methods,
/// * and finally start the logger with
///
///   * [`Logger::start`], or
///   * [`Logger::start_with_specfile`].
pub struct Logger {
    spec: LogSpecification,
    log_target: LogTarget,
    duplicate_err: Duplicate,
    duplicate_out: Duplicate,
    format_for_file: FormatFunction,
    format_for_stderr: FormatFunction,
    format_for_stdout: FormatFunction,
    format_for_writer: FormatFunction,
    #[cfg(feature = "colors")]
    o_palette: Option<String>,
    o_flush_wait: Option<std::time::Duration>,
    flwb: FileLogWriterBuilder,
    other_writers: HashMap<String, Box<dyn LogWriter>>,
    filter: Option<Box<dyn LogLineFilter + Send + Sync>>,
}

enum LogTarget {
    StdErr,
    StdOut,
    Multi(bool, Option<Box<dyn LogWriter>>),
}

/// Create a Logger instance and define how to access the (initial)
/// loglevel-specification.
impl Logger {
    /// Creates a Logger that you provide with an explicit [`LogSpecification`].
    #[must_use]
    pub fn with(logspec: LogSpecification) -> Self {
        Self::from_spec_and_errs(logspec)
    }

    /// Creates a Logger that reads the [`LogSpecification`] from a `String` or `&str`.
    /// See [`LogSpecification`] for the syntax.
    ///
    /// # Errors
    ///
    /// `FlexiLoggerError::Parse` if the String uses an erroneous syntax.
    pub fn try_with_str<S: AsRef<str>>(s: S) -> Result<Self, FlexiLoggerError> {
        Ok(Self::from_spec_and_errs(LogSpecification::parse(
            s.as_ref(),
        )?))
    }

    /// Creates a Logger that reads the [`LogSpecification`] from the environment variable
    /// `RUST_LOG`.
    ///
    /// # Errors
    ///
    /// `FlexiLoggerError::Parse` if the value of `RUST_LOG` uses an erroneous syntax.
    pub fn try_with_env() -> Result<Self, FlexiLoggerError> {
        Ok(Self::from_spec_and_errs(LogSpecification::env()?))
    }

    /// Creates a Logger that reads the [`LogSpecification`] from the environment variable
    /// `RUST_LOG`, or derives it from the given `String`, if `RUST_LOG` is not set.
    ///
    /// # Errors
    ///
    /// `FlexiLoggerError::Parse` if the used String uses an erroneous syntax.
    pub fn try_with_env_or_str<S: AsRef<str>>(s: S) -> Result<Self, FlexiLoggerError> {
        Ok(Self::from_spec_and_errs(LogSpecification::env_or_parse(s)?))
    }

    fn from_spec_and_errs(spec: LogSpecification) -> Self {
        #[cfg(feature = "colors")]
        {
            // Enable ASCII escape sequence support on Windows consoles,
            // but disable coloring on unsupported Windows consoles
            if cfg!(windows) && !yansi::Paint::enable_windows_ascii() {
                yansi::Paint::disable();
            }
        }

        Self {
            spec,
            log_target: LogTarget::StdErr,
            duplicate_err: Duplicate::None,
            duplicate_out: Duplicate::None,
            format_for_file: default_format,

            #[cfg(feature = "colors")]
            format_for_stdout: AdaptiveFormat::Default.format_function(if cfg!(feature = "atty") {
                atty::is(atty::Stream::Stdout)
            } else {
                false
            }),
            #[cfg(feature = "colors")]
            format_for_stderr: AdaptiveFormat::Default.format_function(if cfg!(feature = "atty") {
                atty::is(atty::Stream::Stderr)
            } else {
                false
            }),

            #[cfg(not(feature = "colors"))]
            format_for_stdout: default_format,
            #[cfg(not(feature = "colors"))]
            format_for_stderr: default_format,

            format_for_writer: default_format,
            #[cfg(feature = "colors")]
            o_palette: None,
            o_flush_wait: None,
            flwb: FileLogWriter::builder(FileSpec::default()),
            other_writers: HashMap::<String, Box<dyn LogWriter>>::new(),
            filter: None,
        }
    }
}

/// Simple methods for influencing the behavior of the Logger.
impl Logger {
    /// Log is written to stderr (which is the default).
    #[must_use]
    pub fn log_to_stderr(mut self) -> Self {
        self.log_target = LogTarget::StdErr;
        self
    }

    /// Log is written to stdout.
    #[must_use]
    pub fn log_to_stdout(mut self) -> Self {
        self.log_target = LogTarget::StdOut;
        self
    }

    /// Log is written to a file.
    ///
    ///
    /// The default filename pattern is `<program_name>_<date>_<time>.<suffix>`,
    ///  e.g. `myprog_2015-07-08_10-44-11.log`.
    ///
    /// You can duplicate to stdout and stderr, and you can add additional writers.
    #[must_use]
    pub fn log_to_file(mut self, file_spec: FileSpec) -> Self {
        self.log_target = LogTarget::Multi(true, None);
        self.flwb = self.flwb.file_spec(file_spec);
        self
    }

    /// Log is written to the provided writer.
    ///
    /// You can duplicate to stdout and stderr, and you can add additional writers.
    #[must_use]
    pub fn log_to_writer(mut self, w: Box<dyn LogWriter>) -> Self {
        self.log_target = LogTarget::Multi(false, Some(w));
        self
    }

    /// Log is written to a file, as with [`Logger::log_to_file`], _and_ to an alternative
    /// [`LogWriter`] implementation.
    ///
    /// And you can duplicate to stdout and stderr, and you can add additional writers.
    #[must_use]
    pub fn log_to_file_and_writer(mut self, file_spec: FileSpec, w: Box<dyn LogWriter>) -> Self {
        self.log_target = LogTarget::Multi(true, Some(w));
        self.flwb = self.flwb.file_spec(file_spec);
        self
    }

    /// Log is processed, including duplication, but not written to any destination.
    ///
    /// This can be useful e.g. for running application tests with all log-levels active and still
    /// avoiding tons of log files etc.
    /// Such tests ensure that the log calls which are normally not active
    /// will not cause undesired side-effects when activated
    /// (note that the log macros may prevent arguments of inactive log-calls from being evaluated).
    ///
    /// Or, if you want to get logs both to stdout and stderr, but nowhere else,
    /// then use this option and combine it with
    /// [`Logger::duplicate_to_stdout`] and [`Logger::duplicate_to_stderr`].
    #[must_use]
    pub fn do_not_log(mut self) -> Self {
        self.log_target = LogTarget::Multi(false, None);
        self
    }

    /// Makes the logger print an info message to stdout with the name of the logfile
    /// when a logfile is opened for writing.
    #[must_use]
    pub fn print_message(mut self) -> Self {
        self.flwb = self.flwb.print_message();
        self
    }

    /// Makes the logger write messages with the specified minimum severity additionally to stderr.
    ///
    /// Does not work with [`Logger::log_to_stdout`] or [`Logger::log_to_stderr`].
    #[must_use]
    pub fn duplicate_to_stderr(mut self, dup: Duplicate) -> Self {
        self.duplicate_err = dup;
        self
    }

    /// Makes the logger write messages with the specified minimum severity additionally to stdout.
    ///
    /// Does not work with [`Logger::log_to_stdout`] or [`Logger::log_to_stderr`].
    #[must_use]
    pub fn duplicate_to_stdout(mut self, dup: Duplicate) -> Self {
        self.duplicate_out = dup;
        self
    }

    /// Makes the logger use the provided format function for all messages
    /// that are written to files, stderr, stdout, or to an additional writer.
    ///
    /// You can either choose one of the provided log-line formatters,
    /// or you create and use your own format function with the signature <br>
    /// ```rust
    /// fn my_format(
    ///    write: &mut dyn std::io::Write,
    ///    now: &mut flexi_logger::DeferredNow,
    ///    record: &log::Record,
    /// ) -> std::io::Result<()>
    /// # {unimplemented!("")}
    /// ```
    ///
    /// By default, [`default_format`] is used for output to files and to custom writers,
    /// and [`AdaptiveFormat::Default`] is used for output to `stderr` and `stdout`.
    /// If the feature `colors` is switched off, [`default_format`] is used for all outputs.
    pub fn format(mut self, format: FormatFunction) -> Self {
        self.format_for_file = format;
        self.format_for_stderr = format;
        self.format_for_stdout = format;
        self.format_for_writer = format;
        self
    }

    /// Makes the logger use the provided format function for messages
    /// that are written to files.
    ///
    /// Regarding the default, see [`Logger::format`].
    pub fn format_for_files(mut self, format: FormatFunction) -> Self {
        self.format_for_file = format;
        self
    }

    /// Makes the logger use the provided format function for messages
    /// that are written to stderr.
    ///
    /// Regarding the default, see [`Logger::format`].
    pub fn format_for_stderr(mut self, format_function: FormatFunction) -> Self {
        self.format_for_stderr = format_function;
        self
    }

    /// Makes the logger use the specified format for messages that are written to `stderr`.
    /// Coloring is used if `stderr` is a tty.
    ///
    /// Regarding the default, see [`Logger::format`].
    ///
    /// Only available with feature `colors`.
    #[cfg(feature = "atty")]
    #[must_use]
    pub fn adaptive_format_for_stderr(mut self, adaptive_format: AdaptiveFormat) -> Self {
        #[cfg(feature = "atty")]
        let is_tty = atty::is(atty::Stream::Stderr);
        #[cfg(not(feature = "atty"))]
        let is_tty = false;

        self.format_for_stderr = adaptive_format.format_function(is_tty);
        self
    }

    /// Makes the logger use the provided format function to format messages
    /// that are written to stdout.
    ///
    /// Regarding the default, see [`Logger::format`].
    pub fn format_for_stdout(mut self, format_function: FormatFunction) -> Self {
        self.format_for_stdout = format_function;
        self
    }

    /// Makes the logger use the specified format for messages that are written to `stdout`.
    /// Coloring is used if `stdout` is a tty.
    ///
    /// Regarding the default, see [`Logger::format`].
    ///
    /// Only available with feature `colors`.
    #[cfg(feature = "atty")]
    #[must_use]
    pub fn adaptive_format_for_stdout(mut self, adaptive_format: AdaptiveFormat) -> Self {
        #[cfg(feature = "atty")]
        let is_tty = atty::is(atty::Stream::Stdout);
        #[cfg(not(feature = "atty"))]
        let is_tty = false;

        self.format_for_stdout = adaptive_format.format_function(is_tty);
        self
    }

    /// Allows specifying a format function for an additional writer.
    /// Note that it is up to the implementation of the additional writer
    /// whether it evaluates this setting or not.
    ///
    /// Regarding the default, see [`Logger::format`].
    pub fn format_for_writer(mut self, format: FormatFunction) -> Self {
        self.format_for_writer = format;
        self
    }

    /// Sets the color palette for function [`style`](crate::style), which is used in the
    /// provided coloring format functions.
    ///
    /// The palette given here overrides the default palette.
    ///
    /// The palette is specified in form of a String that contains a semicolon-separated list
    /// of numbers (0..=255) and/or dashes (´-´).
    /// The first five values denote the fixed color that is
    /// used for coloring `error`, `warn`, `info`, `debug`, and `trace` messages.
    ///
    /// The String `"196;208;-;7;8"` describes the default palette, where color 196 is
    /// used for error messages, and so on. The `-` means that no coloring is done,
    /// i.e., with `"-;-;-;-;-"` all coloring is switched off.
    ///
    /// The palette can further be overridden at runtime by setting the environment variable
    /// `FLEXI_LOGGER_PALETTE` to a palette String. This allows adapting the used text colors to
    /// differently colored terminal backgrounds.
    ///
    /// For your convenience, if you want to specify your own palette,
    /// you can produce a colored list with all 255 colors with `cargo run --example colors`.
    ///
    /// Only available with feature `colors`.
    #[cfg(feature = "colors")]
    #[must_use]
    pub fn set_palette(mut self, palette: String) -> Self {
        self.o_palette = Some(palette);
        self
    }

    /// Prevent indefinite growth of the log file by applying file rotation
    /// and a clean-up strategy for older log files.
    ///
    /// By default, the log file is fixed while your program is running and will grow indefinitely.
    /// With this option being used, when the log file reaches the specified criterion,
    /// the file will be closed and a new file will be opened.
    ///
    /// Note that also the filename pattern changes:
    ///
    /// - by default, no timestamp is added to the filename if rotation is used
    /// - the logs are always written to a file with infix `_rCURRENT`
    /// - when the rotation criterion is fulfilled, it is closed and renamed to a file
    ///   with another infix (see `Naming`),
    ///   and then the logging continues again to the (fresh) file with infix `_rCURRENT`.
    ///
    /// Example:
    ///
    /// After some logging with your program `my_prog` and rotation with `Naming::Numbers`,
    /// you will find files like
    ///
    /// ```text
    /// my_prog_r00000.log
    /// my_prog_r00001.log
    /// my_prog_r00002.log
    /// my_prog_rCURRENT.log
    /// ```
    ///
    /// ## Parameters
    ///
    /// `criterion` defines *when* the log file should be rotated, based on its size or age.
    /// See [`Criterion`] for details.
    ///
    /// `naming` defines the naming convention for the rotated log files.
    /// See [`Naming`] for details.
    ///
    /// `cleanup` defines the strategy for dealing with older files.
    /// See [`Cleanup`] for details.
    #[must_use]
    pub fn rotate(mut self, criterion: Criterion, naming: Naming, cleanup: Cleanup) -> Self {
        self.flwb = self.flwb.rotate(criterion, naming, cleanup);
        self
    }

    /// When [`Logger::rotate`] is used with some [`Cleanup`] variant other than [`Cleanup::Never`],
    /// then this method can be used to define
    /// if the cleanup activities (finding files, deleting files, evtl compressing files) are
    /// delegated to a background thread (which is the default,
    /// to minimize the blocking impact to your application caused by IO operations),
    /// or whether they are done synchronously in the current log-call.
    ///
    /// If you call this method with `use_background_thread = false`,
    /// the cleanup is done synchronously.
    #[must_use]
    pub fn cleanup_in_background_thread(mut self, use_background_thread: bool) -> Self {
        self.flwb = self
            .flwb
            .cleanup_in_background_thread(use_background_thread);
        self
    }

    /// Apply the provided filter before really writing log lines.
    ///
    /// See the documentation of module [`filter`](crate::filter) for a usage example.
    #[must_use]
    pub fn filter(mut self, filter: Box<dyn LogLineFilter + Send + Sync>) -> Self {
        self.filter = Some(filter);
        self
    }

    /// Makes the logger append to the specified output file, if it exists already;
    /// by default, the file would be truncated.
    ///
    /// This option only has an effect if logs are written to files, but
    /// it will hardly make an effect if [`FileSpec::suppress_timestamp`] is not used.
    #[must_use]
    pub fn append(mut self) -> Self {
        self.flwb = self.flwb.append();
        self
    }

    /// The specified path will be used on linux systems to create a symbolic link
    /// to the current log file.
    ///
    /// This option has no effect on filesystems where symlinks are not supported,
    /// and it only has an effect if logs are written to files.
    ///
    /// ### Example
    ///
    /// You can use the symbolic link to follow the log output with `tail`,
    /// even if the log files are rotated.
    ///
    /// Assuming you use `create_symlink("link_to_log_file")`, then use:
    ///
    /// ```text
    /// tail --follow=name --max-unchanged-stats=1 --retry link_to_log_file
    /// ```
    ///
    pub fn create_symlink<P: Into<PathBuf>>(mut self, symlink: P) -> Self {
        self.flwb = self.flwb.create_symlink(symlink);
        self
    }

    /// Registers a [`LogWriter`] implementation under the given target name.
    ///
    /// The target name must not start with an underscore.
    /// See module [`writers`](crate::writers) for more details.
    pub fn add_writer<S: Into<String>>(
        mut self,
        target_name: S,
        writer: Box<dyn LogWriter>,
    ) -> Self {
        self.other_writers.insert(target_name.into(), writer);
        self
    }

    /// Sets the write mode for the logger.
    ///
    /// See [`WriteMode`] for more (important!) details.
    #[must_use]
    pub fn write_mode(mut self, write_mode: WriteMode) -> Self {
        self.flwb = self.flwb.write_mode(write_mode.get_fl_write_mode());
        self.o_flush_wait = write_mode.get_duration();
        self
    }

    /// Use Windows line endings, rather than just `\n`.
    #[must_use]
    pub fn use_windows_line_ending(mut self) -> Self {
        self.flwb = self.flwb.use_windows_line_ending();
        self
    }
}

/// Alternative set of methods to control the behavior of the Logger.
/// Use these methods when you want to control the settings flexibly,
/// e.g. with commandline arguments via `docopts` or `clap`.
impl Logger {
    /// With true, makes the logger print an info message to stdout, each time
    /// when a new file is used for log-output.
    #[must_use]
    pub fn o_print_message(mut self, print_message: bool) -> Self {
        self.flwb = self.flwb.o_print_message(print_message);
        self
    }

    /// By default, and with None, the log file will grow indefinitely.
    /// If a `rotate_config` is set, when the log file reaches or exceeds the specified size,
    /// the file will be closed and a new file will be opened.
    /// Also the filename pattern changes: instead of the timestamp, a serial number
    /// is included into the filename.
    ///
    /// The size is given in bytes, e.g. `o_rotate_over_size(Some(1_000))` will rotate
    /// files once they reach a size of 1 kB.
    ///
    /// The cleanup strategy allows delimiting the used space on disk.
    #[must_use]
    pub fn o_rotate(mut self, rotate_config: Option<(Criterion, Naming, Cleanup)>) -> Self {
        self.flwb = self.flwb.o_rotate(rotate_config);
        self
    }

    /// This option only has an effect if `log_to_file` is set to true.
    ///
    /// If append is set to true, makes the logger append to the specified output file, if it exists.
    /// By default, or with false, the file would be truncated.
    ///
    /// This option will hardly make an effect if `suppress_timestamp()` is not used.
    #[must_use]
    pub fn o_append(mut self, append: bool) -> Self {
        self.flwb = self.flwb.o_append(append);
        self
    }

    /// This option only has an effect if `log_to_file` is set to true.
    ///
    /// If a String is specified, it will be used on linux systems to create in the current folder
    /// a symbolic link with this name to the current log file.
    pub fn o_create_symlink<P: Into<PathBuf>>(mut self, symlink: Option<P>) -> Self {
        self.flwb = self.flwb.o_create_symlink(symlink);
        self
    }
}

/// Finally, start logging, optionally with a spec-file.
impl Logger {
    /// Consumes the Logger object and initializes `flexi_logger`.
    ///
    /// **Keep the [`LoggerHandle`] alive up to the very end of your program!**
    /// Dropping the [`LoggerHandle`] flushes and shuts down [`FileLogWriter`]s
    /// and other [`LogWriter`]s, and then may prevent further logging!
    /// This should happen immediately before the program terminates, but not earlier.
    ///
    /// Dropping the [`LoggerHandle`] is uncritical
    /// only with [`Logger::log_to_stdout`] or [`Logger::log_to_stderr`].
    ///
    /// The [`LoggerHandle`] also allows updating the log specification programmatically,
    /// e.g. to intensify logging for (buggy) parts of a (test) program, etc.
    ///
    /// # Example
    ///
    /// ```rust
    /// use flexi_logger::{Logger,WriteMode, FileSpec};
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let _logger = Logger::try_with_str("info")?
    ///         .log_to_file(FileSpec::default())
    ///         .write_mode(WriteMode::BufferAndFlush)
    ///         .start()?;
    ///
    ///     // ... do all your work and join back all threads whose logs you want to see ...
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Several variants of [`FlexiLoggerError`] can occur.
    pub fn start(self) -> Result<LoggerHandle, FlexiLoggerError> {
        let (boxed_logger, handle) = self.build()?;
        log::set_boxed_logger(boxed_logger)?;
        Ok(handle)
    }

    /// Builds a boxed logger and a `LoggerHandle` for it,
    /// but does not initialize the global logger.
    ///
    /// The returned boxed logger implements the [`Log`](log::Log) trait
    /// and can be installed manually or nested within another logger.
    ///
    /// **Keep the [`LoggerHandle`] alive up to the very end of your program!**
    /// See [`Logger::start`] for more details.
    ///
    /// # Errors
    ///
    /// Several variants of [`FlexiLoggerError`] can occur.
    pub fn build(self) -> Result<(Box<dyn log::Log>, LoggerHandle), FlexiLoggerError> {
        #[cfg(feature = "colors")]
        crate::formats::set_palette(&self.o_palette)?;

        let a_primary_writer = Arc::new(match self.log_target {
            LogTarget::StdOut => {
                PrimaryWriter::stdout(self.format_for_stdout, &self.flwb.buffersize())
            }
            LogTarget::StdErr => {
                PrimaryWriter::stderr(self.format_for_stderr, &self.flwb.buffersize())
            }
            LogTarget::Multi(use_file, mut o_writer) => PrimaryWriter::multi(
                self.duplicate_err,
                self.duplicate_out,
                self.format_for_stderr,
                self.format_for_stdout,
                if use_file {
                    Some(Box::new(
                        self.flwb.format(self.format_for_file).try_build()?,
                    ))
                } else {
                    None
                },
                {
                    if let Some(ref mut writer) = o_writer {
                        writer.format(self.format_for_writer);
                    }
                    o_writer
                },
            ),
        });

        let a_other_writers = Arc::new(self.other_writers);

        if let Some(wait_time) = self.o_flush_wait {
            let pw = Arc::clone(&a_primary_writer);
            let ows = Arc::clone(&a_other_writers);
            std::thread::Builder::new()
                .name("flexi_logger-flusher".to_string())
                .stack_size(128)
                .spawn(move || {
                    let (_sender, receiver): (Sender<()>, Receiver<()>) = channel();
                    loop {
                        receiver.recv_timeout(wait_time).ok();
                        pw.flush().ok();
                        for w in ows.values() {
                            w.flush().ok();
                        }
                    }
                })?;
        }

        let max_level = self.spec.max_level();
        let a_l_spec = Arc::new(RwLock::new(self.spec));

        let flexi_logger = FlexiLogger::new(
            Arc::clone(&a_l_spec),
            Arc::clone(&a_primary_writer),
            Arc::clone(&a_other_writers),
            self.filter,
        );

        let handle = LoggerHandle::new(a_l_spec, a_primary_writer, a_other_writers);
        handle.reconfigure(max_level);
        Ok((Box::new(flexi_logger), handle))
    }

    /// Consumes the Logger object and initializes `flexi_logger` in a way that
    /// subsequently the log specification can be updated,
    /// while the program is running, by editing a file.
    ///
    /// Uses the spec that was given to the factory method ([`Logger::with`] etc)
    /// as initial spec and then tries to read the logspec from a file.
    ///
    /// If the file does not exist, `flexi_logger` creates the file and fills it
    /// with the initial spec (and in the respective file format, of course).
    ///
    /// **Keep the returned [`LoggerHandle`] alive up to the very end of your program!**
    /// See [`Logger::start`] for more details.
    ///
    /// # Feature dependency
    ///
    /// The implementation of this configuration method uses some additional crates
    /// that you might not want to depend on with your program if you don't use this functionality.
    /// For that reason the method is only available if you activate the
    /// `specfile` feature. See the usage section on
    /// [crates.io](https://crates.io/crates/flexi_logger) for details.
    ///
    /// # Usage
    ///
    /// A logger initialization like
    ///
    /// ```rust,no_run
    /// use flexi_logger::Logger;
    /// Logger::try_with_str("info")
    ///     .unwrap()
    ///     // more logger configuration
    ///     .start_with_specfile("logspecification.toml");
    /// ```
    ///
    /// will create the file `logspecification.toml` (if it does not yet exist) with this content:
    ///
    /// ```toml
    /// ### Optional: Default log level
    /// global_level = 'info'
    /// ### Optional: specify a regular expression to suppress all messages that don't match
    /// #global_pattern = 'foo'
    ///
    /// ### Specific log levels per module are optionally defined in this section
    /// [modules]
    /// #'mod1' = 'warn'
    /// #'mod2' = 'debug'
    /// #'mod2::mod3' = 'trace'
    /// ```
    ///
    /// You can subsequently edit and modify the file according to your needs,
    /// while the program is running, and it will immediately take your changes into account.
    ///
    /// Currently only toml-files are supported, the file suffix thus must be `.toml`.
    ///
    /// The initial spec remains valid if the file cannot be read.
    ///
    /// If you update the specfile subsequently while the program is running, `flexi_logger`
    /// re-reads it automatically and adapts its behavior according to the new content.
    /// If the file cannot be read anymore, e.g. because the format is not correct, the
    /// previous logspec remains active.
    /// If the file is corrected subsequently, the log spec update will work again.
    ///
    /// # Errors
    ///
    /// Several variants of [`FlexiLoggerError`] can occur.
    #[cfg(feature = "specfile_without_notification")]
    pub fn start_with_specfile<P: AsRef<Path>>(
        self,
        specfile: P,
    ) -> Result<LoggerHandle, FlexiLoggerError> {
        // Make logging work, before caring for the specfile
        let (boxed_logger, handle) = self.build()?;
        log::set_boxed_logger(boxed_logger)?;
        setup_specfile(specfile, handle.clone())?;
        Ok(handle)
    }

    /// Builds a boxed logger and a `LoggerHandle` for it,
    /// but does not initialize the global logger.
    ///
    /// See also [`Logger::start`] and [`Logger::start_with_specfile`].
    /// for the properties of the returned logger.
    ///
    /// # Errors
    ///
    /// Several variants of [`FlexiLoggerError`] can occur.
    #[cfg(feature = "specfile_without_notification")]
    pub fn build_with_specfile<P: AsRef<Path>>(
        self,
        specfile: P,
    ) -> Result<(Box<dyn log::Log>, LoggerHandle), FlexiLoggerError> {
        let (boxed_log, handle) = self.build()?;
        setup_specfile(specfile, handle.clone())?;
        Ok((boxed_log, handle))
    }
}

#[cfg(feature = "specfile_without_notification")]
fn setup_specfile<P: AsRef<Path>>(
    specfile: P,
    mut handle: LoggerHandle,
) -> Result<(), FlexiLoggerError> {
    let specfile = specfile.as_ref().to_owned();
    synchronize_handle_with_specfile(&mut handle, &specfile)?;

    #[cfg(feature = "specfile")]
    {
        // Now that the file exists, we can canonicalize the path
        let specfile = specfile
            .canonicalize()
            .map_err(FlexiLoggerError::SpecfileIo)?;

        // Watch the parent folder of the specfile, using debounced events
        let (tx, rx) = std::sync::mpsc::channel();
        let debouncing_delay = std::time::Duration::from_millis(1000);
        let mut watcher = watcher(tx, debouncing_delay)?;
        watcher.watch(&specfile.parent().unwrap(), RecursiveMode::NonRecursive)?;

        // in a separate thread, reread the specfile when it was updated
        std::thread::Builder::new()
            .name("flexi_logger-specfile-watcher".to_string())
            .stack_size(128 * 1024)
            .spawn(move || {
                let _anchor_for_watcher = watcher; // keep it alive!
                loop {
                    match rx.recv() {
                        Ok(debounced_event) => match debounced_event {
                            DebouncedEvent::Create(ref path) | DebouncedEvent::Write(ref path) => {
                                if path.canonicalize().map(|x| x == specfile).unwrap_or(false) {
                                    match log_spec_string_from_file(&specfile)
                                        .map_err(FlexiLoggerError::SpecfileIo)
                                        .and_then(|s| LogSpecification::from_toml(&s))
                                    {
                                        Ok(spec) => handle.set_new_spec(spec),
                                        Err(e) => eprintln!(
                                            "[flexi_logger] rereading the log specification file \
                                             failed with {:?}, \
                                             continuing with previous log specification",
                                            e
                                        ),
                                    }
                                }
                            }
                            _event => {}
                        },
                        Err(e) => {
                            eprintln!("[flexi_logger] error while watching the specfile: {:?}", e)
                        }
                    }
                }
            })?;
    }
    Ok(())
}

// If the specfile exists, read the file and update the log_spec from it;
// otherwise try to create the file, with the current spec as content, under the specified name.
#[cfg(feature = "specfile_without_notification")]
pub(crate) fn synchronize_handle_with_specfile(
    handle: &mut LoggerHandle,
    specfile: &Path,
) -> Result<(), FlexiLoggerError> {
    if specfile
        .extension()
        .unwrap_or_else(|| std::ffi::OsStr::new(""))
        .to_str()
        .unwrap_or("")
        != "toml"
    {
        return Err(FlexiLoggerError::SpecfileExtension(
            "only spec files with extension toml are supported",
        ));
    }

    if Path::is_file(specfile) {
        let s = log_spec_string_from_file(specfile).map_err(FlexiLoggerError::SpecfileIo)?;
        handle.set_new_spec(LogSpecification::from_toml(&s)?);
    } else {
        if let Some(specfolder) = specfile.parent() {
            std::fs::DirBuilder::new()
                .recursive(true)
                .create(specfolder)
                .map_err(FlexiLoggerError::SpecfileIo)?;
        }
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(specfile)
            .map_err(FlexiLoggerError::SpecfileIo)?;

        handle
            .current_spec()
            .read()
            .map_err(|_e| FlexiLoggerError::Poison)?
            .to_toml(&mut file)?;
    }
    Ok(())
}

#[cfg(feature = "specfile_without_notification")]
pub(crate) fn log_spec_string_from_file<P: AsRef<Path>>(
    specfile: P,
) -> Result<String, std::io::Error> {
    let mut buf = String::new();
    let mut file = std::fs::File::open(specfile)?;
    file.read_to_string(&mut buf)?;
    Ok(buf)
}

/// Used to control which messages are to be duplicated to stderr, when `log_to_file()` is used.
#[derive(Debug)]
pub enum Duplicate {
    /// No messages are duplicated.
    None,
    /// Only error messages are duplicated.
    Error,
    /// Error and warn messages are duplicated.
    Warn,
    /// Error, warn, and info messages are duplicated.
    Info,
    /// Error, warn, info, and debug messages are duplicated.
    Debug,
    /// All messages are duplicated.
    Trace,
    /// All messages are duplicated.
    All,
}

/// Describes if the log output should be written synchronously or asynchronously,
/// and if and how file I/O should be buffered and flushed.
///
/// Is used in [`Logger::write_mode`].
///
/// Using buffering reduces the program's I/O overhead, and thus increases overall performance,
/// which can become relevant if logging is used heavily.
/// On the other hand, if logging is used with low frequency,
/// buffering can defer the appearance of log lines significantly,
/// so regular flushing is usually advisable with buffering.
///
/// **Note** that for all options except `Direct` you should keep the [`LoggerHandle`] alive
/// up to the very end of your program to ensure that all buffered log lines are flushed out
/// (which happens automatically when the [`LoggerHandle`] is dropped)
/// before the program terminates.
/// [See here for an example](code_examples/index.html#choose-the-write-mode).
///
/// **Note** further that flushing uses an extra thread (with minimal stack).
#[derive(Copy, Clone)]
pub enum WriteMode {
    /// Do not buffer (default).
    ///
    /// Every log line is directly written to the output, without buffering.
    /// This allows seeing new log lines in real time, and does not need additional threads.
    Direct,

    /// Buffer with default capacity ([`DEFAULT_BUFFER_CAPACITY`])
    /// and flush with default interval ([`DEFAULT_FLUSH_INTERVAL`]).
    BufferAndFlush,

    /// Buffer and  flush with given buffer capacity and flush interval.
    BufferAndFlushWith(usize, Duration),

    /// Lets the `FileLogWriter` send logs through an unbounded channel to an output thread, which
    /// does the file output, the rotation, and the cleanup.
    ///
    /// Uses buffered output to reduce overhead, and a bounded message pool to reduce allocations.
    /// The log output is flushed regularly with the given interval.
    ///
    /// See [here](code_examples/index.html#choose-the-write-mode) for an example.
    ///
    /// Only available with feature `async`.
    #[cfg(feature = "async")]
    Async,

    /// Like Async, but allows using non-default parameter values.
    ///
    /// Only available with feature `async`.
    #[cfg(feature = "async")]
    AsyncWith {
        /// Size of the output buffer for the file.
        bufsize: usize,
        /// Capacity of the pool for the message buffers.
        pool_capa: usize,
        /// Capacity of an individual message buffer.
        message_capa: usize,
        /// The interval for flushing the output.
        flush_interval: Duration,
    },

    /// Buffer, but don't flush.
    ///
    /// This might be handy if you want to minimize I/O but don't want to create
    /// the extra thread for flushing and don't care if log lines appear with delay.
    BufferDontFlush,
}
impl WriteMode {
    fn get_fl_write_mode(&self) -> FlWriteMode {
        match self {
            Self::Direct => FlWriteMode::DontBuffer,
            Self::BufferDontFlush | Self::BufferAndFlush => {
                FlWriteMode::Buffer(DEFAULT_BUFFER_CAPACITY)
            }
            Self::BufferAndFlushWith(bufsize, _) => FlWriteMode::Buffer(*bufsize),
            #[cfg(feature = "async")]
            Self::Async => FlWriteMode::BufferAsync(
                DEFAULT_BUFFER_CAPACITY,
                DEFAULT_POOL_CAPA,
                DEFAULT_MESSAGE_CAPA,
            ),
            #[cfg(feature = "async")]
            Self::AsyncWith {
                bufsize,
                pool_capa,
                message_capa,
                flush_interval: _,
            } => FlWriteMode::BufferAsync(*bufsize, *pool_capa, *message_capa),
        }
    }
    fn get_duration(&self) -> Option<Duration> {
        #[allow(clippy::match_same_arms)]
        match self {
            Self::Direct | Self::BufferDontFlush => None,
            Self::BufferAndFlush => Some(DEFAULT_FLUSH_INTERVAL),
            Self::BufferAndFlushWith(_, flush_interval) => Some(*flush_interval),
            #[cfg(feature = "async")]
            Self::Async => Some(DEFAULT_FLUSH_INTERVAL),
            #[cfg(feature = "async")]
            Self::AsyncWith {
                bufsize: _,
                pool_capa: _,
                message_capa: _,
                flush_interval,
            } => Some(*flush_interval),
        }
    }
}
