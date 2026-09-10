#![warn(
    clippy::all,
    clippy::perf,
    clippy::style,
    clippy::panic,
    clippy::unwrap_used
)]
use std::{error::Error, io::ErrorKind, prelude::rust_2024::Future, sync::Arc};
use std::{io::SeekFrom, pin::Pin};

use ffi::LogLevel;
use futures::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};
use tokio::runtime::Runtime;
use tokio_util::bytes::BytesMut;
use tracing::{debug, error, info, info_span, trace, warn};

#[cxx::bridge(namespace = "digirati::kaduceus")]
#[allow(warnings)]
mod ffi {
    #[derive(Debug, Eq, PartialEq)]
    enum LogLevel {
        Debug,
        Info,
        Warning,
        Error,
    }

    #[derive(Debug, Eq, PartialEq, Default)]
    struct Region {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    }

    #[derive(Debug, Eq, PartialEq)]
    struct Info {
        width: u32,
        height: u32,
        tile_width: u32,
        tile_height: u32,
        dwt_levels: u32,
    }

    // Rust types and signatures exposed to C++.
    extern "Rust" {
        type AsyncReader;
        fn read(&mut self, buffer: &mut [u8]) -> Result<isize>;
        fn seek(&mut self, offset: i64) -> Result<u64>;

        fn log(level: LogLevel, message: &str);
    }

    // C++ types and signatures exposed to Rust.
    unsafe extern "C++" {
        include!("kaduceus/src/kaduceus.h");

        // type CxxKakaduCompressedSourceNotifier;
        // fn notify(self: Pin<&mut CxxKakaduCompressedSourceNotifier>);
        // fn notify_unblocked(self: Pin<&mut CxxKakaduCompressedSourceNotifier>);

        type CxxKakaduDecompressor;
        fn finish(self: Pin<&mut CxxKakaduDecompressor>, error_code: &mut i32) -> Result<bool>;
        fn process(
            self: Pin<&mut CxxKakaduDecompressor>,
            data: &mut [u8],
            output_region: &mut Region,
        ) -> Result<bool>;

        type CxxKakaduContext;
        fn create_kakadu_context() -> SharedPtr<CxxKakaduContext>;

        type CxxKakaduImage;
        fn create_kakadu_image_reader(
            ctx: SharedPtr<CxxKakaduContext>,
            reader: Box<AsyncReader>,
        ) -> Result<UniquePtr<CxxKakaduImage>>;

        fn info(self: Pin<&mut CxxKakaduImage>) -> Result<Info>;

        /// Opens the given [Region] of interest for decompression.
        fn open(
            self: Pin<&mut CxxKakaduImage>,
            roi: &Region,
            scaled_width: u32,
            scaled_height: u32,
        ) -> Result<UniquePtr<CxxKakaduDecompressor>>;
    }
}

pub use ffi::{Info, Region};

unsafe impl Sync for ffi::CxxKakaduContext {}
unsafe impl Send for ffi::CxxKakaduContext {}

pub fn log(level: LogLevel, message: &str) {
    match level {
        LogLevel::Debug => debug!(target: "kakadu", message),
        LogLevel::Info => info!(target: "kakadu", message),
        LogLevel::Warning => warn!(target: "kakadu", message),
        LogLevel::Error => error!(target: "kakadu", message),
        _ => trace!(target: "kakadu", message),
    };
}

#[derive(Clone)]
pub struct KakaduContext {
    pub(crate) inner: cxx::SharedPtr<ffi::CxxKakaduContext>,
}

impl Default for KakaduContext {
    fn default() -> Self {
        Self {
            inner: ffi::create_kakadu_context(),
        }
    }
}

/// A decode in progress.
///
/// ⚠️ **`finish()` must run before the C++ destructor.** Kakadu requires the decompressor to be
/// finished — outstanding thread work retired — before its thread group is torn down; destroying an
/// unfinished decompressor trips an internal assertion in `kdu_thread_context::leave_group` ("a
/// group left with a lock still held"). Under concurrency that aborts the process.
///
/// Previously `finish()` was called only in the branch of `process()` where the decode reported
/// itself complete. Every other exit — an error out of `process()`, an error raised by the caller,
/// a decode abandoned part-way — dropped the `UniquePtr` and ran `~CxxKakaduDecompressor` with no
/// `finish()` at all. The `Drop` below closes that gap, which makes this type behave like
/// Cantaloupe's `AutoCloseable` reader: ordered teardown on *every* path, not just the happy one.
#[allow(dead_code)]
pub struct KakaduDecompressor {
    pub(crate) inner: cxx::UniquePtr<ffi::CxxKakaduDecompressor>,
    /// Set the moment a finish is *attempted*, not when one succeeds — a failed finish must never
    /// be retried, least of all from `drop()`.
    finished: bool,
}

impl KakaduDecompressor {
    pub(crate) fn new(inner: cxx::UniquePtr<ffi::CxxKakaduDecompressor>) -> KakaduDecompressor {
        Self {
            inner,
            finished: false,
        }
    }

