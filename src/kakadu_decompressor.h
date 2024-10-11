#pragma once

#include <memory>

#include <kdu_compressed.h>
#include <kdu_region_decompressor.h>

#include "kakadu_context.h"

namespace digirati::kaduceus {
    class KakaduDecompressor final {
    public:
        /**
         * Creates a new Kakadu decompressor.
         *
         * @param codestream the codestream to decompress
         * @param roi the region of interest to decompress
         */
        explicit KakaduDecompressor(std::shared_ptr<KakaduContext> ctx, kdu_core::kdu_codestream codestream, kdu_core::kdu_dims roi, kdu_supp::kdu_channel_mapping channel_mapping);

    private:
        std::shared_ptr<KakaduContext> ctx;
        kdu_supp::kdu_channel_mapping channel_mapping;
        kdu_core::kdu_codestream codestream;
        kdu_supp::kdu_region_decompressor decompressor;
    };

}
