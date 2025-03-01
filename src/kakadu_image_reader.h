#pragma once

#include <memory>

#include <kdu_compressed.h>

#include "data_source.h"
#include "kakadu_decompressor.h"

namespace digirati::kaduceus {
class KakaduImageReader final {
public:
    explicit KakaduImageReader(std::shared_ptr<KakaduContext> ctx, rust::Box<AsyncReader>& reader);

    struct Info info();
    std::unique_ptr<KakaduDecompressor> open(const struct Region& region);

private:
    AsyncReaderCompressedSource compressed_source;
    std::shared_ptr<KakaduContext> ctx;
    kdu_supp::jp2_family_src family_source;
    kdu_supp::jp2_source source;
    kdu_core::kdu_codestream codestream;
};

std::unique_ptr<KakaduImageReader> create_kakadu_image_reader(std::shared_ptr<KakaduContext> ctx, rust::Box<AsyncReader> reader);
std::shared_ptr<KakaduContext> create_kakadu_context();
}
