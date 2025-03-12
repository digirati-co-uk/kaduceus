use std::{
    error::Error,
    path::{Path, PathBuf},
};

use image::{ImageBuffer, Pixel, Rgba};
use kaduceus::{KakaduContext, KakaduImage, Region};
use tokio_util::compat::TokioAsyncReadCompatExt;

fn image_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(name);

    path
}

fn decompress_image<P: AsRef<Path>>(path: P) -> Result<(), Box<dyn Error>> {
    let path = path.as_ref();
    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("test must not be run from an async context");

    let filename = path
        .file_name()
        .map(|str| str.to_string_lossy())
        .unwrap_or("unknown".into());

    let file = rt.block_on(async move { tokio::fs::File::open(path).await.unwrap() });

    let ctx = KakaduContext::default();
    let mut reader = KakaduImage::new(ctx, file.compat(), Some(filename.into()));
    let info = reader.info();
    let region = Region {
        x: 0,
        y: 0,
        width: info.width,
        height: info.height,
    };

    let mut data = vec![0; (region.width * region.height) as usize];
    let mut decompressor = reader.open_region(region);
    let finished = decompressor.process(&mut data[..])?;
    let bytes: Vec<u8> = data.iter().flat_map(|e| i32::to_le_bytes(*e)).collect();

    let buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(info.width, info.height, &bytes[..]).unwrap();
    let mut output = std::fs::File::create("output.png").unwrap();
    buffer.write_to(&mut output, image::ImageFormat::Png)?;
    Ok(())
}

#[test]
fn test_reference_image() {
    tracing_subscriber::fmt::init();

    decompress_image("testdata/reference.jp2").expect("failed");
}
