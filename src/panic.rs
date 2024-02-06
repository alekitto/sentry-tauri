//! The Sentry Panic handler integration.
//!
//! The `PanicIntegration`, which is enabled by default in `sentry`, installs a
//! panic handler that will automatically dispatch all errors to Sentry that
//! are caused by a panic.
//! Additionally, panics are forwarded to the previously registered panic hook.
//!
//! # Configuration
//!
//! The panic integration can be configured with an additional extractor, which
//! might optionally create a sentry `Event` out of a `PanicInfo`.
//!
//! ```
//! let integration = sentry_tauri::PanicIntegration::default().add_extractor(|info| None);
//! ```

#![warn(missing_docs)]

use std::io::{Read, Seek};
use std::panic::{self, PanicInfo};
use std::path::PathBuf;
use std::sync::Once;

use sentry::protocol::{Attachment, AttachmentType, Event, Exception, Level, Mechanism};
use sentry::{ClientOptions, Integration};
use sentry_backtrace::current_stacktrace;

fn get_dump_fn() -> PathBuf {
    let pid = std::process::id();
    let mut dump_fn = std::env::temp_dir();
    dump_fn.push(format!("dump_{}.mdmp", pid));

    dump_fn
}

#[cfg(target_os = "linux")]
fn write_minidump() -> Result<(PathBuf, Vec<u8>), Box<dyn std::error::Error>> {
    let mut writer =
        minidump_writer::minidump_writer::MinidumpWriter::new(std::process::id() as _, unsafe {
            libc::syscall(libc::SYS_gettid)
        }
            as i32);

    writer.sanitize_stack();

    let dump_fn = get_dump_fn();
    let mut minidump_file = std::fs::File::create(&dump_fn)?;

    Ok((dump_fn, writer.dump(&mut minidump_file)?))
}

#[cfg(target_os = "macos")]
fn write_minidump() -> Result<(PathBuf, Vec<u8>), Box<dyn std::error::Error>> {
    // Defaults to dumping the current process and thread.
    let mut writer = minidump_writer::minidump_writer::MinidumpWriter::new(None, None);

    let dump_fn = get_dump_fn();
    let mut minidump_file = std::fs::File::create(&dump_fn)?;

    Ok((dump_fn, writer.dump(&mut minidump_file)?))
}

#[cfg(target_os = "windows")]
fn write_minidump() -> Result<(PathBuf, Vec<u8>), Box<dyn std::error::Error>> {
    let dump_fn = get_dump_fn();
    let mut minidump_file = std::fs::File::create(&dump_fn)?;

    // Attempts to write the minidump
    minidump_writer::minidump_writer::MinidumpWriter::dump_local_context(
        // The exception code, presumably one of STATUS_*. Defaults to STATUS_NONCONTINUABLE_EXCEPTION if not specified
        None,
        // If not specified, uses the current thread as the "crashing" thread,
        // so this is equivalent to passing `None`, but it could be any thread
        // in the process
        Some(unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() }),
        None,
        &mut minidump_file,
    )?;

    let mut buf = vec![];
    minidump_file.seek(std::io::SeekFrom::Start(0))?;
    minidump_file.read_to_end(&mut buf)?;

    Ok((dump_fn, buf))
}

/// A panic handler that sends to Sentry.
///
/// This panic handler reports panics to Sentry. It also attempts to prevent
/// double faults in some cases where it's known to be unsafe to invoke the
/// Sentry panic handler.
pub fn panic_handler(info: &PanicInfo<'_>) {
    sentry::with_integration(|integration: &PanicIntegration, hub| {
        hub.with_scope(
            |scope| {
                let Ok((filename, buffer)) = write_minidump() else {
                    return;
                };

                scope.add_attachment(Attachment {
                    buffer,
                    filename: filename.to_string_lossy().to_string(),
                    ty: Some(AttachmentType::Minidump),
                    ..Default::default()
                });
            },
            || {
                hub.capture_event(integration.event_from_panic_info(info));
            },
        );

        if let Some(client) = hub.client() {
            client.flush(None);
        }
    });
}

type PanicExtractor = dyn Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync;

/// The Sentry Panic handler Integration.
#[derive(Default)]
pub struct PanicIntegration {
    extractors: Vec<Box<PanicExtractor>>,
}

impl std::fmt::Debug for PanicIntegration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PanicIntegration")
            .field("extractors", &self.extractors.len())
            .finish()
    }
}

static INIT: Once = Once::new();

#[cfg(unix)]
unsafe extern "C" fn sigsegv_handler(signum: std::ffi::c_int) {
    eprintln!("received signal {}", signum);

    let mut sigs = std::mem::zeroed::<libc::sigset_t>();
    libc::sigemptyset(&mut sigs);
    libc::sigaddset(&mut sigs, signum);
    libc::sigprocmask(libc::SIG_UNBLOCK, &sigs, std::ptr::null_mut());

    panic!("Segmentation fault!");
}

impl Integration for PanicIntegration {
    fn name(&self) -> &'static str {
        "panic"
    }

    fn setup(&self, _cfg: &mut ClientOptions) {
        INIT.call_once(|| {
            let next = panic::take_hook();
            panic::set_hook(Box::new(move |info| {
                panic_handler(info);
                next(info);
            }));

            #[cfg(unix)]
            unsafe {
                let handler = sigsegv_handler as *const fn(std::ffi::c_int);
                libc::signal(libc::SIGSEGV, handler as libc::sighandler_t);
            }
        });
    }
}

/// Extract the message of a panic.
pub fn message_from_panic_info<'a>(info: &'a PanicInfo<'_>) -> &'a str {
    match info.payload().downcast_ref::<&'static str>() {
        Some(s) => s,
        None => match info.payload().downcast_ref::<String>() {
            Some(s) => &s[..],
            None => "Box<Any>",
        },
    }
}

impl PanicIntegration {
    /// Creates a new Panic Integration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a new extractor.
    #[must_use]
    pub fn add_extractor<F>(mut self, f: F) -> Self
    where
        F: Fn(&PanicInfo<'_>) -> Option<Event<'static>> + Send + Sync + 'static,
    {
        self.extractors.push(Box::new(f));
        self
    }

    /// Creates an event from the given panic info.
    ///
    /// The stacktrace is calculated from the current frame.
    pub fn event_from_panic_info(&self, info: &PanicInfo<'_>) -> Event<'static> {
        for extractor in &self.extractors {
            if let Some(event) = extractor(info) {
                return event;
            }
        }

        // TODO: We would ideally want to downcast to `std::error:Error` here
        // and use `event_from_error`, but that way we won‘t get meaningful
        // backtraces yet.

        let msg = message_from_panic_info(info);
        Event {
            exception: vec![Exception {
                ty: "panic".into(),
                mechanism: Some(Mechanism {
                    ty: "panic".into(),
                    handled: Some(false),
                    ..Default::default()
                }),
                value: Some(msg.to_string()),
                stacktrace: current_stacktrace(),
                ..Default::default()
            }]
            .into(),
            level: Level::Fatal,
            ..Default::default()
        }
    }
}
