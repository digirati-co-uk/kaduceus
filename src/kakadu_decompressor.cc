#include "kakadu_decompressor.h"
#include "rust/cxx.h"

#include "kaduceus/src/lib.rs.h"

namespace digirati::kaduceus {

CxxKakaduDecompressor::CxxKakaduDecompressor(std::shared_ptr<CxxKakaduContext> ctx, kdu_core::kdu_codestream codestream, kdu_core::kdu_dims roi, kdu_core::kdu_uint32 scaled_width, kdu_core::kdu_uint32 scaled_height)
    : codestream(codestream)
    , roi(roi)
{
    channel_mapping.configure(codestream);

    thread_env.create();

    for (int i = 0; i < std::thread::hardware_concurrency(); i++) {
        thread_env.add_thread();
    }

    auto numerator = kdu_core::kdu_coords(scaled_width, scaled_height);
    auto denominator = kdu_core::kdu_coords(roi.access_size()->x, roi.access_size()->y);

    kdu_core::kdu_dims dims = decompressor.find_render_dims(roi, kdu_core::kdu_coords(1, 1), numerator, denominator);

    decompressor.mem_configure(&ctx->membroker);
    decompressor.start(codestream,
        &this->channel_mapping,
        -1,
        0,
        1024,
        dims,
        numerator,
        denominator,
        /* precise = */ true,
        kdu_core::KDU_WANT_OUTPUT_COMPONENTS,
        /* fastest = */ true,
        &thread_env,
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
