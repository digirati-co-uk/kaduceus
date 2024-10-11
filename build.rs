use glob::glob;
use rustflags::Flag;
use std::path::PathBuf;

pub enum Arch {
    X64,
    Arm,
}

fn main() {
    let kdu_root = std::env::var("KDU_ROOT")
        .map(PathBuf::from)
        .unwrap_or(PathBuf::from("kakadu"));

    let target = std::env::var("TARGET").expect("target env var not set");
    let arch = match target.as_str() {
        "x86_64-unknown-linux-gnu" => Arch::X64,
        "aarch64-unknown-linux-gnu" => Arch::Arm,
        _ => panic!(),
    };

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
        "apps/jp2/jpx.cpp",
        "apps/compressed_io/file_io.cpp",
        "apps/client_server/kdu_client_window.cpp",
        "apps/support/kdu_stripe_decompressor.cpp",
        "apps/support/ssse3_stripe_transfer.cpp",
        "apps/support/avx2_region_decompressor.cpp",
        "apps/support/kdu_stream_decompressor.cpp",
        "apps/support/neon_stripe_transfer.cpp",
        "apps/support/supp_local.cpp",
        "apps/support/kdu_stripe_compressor.cpp",
        "apps/support/kdu_region_decompressor.cpp",
        "apps/support/ssse3_region_decompressor.cpp",
        "apps/support/sse4_region_decompressor.cpp",
        "apps/support/avx2_stripe_transfer.cpp",
        "apps/support/neon_region_decompressor.cpp",
    ];

    let kdu_includes = &[
        "coresys/common",
        "coresys/shared",
        "apps/support",
        "apps/jp2",
        "apps/compressed_io",
    ];

    let mut build = cxx_build::bridge("src/lib.rs");

    build.std("c++20");
    build.files(kdu_coresys_sources.map(|path| kdu_root.join(path)));
    build.includes(kdu_includes.map(|path| kdu_root.join(path)));
    build.define("KDU_SIMD_OPTIMIZATIONS", None);

    // If not present, no overrides.
    let _ = build.try_flags_from_environment("KDU_CXXFLAGS");

    // Enable fused multiply-add, and enable fast-math to make sure code benefits from it.
    build.flag("-mfma");
    build.flag("-ffast-math");

    for flag in rustflags::from_env() {
        match flag {
            Flag::Codegen {
                opt,
                value: Some(cpu),
            } if opt == "target-cpu" => {
                if cpu == "native" {
                    build.flag("-march=native");
                } else {
                    build.flag(format!("-mcpu={}", cpu));
                }
            }
            Flag::Codegen { opt, .. } if opt == "linker-plugin-lto" => {
                println!("cargo:warning=LTO is enabled");
                build.flag("-flto");
            }
            _ => continue,
        }
    }

    match arch {
        Arch::X64 => {
            build.flag("-mavx2");
            build.define("KDU_X86_INTRINSICS", None);
        }
        Arch::Arm => {
            build.flag("-mfloat-abi=soft");
            build.define("KDU_NEON_INTRINSICS", None);
        }
    }

    build.flag("-Wno-everything");

    let source_files = glob("src/*.cc").expect("failed to get sources");
    let headers = glob("src/*.h").expect("failed to get headers");
    let all = source_files.chain(headers).filter_map(|file| file.ok());

    for file in all {
        if file.extension().is_some_and(|ext| ext == "cc") {
            build.file(&file);
        }

        let source_path = file.to_string_lossy();
        println!("cargo::rerun-if-changed={source_path}");
    }

    println!("cargo::rerun-if-changed=src/lib.rs");

    build.compile("kaduceus");
}
