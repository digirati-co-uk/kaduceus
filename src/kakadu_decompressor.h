#pragma once

#include <memory>

#include <kdu_compressed.h>
#include <kdu_region_decompressor.h>

#include "kakadu_context.h"
#include "rust/cxx.h"

namespace digirati::kaduceus {
struct Region;

class KakaduDecompressor final {
public:
    /**
     * Creates a new Kakadu decompressor.
     *
     * @param codestream the codestream to decompress
     * @param roi the region of interest to decompress
     */
    explicit KakaduDecompressor(std::shared_ptr<KakaduContext> ctx, kdu_core::kdu_codestream codestream, kdu_core::kdu_dims roi);

    bool process(rust::Slice<kdu_core::kdu_int32> output, Region& output_region);

    /// @brief
    /// @return
    bool finish(kdu_core::kdu_exception& error_code);

private:
    std::shared_ptr<KakaduContext> ctx;
    kdu_supp::kdu_channel_mapping channel_mapping;
    kdu_core::kdu_codestream codestream;
    kdu_supp::kdu_region_decompressor decompressor;
    kdu_core::kdu_dims roi;
    kdu_core::kdu_dims incomplete_region;
};

}
