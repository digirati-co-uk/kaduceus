#pragma once

#include <memory>

#include <kdu_compressed.h>

#include "data_source.h"
#include "kakadu_decompressor.h"

namespace digirati::kaduceus {
class CxxKakaduImage final {
public:
    explicit CxxKakaduImage(std::shared_ptr<CxxKakaduContext> ctx, rust::Box<AsyncReader>& reader);
    ~CxxKakaduImage();

    struct Info info();
    std::unique_ptr<CxxKakaduDecompressor> open(const struct Region& region, kdu_core::kdu_uint32 scaled_width, kdu_core::kdu_uint32 scaled_height);

private:
    AsyncReaderCompressedSource compressed_source;
    std::shared_ptr<CxxKakaduContext> ctx;
    kdu_supp::jp2_family_src family_source;
    kdu_supp::jpx_source jpx_source;
    kdu_core::kdu_codestream codestream;

    ::std::uint32_t width;
    ::std::uint32_t height;
    ::std::uint32_t tile_width;
    ::std::uint32_t tile_height;
    ::std::uint32_t dwt_levels;
};

std::unique_ptr<CxxKakaduImage> create_kakadu_image_reader(std::shared_ptr<CxxKakaduContext> ctx, rust::Box<AsyncReader> reader);
std::shared_ptr<CxxKakaduContext> create_kakadu_context();
}
