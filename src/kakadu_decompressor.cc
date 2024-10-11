#include "kakadu_decompressor.h"

namespace digirati::kaduceus {

KakaduDecompressor::KakaduDecompressor(std::shared_ptr<KakaduContext> ctx, kdu_core::kdu_codestream codestream, kdu_core::kdu_dims roi, kdu_supp::kdu_channel_mapping channel_mapping)
    : codestream(codestream)
    , channel_mapping(channel_mapping)
{
    decompressor.mem_configure(&ctx->membroker);
    decompressor.start(codestream,
        &this->channel_mapping,
        -1,
        32,
        1,
        roi,
        kdu_core::kdu_coords(1, 1),
        kdu_core::kdu_coords(1, 1),
        false,
        kdu_core::KDU_WANT_OUTPUT_COMPONENTS,
        true,
        &ctx->threading_env,
        &ctx->threading_queue);
}

}
