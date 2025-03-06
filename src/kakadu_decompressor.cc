#include "kakadu_decompressor.h"
#include "rust/cxx.h"

#include "kaduceus/src/lib.rs.h"

namespace digirati::kaduceus {

KakaduDecompressor::KakaduDecompressor(std::shared_ptr<KakaduContext> ctx, kdu_core::kdu_codestream codestream, kdu_core::kdu_dims roi)
    : codestream(codestream)
    , roi(roi)
{
    channel_mapping.configure(codestream);

    decompressor.mem_configure(&ctx->membroker);
    decompressor.start(codestream,
        &this->channel_mapping,
        -1,
        0,
        32,
        roi,
        kdu_core::kdu_coords(1, 1),
        kdu_core::kdu_coords(1, 1),
        /* precise = */ false,
        kdu_core::KDU_WANT_OUTPUT_COMPONENTS,
        /* fastest = */ true,
        &ctx->threading_env,
        &ctx->threading_queue);

        incomplete_region.assign(roi);
}

bool KakaduDecompressor::process(rust::Slice<kdu_core::kdu_int32> output, Region &output_region)
{
    kdu_core::kdu_dims new_region;

    auto incomplete = decompressor.process(output.data(), *roi.access_pos(), 0, 0, output.length(), incomplete_region, new_region);
    auto new_pos = new_region.access_pos();
    auto new_size = new_region.access_size();

    new_pos->subtract(*roi.access_pos());
    output_region.x = new_pos->get_x();
    output_region.y = new_pos->get_y();
    output_region.width = new_size->get_x();
    output_region.height = new_size->get_y();

    return incomplete;
}

bool KakaduDecompressor::finish(kdu_core::kdu_exception &error_code) {
    return decompressor.finish(&error_code);
}

}
