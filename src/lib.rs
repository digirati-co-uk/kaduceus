use cxx::UniquePtr;
use ffi::{Info, LogLevel, Region};
use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek};
use tokio::runtime::Builder;
use tracing::{debug, error, info, info_span, span, trace, warn, Instrument, Level};

#[cxx::bridge(namespace = "digirati::kdurs")]
pub mod ffi {
    #[derive(Debug, Eq, PartialEq)]
    enum LogLevel {
        Debug,
        Info,
        Warning,
        Error,
    }

    #[derive(Debug, Eq, PartialEq)]
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
        fn read(&mut self, buffer: &mut [u8]) -> Result<usize>;

        fn log(level: LogLevel, message: &str);
    }

    // C++ types and signatures exposed to Rust.
    unsafe extern "C++" {
        include!("kdurs/src/kakadu_rs.h");

        type KakaduDecompressor;

        type KakaduContext;
        fn create_kakadu_context() -> SharedPtr<KakaduContext>;

        type KakaduImageReader;
        fn create_kakadu_image_reader(
            ctx: SharedPtr<KakaduContext>,
            reader: Box<AsyncReader>,
        ) -> UniquePtr<KakaduImageReader>;

        fn info(self: Pin<&mut KakaduImageReader>) -> Info;

        fn decompress(
            self: Pin<&mut KakaduImageReader>,
            roi: &Region,
        ) -> UniquePtr<KakaduDecompressor>;
        // fn pull_stripes(self: Pin<&mut KakaduImageReader>) -> Result<usize>;
    }
}

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
    pub(crate) inner: cxx::SharedPtr<ffi::KakaduContext>,
}

impl Default for KakaduContext {
    fn default() -> Self {
        Self {
            inner: ffi::create_kakadu_context(),
        }
    }
}

pub struct KakaduDecompressor {
    pub(crate) inner: cxx::UniquePtr<ffi::KakaduDecompressor>,
    pub(crate) span: tracing::Span,
}

impl KakaduDecompressor {
    pub(crate) fn new(
        inner: cxx::UniquePtr<ffi::KakaduDecompressor>,
        span: tracing::Span,
    ) -> KakaduDecompressor {
        Self { inner, span }
    }
}

pub struct KakaduImageReader {
    pub(crate) inner: cxx::UniquePtr<ffi::KakaduImageReader>,
    pub(crate) span: tracing::Span,
}

impl KakaduImageReader {
    pub fn new(
        ctx: KakaduContext,
        input: impl AsyncRead + 'static,
        image_name: Option<String>,
    ) -> KakaduImageReader {
        let span = info_span!("image_reader", image_name = image_name);
        let read_span = info_span!(parent: &span, "read", requested = tracing::field::Empty);
        let input_reader = Box::new(AsyncReader::new(input, read_span));
        let inner = ffi::create_kakadu_image_reader(ctx.inner, input_reader);

        Self { span, inner }
    }

    pub fn decompress(&mut self, region: Region) -> KakaduDecompressor {
        self.span.in_scope(|| {
            let inner_span = info_span!("decompress", region = ?region);
            let inner_decompressor = self.inner.pin_mut().decompress(&region);

            KakaduDecompressor::new(inner_decompressor, inner_span)
        })
    }

    pub fn info(&mut self) -> ffi::Info {
        self.span.in_scope(|| {
            let inner_span = info_span!("image_info");
            inner_span.in_scope(|| self.inner.pin_mut().info())
        })
    }
}

pub struct AsyncReader {
    stream: Pin<Box<dyn AsyncRead>>,
    span: tracing::Span,
}

impl AsyncReader {
    pub fn new<R: AsyncRead + 'static>(source: R, span: tracing::Span) -> Self {
        Self {
            stream: Box::pin(source),
            span,
        }
    }
}

impl AsyncReader {
    pub fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.span.in_scope(|| {
            self.span.record("requested", buffer.len());

            let rt = Builder::new_current_thread()
                .build()
                .expect("failed to construct lightweight runtime");

            let task = async { self.stream.read(buffer).await }.instrument(self.span.clone());

            rt.block_on(task)
                .inspect(|size| info!(size, "read fulfilled"))
                .inspect_err(|err| error!(?err, "read failed"))
        })
    }
}
