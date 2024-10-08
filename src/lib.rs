use std::pin::Pin;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeek};
use tokio::runtime::Builder;

#[cxx::bridge(namespace = "digirati::kdurs")]
mod ffi {
    // Rust types and signatures exposed to C++.
    extern "Rust" {
        type AsyncReader;
        fn read(&mut self, buffer: &mut [u8]) -> Result<usize>;
    }

    // C++ types and signatures exposed to Rust.
    unsafe extern "C++" {
        include!("kdurs/src/kakadu_rs.h");

        type Jp2Decompressor;
    }
}

pub struct AsyncReader {
    stream: Pin<Box<dyn AsyncRead>>,
}

impl AsyncReader {
    pub fn new<R: AsyncRead + 'static>(source: R) -> Self {
        Self {
            stream: Box::pin(source),
        }
    }
}

impl AsyncReader {
    pub fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let task = async { self.stream.read(buffer).await };

        let rt = Builder::new_current_thread()
            .build()
            .expect("failed to construct runtime");
        rt.block_on(task)
    }
}
