#![warn(
    clippy::all,
    clippy::perf,
    clippy::style,
    clippy::panic,
    clippy::unwrap_used
)]
use std::{error::Error, sync::Arc};
use std::{io::SeekFrom, pin::Pin};

use ffi::LogLevel;
use futures::{AsyncRead, AsyncReadExt, AsyncSeek, AsyncSeekExt};
use tokio::runtime::Runtime;
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

        type CxxKakaduDecompressor;
        fn finish(self: Pin<&mut CxxKakaduDecompressor>, error_code: &mut i32) -> bool;
        fn process(
            self: Pin<&mut CxxKakaduDecompressor>,
            data: &mut [u8],
            output_region: &mut Region,
        ) -> bool;

        type CxxKakaduContext;
        fn create_kakadu_context() -> SharedPtr<CxxKakaduContext>;

        type CxxKakaduImage;
        fn create_kakadu_image_reader(
            ctx: SharedPtr<CxxKakaduContext>,
            reader: Box<AsyncReader>,
        ) -> UniquePtr<CxxKakaduImage>;

        fn info(self: Pin<&mut CxxKakaduImage>) -> Info;

        /// Opens the given [Region] of interest for decompression.
        fn open(
            self: Pin<&mut CxxKakaduImage>,
            roi: &Region,
            scaled_width: u32,
            scaled_height: u32,
        ) -> UniquePtr<CxxKakaduDecompressor>;
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

#[allow(dead_code)]
pub struct KakaduDecompressor {
    pub(crate) inner: cxx::UniquePtr<ffi::CxxKakaduDecompressor>,
}

impl KakaduDecompressor {
    pub(crate) fn new(inner: cxx::UniquePtr<ffi::CxxKakaduDecompressor>) -> KakaduDecompressor {
        Self { inner }
    }

    pub fn process(&mut self, data: &mut [u8]) -> Result<Region, Box<dyn Error + 'static>> {
        let mut decompressor = self.inner.pin_mut();
        let mut region = Region::default();
        let incomplete = decompressor.as_mut().process(data, &mut region);

        if incomplete {
            return Ok(region);
        } else {
            let mut _error_code = 0i32;
            if decompressor.finish(&mut _error_code) {
                return Ok(region);
            } else {
                return Err("Kakadu Decompression error".into());
            }
        }
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
    ) -> KakaduImage {
        let span = info_span!("image_reader", image_name = image_name);
        let input_reader = Box::new(AsyncReader::new(executor, stream, span.clone()));
        let inner = ffi::create_kakadu_image_reader(ctx.inner, input_reader);

        Self {
            span: span.clone(),
            inner,
        }
    }

    #[tracing::instrument(parent=self.span.clone(), skip(self))]
    pub fn open_region(
        &mut self,
        region: Region,
        scaled_width: u32,
        scaled_height: u32,
    ) -> KakaduDecompressor {
        let inner_decompressor = self
            .inner
            .pin_mut()
            .open(&region, scaled_width, scaled_height);

        KakaduDecompressor::new(inner_decompressor)
    }

    pub fn info(&mut self) -> ffi::Info {
        self.span.in_scope(|| {
            let inner_span = info_span!("image_info");
            inner_span.in_scope(|| self.inner.pin_mut().info())
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

impl Drop for AsyncReader {
    fn drop(&mut self) {
        println!("drop called")
    }
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
                let avail = self.stream.read(&mut buffer[read..]).await?;
                read += avail;
                info!(avail, "read cycle");

                if avail == 0 {
                    break;
                }
            }
            Ok(read)
        };

        self.executor
            .block_on(task)
            .inspect(|size| info!(size, "read fulfilled"))
            .inspect_err(|err| error!(?err, "read failed"))
            .map(|size| if size == 0 { 0 } else { size as isize })
    }

    #[tracing::instrument(parent=self.reader_span.clone(), skip(self))]
    pub fn seek(&mut self, offset: i64) -> std::io::Result<u64> {
        let task = async { self.stream.seek(SeekFrom::Start(offset as u64)).await };

        self.executor
            .block_on(task)
            .inspect(|size| info!(size, "seek fulfilled"))
            .inspect_err(|err| error!(?err, "seek failed"))
    }
}