    /// Has this decode completed (or been finished explicitly)?
    ///
    /// Callers should prefer this to inferring completion from an empty `Region`: an empty region
    /// is a *proxy* for the completion flag, and the two need not coincide.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Retire the decode. Idempotent, and safe to call before dropping.
    pub fn finish(&mut self) -> Result<(), Box<dyn Error + 'static>> {
        self.finish_once()
    }

    fn finish_once(&mut self) -> Result<(), Box<dyn Error + 'static>> {
        if self.finished || self.inner.is_null() {
            return Ok(());
        }
        self.finished = true;
        let mut error_code = 0i32;
        if self.inner.pin_mut().finish(&mut error_code)? {
            Ok(())
        } else {
            Err(format!("Kakadu decompression error (code {error_code})").into())
        }
    }

    pub fn process(&mut self, data: &mut [u8]) -> Result<Region, Box<dyn Error + 'static>> {
        // ⚠️ Calling into a finished C++ decompressor may abort the process (see
        // `examples/probe.rs`). Turn that into an ordinary Rust error instead.
        if self.finished {
            return Err("process() called on a decompressor that has already finished".into());
        }

        let mut region = Region::default();
        let incomplete = self.inner.pin_mut().process(data, &mut region)?;

        if incomplete {
            return Ok(region);
        }

        // Complete: finish now, while we are still on a path that can report an error properly.
        self.finish_once()?;
        Ok(region)
    }
}

impl Drop for KakaduDecompressor {
    fn drop(&mut self) {
        // The safety net for every path `process()` does not reach: errors, early returns,
        // abandonment. `finish_once` is idempotent and marks itself before calling through, so a
        // failing finish cannot loop.
        //
        // ⚠️ A panic in `drop` during unwinding aborts the process, so the result is deliberately
        // discarded — by the time we are here there is nobody left to report it to.
        let _ = self.finish_once();
    }
}

pub struct KakaduImage {
    pub(crate) inner: cxx::UniquePtr<ffi::CxxKakaduImage>,
    pub(crate) span: tracing::Span,
}

unsafe impl Send for KakaduImage {}

impl KakaduImage {
    pub fn new(
        executor: Arc<Runtime>,
        ctx: KakaduContext,
        stream: impl AsyncSeekableRead + 'static,
        image_name: Option<String>,
    ) -> Result<KakaduImage, Box<dyn Error + 'static>> {
        let span = info_span!("image_reader", image_name = image_name);
        let input_reader = Box::new(AsyncReader::new(executor, stream, span.clone()));
        let inner = ffi::create_kakadu_image_reader(ctx.inner, input_reader)?;

        Ok(Self {
            span: span.clone(),
            inner,
        })
    }

    #[tracing::instrument(parent=self.span.clone(), skip(self))]
    pub fn open_region(
        &mut self,
        region: Region,
        scaled_width: u32,
        scaled_height: u32,
    ) -> Result<KakaduDecompressor, Box<dyn Error + 'static>> {
        let inner_decompressor = self
            .inner
            .pin_mut()
            .open(&region, scaled_width, scaled_height)?;

        Ok(KakaduDecompressor::new(inner_decompressor))
    }

    pub fn info(&mut self) -> Result<ffi::Info, Box<dyn Error + 'static>> {
        self.span.in_scope(|| {
            let inner_span = info_span!("image_info");
            inner_span.in_scope(|| Ok(self.inner.pin_mut().info()?))
        })
    }
}

pub trait AsyncSeekableRead: AsyncRead + AsyncSeek {}
impl<T: AsyncRead + AsyncSeek> AsyncSeekableRead for T {}

pub struct AsyncReader {
    executor: Arc<Runtime>,
    stream: Pin<Box<dyn AsyncSeekableRead>>,
    reader_span: tracing::Span,
}

impl AsyncReader {
    pub fn new<R: AsyncRead + AsyncSeek + 'static>(
        executor: Arc<Runtime>,
        source: R,
        reader_span: tracing::Span,
    ) -> Self {
        Self {
            executor,
            stream: Box::pin(source),
            reader_span,
        }
    }
}

impl AsyncReader {
    #[tracing::instrument(parent=self.reader_span.clone(), skip(self, buffer))]
    pub fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<isize> {
        info!(buffer_size = buffer.len(), "read requested");

        let task = async {
            let mut read = 0;

            while read < buffer.len() {
                let avail = match self.stream.read(&mut buffer[read..]).await {
                    Ok(v) => v,
                    Err(e) if e.kind() == ErrorKind::UnexpectedEof => break,
                    Err(e) => return Err(e),
                };

                read += avail;

                if avail == 0 {
                    break;
                }
            }

            Ok(read)
        };

        self.executor.block_on(task).map(|size| size as _)
    }

    #[tracing::instrument(parent=self.reader_span.clone(), skip(self))]
    pub fn seek(&mut self, offset: i64) -> std::io::Result<u64> {
        // Reject negative offsets rather than casting them. `offset as u64` turns -1 into
        // u64::MAX, which `SeekFrom::Start` accepts without complaint; every subsequent read then
        // returns 0 bytes and the decoder is quietly fed a truncated codestream. An error here is
        // recoverable, a silent short read is not.
        if offset < 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("negative seek offset: {offset}"),
            ));
        }

        self.executor
            .block_on(self.stream.seek(SeekFrom::Start(offset as u64)))
    }
}

pub trait AsyncCompressedSource {
    fn read_at<'a, 'b>(
        &'a mut self,
        pos: u64,
        buffer: &'b mut [u8],
    ) -> impl Future<Output = usize> + 'b
    where
        'a: 'b;
}

pub struct TestCompressedSource {
    inner: Vec<u8>,
}

impl AsyncCompressedSource for TestCompressedSource {
    fn read_at<'a, 'b>(
        &'a mut self,
        pos: u64,
        buffer: &'b mut [u8],
    ) -> impl Future<Output = usize> + 'b
    where
        'a: 'b,
    {
        async move {
            buffer.copy_from_slice(&self.inner);
            buffer.len()
        }
    }
}
