#include "kakadu_decompressor.h"
#include "rust/cxx.h"

#include "kaduceus/src/lib.rs.h"

#include <numeric>

namespace digirati::kaduceus {
using namespace kdu_core;

static void compute_expands(
    const kdu_dims& roi,
    unsigned int scaled_width,
    unsigned int scaled_height,
    kdu_coords& expand_numerator,
    kdu_coords& expand_denominator,
    int& discard_level,
    int max_discard_levels)
{
    expand_numerator.x = scaled_width;
    expand_numerator.y = scaled_height;

    const unsigned int roi_x = (unsigned int)roi.size.x;
    const unsigned int roi_y = (unsigned int)roi.size.y;

    if (scaled_width >= roi_x || scaled_height >= roi_y) {
        expand_denominator.x = roi.size.x;
        expand_denominator.y = roi.size.y;
    } else {
        int discard_x = std::countl_zero(scaled_width) - std::countl_zero(roi_x);
        int discard_y = std::countl_zero(scaled_height) - std::countl_zero(roi_y);

        discard_level = std::min(std::min(discard_x, discard_y), max_discard_levels); // TODO use discard_x/y independently?

        unsigned int discarded_roi_x = 1 + ((roi_x - 1) >> discard_level);
        unsigned int discarded_roi_y = 1 + ((roi_y - 1) >> discard_level);

        if (discarded_roi_x < scaled_width || discarded_roi_y < scaled_height) {
            discard_level--;
            discarded_roi_x = 1 + ((roi_x - 1) >> discard_level);
            discarded_roi_y = 1 + ((roi_y - 1) >> discard_level);
        }

        expand_denominator.x = discarded_roi_x;
        expand_denominator.y = discarded_roi_y;
    }
}

CxxKakaduDecompressor::CxxKakaduDecompressor(std::shared_ptr<CxxKakaduContext> ctx, kdu_core::kdu_codestream codestream, kdu_core::kdu_dims roi, kdu_core::kdu_uint32 scaled_width, kdu_core::kdu_uint32 scaled_height)
    : codestream(codestream)
    , roi(roi)
{
    channel_mapping.configure(codestream);

    thread_env.create();
    for (int i = 0; i < std::thread::hardware_concurrency(); i++) {
        thread_env.add_thread();
    }

    int max_discard_levels = codestream.get_min_dwt_levels();
    int discard_levels = 0;

    kdu_coords expand_numerator;
    kdu_coords expand_denominator;
    compute_expands(roi, scaled_width, scaled_height, expand_numerator, expand_denominator, discard_levels, max_discard_levels);

    kdu_coords subsampling { 1 << discard_levels, 1 << discard_levels };
    kdu_dims dims = decompressor.find_render_dims(roi, subsampling, expand_numerator, expand_denominator);

    decompressor.mem_configure(&ctx->membroker);
    decompressor.start(codestream,
        &this->channel_mapping,
        -1,
        discard_levels,
        8192 * 4,
        dims,
        expand_numerator,
        expand_denominator,
        /* precise = */ true,
        kdu_core::KDU_WANT_OUTPUT_COMPONENTS,
        /* fastest = */ false,
        nullptr,
        nullptr);

    incomplete_region.assign(roi);
}

CxxKakaduDecompressor::~CxxKakaduDecompressor()
{
    thread_env.cs_terminate(codestream);
    thread_env.destroy();
}

bool CxxKakaduDecompressor::process(rust::Slice<kdu_core::kdu_byte> output, Region& output_region)
{
    kdu_core::kdu_dims new_region;
    int chan_offsets[] = {
        0,
        1,
        2,
    };
    auto incomplete = decompressor.process(output.data(), &chan_offsets[0], 3, *roi.access_pos(), 0, 0, output.length() / 3, incomplete_region, new_region, 8, true, 1, 0, 3);
    auto new_pos = new_region.access_pos();
    auto new_size = new_region.access_size();

    new_pos->subtract(*roi.access_pos());
    output_region.x = new_pos->get_x();
    output_region.y = new_pos->get_y();
    output_region.width = new_size->get_x();
    output_region.height = new_size->get_y();

    return incomplete;
}

bool CxxKakaduDecompressor::finish(kdu_core::kdu_exception& error_code)
{
    bool success = decompressor.finish(&error_code);
    decompressor.reset();
    return success;
}

}
