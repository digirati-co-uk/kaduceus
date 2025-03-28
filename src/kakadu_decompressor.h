#pragma once

#include <memory>

#include <kdu_compressed.h>
#include <kdu_region_decompressor.h>

#include "kakadu_context.h"
#include "rust/cxx.h"

namespace digirati::kaduceus {
struct Region;

class CxxKakaduDecompressor final {
public:
    /**
     * Creates a new Kakadu decompressor.
     *
     * @param codestream the codestream to decompress
     * @param roi the region of interest to decompress
     */
    explicit CxxKakaduDecompressor(std::shared_ptr<CxxKakaduContext> ctx, kdu_core::kdu_codestream codestream, kdu_core::kdu_dims roi, kdu_core::kdu_uint32 scaled_width, kdu_core::kdu_uint32 scaled_height);
    ~CxxKakaduDecompressor();

    bool process(rust::Slice<kdu_core::kdu_byte> output, Region& output_region);

    /// @brief
    /// @return
    bool finish(kdu_core::kdu_exception& error_code);

private:
    std::shared_ptr<CxxKakaduContext> ctx;
    kdu_supp::kdu_channel_mapping channel_mapping;
    kdu_core::kdu_codestream codestream;
    kdu_supp::kdu_region_decompressor decompressor;
    kdu_core::kdu_dims roi;
    kdu_core::kdu_dims incomplete_region;
    kdu_core::kdu_thread_env thread_env;
    kdu_core::kdu_thread_queue thread_queue;
};

}
