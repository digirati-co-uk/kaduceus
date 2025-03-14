#pragma once

#include <memory>

#include <kdu_compressed.h>

#include "data_source.h"
#include "kakadu_decompressor.h"

namespace digirati::kaduceus {
class CxxKakaduImage final {
public:
    explicit CxxKakaduImage(std::shared_ptr<CxxKakaduContext> ctx, rust::Box<AsyncReader>& reader);

    struct Info info();
    std::unique_ptr<CxxKakaduDecompressor> open(const struct Region& region);

private:
    AsyncReaderCompressedSource compressed_source;
    std::shared_ptr<CxxKakaduContext> ctx;
    kdu_supp::jp2_family_src family_source;
    kdu_supp::jpx_source jpx_source;
    kdu_core::kdu_codestream codestream;
};

std::unique_ptr<CxxKakaduImage> create_kakadu_image_reader(std::shared_ptr<CxxKakaduContext> ctx, rust::Box<AsyncReader> reader);
std::shared_ptr<CxxKakaduContext> create_kakadu_context();
}
