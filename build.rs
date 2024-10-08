use std::path::PathBuf;

fn main() {
    let kdu_root = Some("/home/gtierney/CLionProjects/untitled1/kakadu/v8_4_1-01787L")
        .map(PathBuf::from)
        .expect("KDU_ROOT environment variable not set");

    let kdu_coresys_sources = &[
        "coresys/kernels/kernels.cpp",
        "coresys/fast_coding/fbc_encoder.cpp",
        "coresys/fast_coding/fbc_encoder_tools.cpp",
        "coresys/fast_coding/fbc_decoder.cpp",
        "coresys/fast_coding/fbc_common.cpp",
        "coresys/fast_coding/fbc_decoder_tools.cpp",
        "coresys/roi/roi.cpp",
        "coresys/threads/kdu_threads.cpp",
        "coresys/parameters/params.cpp",
        "coresys/transform/neon_multi_transform_local.cpp",
        "coresys/transform/analysis2.cpp",
        "coresys/transform/transform.cpp",
        "coresys/transform/colour.cpp",
        "coresys/transform/neon_synthesis2.cpp",
        "coresys/transform/avx2_colour_local.cpp",
        "coresys/transform/synthesis2.cpp",
        "coresys/transform/neon_analysis2.cpp",
        "coresys/transform/transform2.cpp",
        "coresys/transform/multi_transform.cpp",
        "coresys/transform/neon_dwt_local.cpp",
        "coresys/transform/fusion.cpp",
        "coresys/transform/synthesis.cpp",
        "coresys/transform/neon_colour_local.cpp",
        "coresys/transform/avx2_analysis2.cpp",
        "coresys/transform/analysis.cpp",
        "coresys/transform/sse4_multi_transform_local.cpp",
        "coresys/transform/avx2_dwt_local.cpp",
        "coresys/transform/avx2_synthesis2.cpp",
        "coresys/transform/ssse3_colour_local.cpp",
        "coresys/transform/ssse3_dwt_local.cpp",
        "coresys/transform/avx_colour_local.cpp",
        "coresys/coding/block_coding_common.cpp",
        "coresys/coding/mq_decoder.cpp",
        "coresys/coding/avx_coder_local.cpp",
        "coresys/coding/encoder.cpp",
        "coresys/coding/decoder.cpp",
        "coresys/coding/cplex_analysis.cpp",
        "coresys/coding/avx2_coder_local.cpp",
        "coresys/coding/block_decoder.cpp",
        "coresys/coding/neon_coder_local.cpp",
        "coresys/coding/ssse3_coder_local.cpp",
        "coresys/coding/mq_encoder.cpp",
        "coresys/coding/block_encoder.cpp",
        "coresys/shared/core_local.cpp",
        "coresys/messaging/messaging.cpp",
        "coresys/compressed/compressed.cpp",
        "coresys/compressed/blocks.cpp",
        "coresys/compressed/codestream.cpp",
        "coresys/common/kdu_arch.cpp",
        "apps/jp2/jp2.cpp",
        "apps/compressed_io/file_io.cpp",
        "apps/support/kdu_stripe_decompressor.cpp",
        "apps/support/avx2_region_compositor.cpp",
        "apps/support/ssse3_stripe_transfer.cpp",
        "apps/support/avx2_region_decompressor.cpp",
        "apps/support/kdu_stream_decompressor.cpp",
        "apps/support/neon_stripe_transfer.cpp",
        "apps/support/supp_local.cpp",
        "apps/support/kdu_stripe_compressor.cpp",
        "apps/support/kdu_region_animator.cpp",
        "apps/support/neon_region_compositor.cpp",
        "apps/support/kdu_region_decompressor.cpp",
        "apps/support/ssse3_region_decompressor.cpp",
        "apps/support/sse4_region_decompressor.cpp",
        "apps/support/avx2_stripe_transfer.cpp",
        "apps/support/neon_region_decompressor.cpp",
        "apps/support/kdu_region_compositor.cpp",
    ];

    let kdu_includes = &[
        "coresys/common",
        "coresys/shared",
        "apps/support",
        "apps/jp2",
        "apps/compressed_io"
    ];

    let is_debug_build = match std::env::var("PROFILE").as_ref().map(String::as_str) {
        Ok("release") => false,
        Err(e) => {
            eprintln!("Failed to detect build profile, defaulting to debug. {e}");
            true
        }
        _ => true,
    };

    let mut build = cxx_build::bridge("src/lib.rs");

    build.files(kdu_coresys_sources.map(|path| kdu_root.join(path)));
    build.includes(kdu_includes.map(|path| kdu_root.join(path)));
    build.define("KDU_SIMD_OPTIMIZATIONS", "1");
    build.define("KDU_X86_INTRINSICS", "1");
    build.file("src/kakadu_rs.cc");

    // If not present, no overrides.
    let _ = build.try_flags_from_environment("KDU_CXXFLAGS");

    build.flag("-mavx2");
    build.flag("-mfma");

    if !is_debug_build {
        for flag in &["-flto", "-ffat-lto-objects", "-funified-lto"] {
            build.flag(flag);
        }
    }

    build.compile("kdurs");

    println!("cargo::rerun-if-changed=src/lib.rs");
    println!("cargo::rerun-if-changed=src/kakadu_rs.cc");
    println!("cargo::rerun-if-changed=src/kakadu_rs.h");
}
