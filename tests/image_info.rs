use kaduceus::{AsyncReader, Info, KakaduContext, KakaduImageReader};

#[test]
fn test_gold_images() {
    tracing_subscriber::fmt::init();

    let rt = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap();

    let file = rt.block_on(async move {
        tokio::fs::File::open("/home/gtierney/Downloads/reference.jp2")
            .await
            .unwrap()
    });

    let ctx = KakaduContext::default();
    let mut reader = KakaduImageReader::new(ctx, file, Some("my_image.jp2".into()));
    let info = reader.info();

    assert_eq!(
        Info {
            width: 2717,
            height: 3701,
            tile_width: 2717,
            tile_height: 3701,
            dwt_levels: 5,
        },
        info
    );
}
