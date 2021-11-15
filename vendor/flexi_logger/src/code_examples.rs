//! Here are some examples for the `flexi_logger` initialization.
//!
//! ## Contents
//!
//! - [Start minimal: Write logs to stderr](#start-minimal-write-logs-to-stderr)
//! - [Choose the log output channel](#choose-the-log-output-channel)
//! - [Choose the write mode](#choose-the-write-mode)
//! - [Influence the location and name of the log file](#influence-the-location-and-name-of-the-log-file)
//! - [Specify the format for the log lines explicitly](#specify-the-format-for-the-log-lines-explicitly)
//! - [Use a fixed log file, and truncate or append the file on each program start](#use-a-fixed-log-file-and-truncate-or-append-the-file-on-each-program-start)
//! - [Rotate the log file](#rotate-the-log-file)
//! - [Reconfigure the log specification programmatically](#reconfigure-the-log-specification-programmatically)
//! - [Reconfigure the log specification dynamically by editing a spec-file](#reconfigure-the-log-specification-dynamically-by-editing-a-spec-file)
//!
//!
//! ## Start minimal: Write logs to stderr
//!
//! Choose one of three options to specify which log output you want to see, and call start:
//!
//! - Use [`Logger::try_with_env`](crate::Logger::try_with_env) to
//!   provide the log specification in the environment variable `RUST_LOG`:
//!
//!   ```rust
//!   # use flexi_logger::{Logger,FlexiLoggerError};
//!   # fn main() -> Result<(), FlexiLoggerError> {
//!   Logger::try_with_env()?.start()?;
//!   # Ok(())}
//!   ```
//!
//!   Note that if `RUST_LOG` is not set, or if its value cannot be interpreted, nothing is logged.
//!
//! - Use [`Logger::try_with_str`](crate::Logger::try_with_str) to
//!   provide the log specification programmatically:
//!
//!   ```rust
//!   # use flexi_logger::{Logger,FlexiLoggerError};
//!   # fn main() -> Result<(), FlexiLoggerError> {
//!   Logger::try_with_str("info")?.start()?;
//!   # Ok(())}
//!   ```
//!
//! - or use [`Logger::try_with_env_or_str`](crate::Logger::try_with_env_or_str) to
//!   combine both options:
//!
//!   ```rust
//!   # use flexi_logger::{Logger,FlexiLoggerError};
//!   # fn main() -> Result<(), FlexiLoggerError> {
//!   Logger::try_with_env_or_str("info")?.start()?;
//!   # Ok(())}
//!   ```
//!
//! After that, you just use the log-macros from the log crate.
//!
//! ## Choose the log output channel
//!
//! By default, logs are written to `stderr`.
//! With one of
//! [`Logger::log_to_stdout`](crate::Logger::log_to_stdout),
//! [`Logger::log_to_file`](crate::Logger::log_to_file),
//! [`Logger::log_to_writer`](crate::Logger::log_to_writer),
//! [`Logger::log_to_file_and_writer`](crate::Logger::log_to_file_and_writer),
//! or [`Logger::do_not_log`](crate::Logger::do_not_log),
//! you can send the logs to other destinations, or write them not at all.
//!
//! When writing to files or to a writer,
//! you sometimes want to see some parts of the log additionally on the terminal;
//! this can be achieved with
//! [`Logger::duplicate_to_stderr`](crate::Logger::duplicate_to_stderr) or
//! [`Logger::duplicate_to_stdout`](crate::Logger::duplicate_to_stdout),
//! which duplicate log messages to the terminal.
//!
//! ```rust
//! # use flexi_logger::{Logger,Duplicate, FileSpec};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! Logger::try_with_str("info")?
//!     .log_to_file(FileSpec::default())         // write logs to file
//!     .duplicate_to_stderr(Duplicate::Warn)     // print warnings and errors also to the console
//!     .start()?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Choose the write mode
//!
//! By default, every log line is directly written to the output, without buffering.
//! This allows seeing new log lines in real time.
//!
//! With [`Logger::write_mode`](crate::Logger::write_mode)
//! you have some options to change this behavior, e.g.
//! - with [`WriteMode::BufferAndFlush`](crate::WriteMode::BufferAndFlush),
//!   you can reduce the program's I/O overhead and thus increase overall performance,
//!   which can be relevant if logging is used heavily.
//!   In addition, to keep a short maximum wait time
//!   until a log line is visible in the output channel,
//!   an extra thread is created that flushes the buffers regularly.
//!
//!   ```rust
//!   # use flexi_logger::{WriteMode,FileSpec,Logger,Duplicate};
//!   fn main() -> Result<(), Box<dyn std::error::Error>> {
//!       let _logger = Logger::try_with_str("info")?
//!          .log_to_file(FileSpec::default())
//!          .write_mode(WriteMode::BufferAndFlush)
//!          .start()?;
//!       // ... do all your work ...
//!       Ok(())
//!   }
//!   ```
//!
//! - with [`WriteMode::Async`](crate::WriteMode::Async), the `FileLogWriter` (!)
//!   - send logs from your application threads through an unbounded channel to an output thread,
//!     which does the file output, the rotation, and the cleanup.
//!   - additionally uses buffered output, and a bounded message pool to reduce allocations,
//!     and flushing.
//!   If duplication is used, the messages to `stdout` or `stderr` are written synchronously.
//!
//!   ```rust
//!   # use flexi_logger::{WriteMode, Duplicate, FileSpec, Logger};
//!   fn main() -> Result<(), Box<dyn std::error::Error>> {
//!   # #[cfg(feature="async")]
//!       let _logger = Logger::try_with_str("info")?
//!          .log_to_file(FileSpec::default())
//!          .write_mode(WriteMode::Async)
//!          .start()?;
//!       // ... do all your work ...
//!       Ok(())
//!   }
//!   ```
//!
//! **Note** that with all write modes
//! except [`WriteMode::Direct`](crate::WriteMode::Direct) (which is the default)
//! you should keep the [`LoggerHandle`](crate::LoggerHandle) alive
//! up to the very end of your program,
//! because it will, in its Drop implementation, flush all writers
//! to ensure that all buffered log lines are flushed before the program terminates,
//! and then it calls their shutdown method.
//!
//! ## Influence the location and name of the log file
//!
//! By default, the log files are created in the current directory (where the program was started).
//! With [`FileSpec:directory`](crate::FileSpec::directory)
//! you can specify a concrete folder in which the files should be created.
//!
//! Using [`FileSpec::discriminant`](crate::FileSpec::discriminant)
//! you can add a discriminating infix to the log file name.
//!
//! With [`FileSpec::suffix`](crate::FileSpec::suffix)
//! you can change the suffix that is used for the log files.
//!
//! When writing to files, especially when they are in a distant folder, you may want to let the
//! user know where the log file is.
//!
//! [`Logger::print_message`](crate::Logger::print_message)
//! prints an info to `stdout` to which file the log is written.
//!
//! [`Logger::create_symlink`](crate::Logger::create_symlink)
//! creates (on unix-systems only) a symbolic link at the specified path that points to the log file.
//!
//! ```rust
//! # use flexi_logger::{FileSpec,Logger};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! Logger::try_with_str("info")?
//!     .log_to_file(
//!         FileSpec::default()
//!             .directory("traces")             // create files in folder ./traces
//!             .basename("foo")
//!             .discriminant("Sample4711A")     // use infix in log file name
//!             .suffix("trc")                   // use suffix .trc instead of .log
//!     )
//!     .print_message()                         //
//!     .create_symlink("current_run")           // create a symbolic link to the current log file
//!     .start()?;
//! # Ok(())
//! # }
//! ```
//!
//! This example will print a message like
//! "Log is written to `./traces/foo_Sample4711A_2020-11-17_19-24-35.trc`"
//! and, on unix, create a symbolic link called `current_run`.
//!
//! ## Specify the format for the log lines explicitly
//!
//! With [`Logger::format`](crate::Logger::format)
//! you set the format for all used output channels of `flexi_logger`.
//!
//! `flexi_logger` provides a couple of format functions, and you can also create and use your own,
//! e.g. by copying and modifying one of the provided format functions.
//!
//! Depending on the configuration, `flexi_logger` can write logs to multiple channels
//! (stdout, stderr, files, or additional writers)
//! at the same time. You can control the format for each output channel individually, using
//! [`Logger::format_for_files`](crate::Logger::format_for_files),
//! [`Logger::format_for_stderr`](crate::Logger::format_for_stderr),
//! [`Logger::format_for_stdout`](crate::Logger::format_for_stdout), or
//! [`Logger::format_for_writer`](crate::Logger::format_for_writer).
//!
//!  As argument for these functions you can use one of the provided non-coloring format functions
//!
//!  - [`default_format`](crate::default_format)
//!  - [`detailed_format`](crate::detailed_format)
//!  - [`opt_format`](crate::opt_format)
//!  - [`with_thread`](crate::with_thread),
//!
//! or one of their coloring pendants
//!
//!  - [`colored_default_format`](crate::colored_default_format)
//!  - [`colored_detailed_format`](crate::colored_detailed_format)
//!  - [`colored_opt_format`](crate::colored_opt_format)
//!  - [`colored_with_thread`](crate::colored_with_thread),
//!
//! or your own method.
//!
//! ### Adaptive Coloring
//!
//! You can use coloring for `stdout` and/or `stderr`
//! _conditionally_, such that colors
//!
//! * are used when the output goes to a tty,
//! * are suppressed when you e.g. pipe the output to some other program.
//!
//! You achieve that
//! by providing one of the variants of [`AdaptiveFormat`](crate::AdaptiveFormat) to the respective
//! format method, e.g.
//! ```rust
//! # #[cfg(feature = "atty")]
//! # use flexi_logger::AdaptiveFormat;
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # #[cfg(feature = "atty")]
//! # {
//!       flexi_logger::Logger::try_with_str("info")?
//!           .adaptive_format_for_stderr(AdaptiveFormat::Detailed);
//! # }
//! #     Ok(())
//! # }
//! ```
//!
//! ### Defaults
//!
//! `flexi_logger` initializes by default equivalently to this:
//!
//! ```rust
//! # #[cfg(feature = "atty")]
//! # mod example {
//! # use flexi_logger::{Logger,AdaptiveFormat,default_format, FileSpec};
//! # use log::{debug, error, info, trace, warn};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! # Logger::try_with_str("info")?      // Write all error, warn, and info messages
//! #     .log_to_file(FileSpec::default().directory(std::env::temp_dir()))
//!       // ...
//!       .adaptive_format_for_stderr(AdaptiveFormat::Default)
//!       .adaptive_format_for_stdout(AdaptiveFormat::Default)
//!       .format_for_files(default_format)
//!       .format_for_writer(default_format)
//! #     .start()?;
//! # error!("This is an error message");
//! # warn!("This is a warning");
//! # info!("This is an info message");
//! # debug!("This is a debug message - you must not see it!");
//! # trace!("This is a trace message - you must not see it!");
//! #  run()
//! # }
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {Ok(())}
//! # }
//! ```
//!
//! ## Use a fixed log file, and truncate or append the file on each program start
//!
//! With [`Logger::log_to_file`](crate::Logger::log_to_file) and without rotation,
//! `flexi_logger` uses by default files with a timestamp in the name, like
//! `foo_2020-11-16_08-37-44.log` (for a program called `foo`), which are quite unique for each
//! program start.
//!
//! With [`FileSpec::suppress_timestamp`](crate::FileSpec::suppress_timestamp)
//! you get a simple fixed filename, like `foo.log`.
//!
//! In that case, a restart of the program will truncate an existing log file.
//!
//! Use additionally [`Logger::append`](crate::Logger::append)
//! to append the logs of each new run to the existing file.
//!
//! ```rust
//! # use flexi_logger::{FileSpec, Logger};
//! # use log::{debug, error, info, trace, warn};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! Logger::try_with_str("info")? // Write all error, warn, and info messages
//!     // use a simple filename without a timestamp
//!     .log_to_file(
//!         FileSpec::default().suppress_timestamp()
//! #           .directory(std::env::temp_dir())
//!     )
//!     // do not truncate the log file when the program is restarted
//!     .append()
//!     .start()?;
//!
//! # error!("This is an error message");
//! # warn!("This is a warning");
//! # info!("This is an info message");
//! # debug!("This is a debug message - you must not see it!");
//! # trace!("This is a trace message - you must not see it!");
//! #  run()
//! # }
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {Ok(())}
//! ```
//!
//! ## Rotate the log file
//!
//! With rotation, the logs are always written to a file
//! with the infix `rCURRENT`, like e.g. `foo_rCURRENT.log`.
//!
//! [`Logger::rotate`](crate::Logger::rotate)
//! takes three enum arguments that define its behavior:
//!
//! - [`Criterion`](crate::Criterion)
//!    - with [`Criterion::Age`](crate::Criterion::Age) the rotation happens
//!      when the clock switches to a new day, hour, minute, or second
//!    - with [`Criterion::Size`](crate::Criterion::Size) the rotation happens
//!      when the current log file exceeds the specified limit
//!    - with [`Criterion::AgeOrSize`](crate::Criterion::AgeOrSize) the rotation happens
//!      when either of the two limits is reached
//!
//! - [`Naming`](crate::Naming)<br>The current file is then renamed
//!   - with [`Naming::Timestamps`](crate::Naming::Timestamps) to something
//!     like `foo_r2020-11-16_08-56-52.log`
//!   - with [`Naming::Numbers`](crate::Naming::Numbers) to something like `foo_r00000.log`
//!
//!   and a fresh `rCURRENT` file is created.
//!
//! - [`Cleanup`](crate::Cleanup) defines if and how you
//!   avoid accumulating log files indefinitely:
//!   - with [`Cleanup::KeepLogFiles`](crate::Cleanup::KeepLogFiles) you specify
//!     the number of log files that should be retained;
//!     if there are more, the older ones are getting deleted
//!   - with [`Cleanup::KeepCompressedFiles`](crate::Cleanup::KeepCompressedFiles) you specify
//!     the number of log files that should be
//!     retained, and these are being compressed additionally
//!   - with [`Cleanup::KeepLogAndCompressedFiles`](crate::Cleanup::KeepLogAndCompressedFiles)
//!     you specify the number of log files that should be
//!     retained as is, and an additional number that are being compressed
//!   - with [`Cleanup::Never`](crate::Cleanup::Never) no cleanup is done, all files are retained.
//!
//! ```rust
//! # use flexi_logger::{Age, Cleanup, Criterion, FileSpec, Logger, Naming};
//! # use log::{debug, error, info, trace, warn};
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! Logger::try_with_str("info")?      // Write all error, warn, and info messages
//!     .log_to_file(
//!         FileSpec::default()
//! #           .directory(std::env::temp_dir())
//!     )
//!     .rotate(                      // If the program runs long enough,
//!         Criterion::Age(Age::Day), // - create a new file every day
//!         Naming::Timestamps,       // - let the rotated files have a timestamp in their name
//!         Cleanup::KeepLogFiles(7), // - keep at most 7 log files
//!     )
//!     .start()?;
//!
//! #   error!("This is an error message");
//! #   warn!("This is a warning");
//! #   info!("This is an info message");
//! #   debug!("This is a debug message - you must not see it!");
//! #   trace!("This is a trace message - you must not see it!");
//! #    run()
//! # }
//! # fn run() -> Result<(), Box<dyn std::error::Error>> {Ok(())}
//! ```
//!
//! ## Reconfigure the log specification programmatically
//!
//! This can be especially handy in debugging situations where you want to see
//! traces only for a short instant.
//!
//! Obtain the [`LoggerHandle`](crate::LoggerHandle)
//!
//! ```rust
//! # use flexi_logger::Logger;
//! let mut logger = Logger::try_with_str("info").unwrap()
//!     // ... logger configuration ...
//!     .start()
//!     .unwrap();
//! ```
//!
//! and modify the effective log specification from within your code:
//!
//! ```rust, ignore
//! # use flexi_logger::Logger;
//! # let mut logger = Logger::try_with_str("info").unwrap().start().unwrap();
//! // ...
//! logger.parse_and_push_temp_spec("info, critical_mod = trace");
//! // ... critical calls ...
//! logger.pop_temp_spec();
//! // ... continue with the log spec you had before.
//! ```
//!
//! ## Reconfigure the log specification dynamically by editing a spec-file
//!
//! If you start `flexi_logger` with a specfile,
//!
//! ```rust
//! # use flexi_logger::Logger;
//! # let logger =
//! Logger::try_with_str("info").unwrap()
//!     // ... logger configuration ...
//! # ;
//! # #[cfg(feature = "specfile")]
//! # logger
//!    .start_with_specfile("/server/config/logspec.toml")
//!    .unwrap();
//! ```
//!
//! then you can change the log specification dynamically, *while your program is running*,
//! by editing the specfile. This can be a great help e.g. if you want to get detailed traces
//! for _some_ requests to a long running server.
//!
//! See [`Logger::start_with_specfile`](crate::Logger::start_with_specfile)
//! for more information.
//!
//! ## Miscellaneous
//!
//! For the sake of completeness, we refer here to some more configuration methods.
//! See their documentation for more details.
//!
//! [`Logger::set_palette`](crate::Logger::set_palette)
//!
//! [`Logger::cleanup_in_background_thread`](crate::Logger::cleanup_in_background_thread)
//!
//! [`Logger::use_windows_line_ending`](crate::Logger::use_windows_line_ending)
//!
//! [`Logger::add_writer`](crate::Logger::add_writer)
//!
//! [`Logger::add_writer`](crate::LoggerHandle::reset_flw)
