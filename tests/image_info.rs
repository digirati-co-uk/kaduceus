use std::path::{Path, PathBuf};

use insta::assert_debug_snapshot;
use kaduceus::{Info, KakaduContext, KakaduImageReader};

fn image_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(name);

    path
}

fn image_info<P: AsRef<Path>>(path: P) -> Info {
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
    let mut reader = KakaduImageReader::new(ctx, file, Some(filename.into()));

    reader.info()
}

fn test_image_info(name: &str) {
    let mut settings = insta::Settings::clone_current();
    settings.set_snapshot_suffix(name);

    let _guard = settings.bind_to_scope();

    let image_path = image_path(name);
    let image_info = image_info(image_path);

    assert_debug_snapshot!(image_info);
}

#[test]
fn test_reference_image() {
    test_image_info("testdata/reference.jp2");
}
