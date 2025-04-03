#include "kakadu_decompressor.h"
#include "rust/cxx.h"

#include "kaduceus/src/lib.rs.h"

namespace digirati::kaduceus {
using namespace kdu_core;

// Helper GCD function
int gcd(int a, int b)
{
    while (b != 0) {
        int temp = b;
        b = a % b;
        a = temp;
    }
    return a;
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

    double scale_x = (double)scaled_width / roi.size.x;
    double scale_y = (double)scaled_height / roi.size.y;

    int max_levels = codestream.get_min_dwt_levels();
    int discard_levels = 0;

    while (discard_levels < max_levels && scale_x <= 0.5 && scale_y <= 0.5) {
        discard_levels++;
        scale_x *= 2.0;
        scale_y *= 2.0;
    }

    // Calculate the rational expansion factors
    // This is a simplified approach - you may need a better fraction approximation
    int gcd_x = gcd((int)(scale_x * 1000), 1000);
    int gcd_y = gcd((int)(scale_y * 1000), 1000);

    kdu_coords expand_numerator;
    kdu_coords expand_denominator;
    expand_numerator.x = (int)(scale_x * 1000) / gcd_x;
    expand_denominator.x = 1000 / gcd_x;

    expand_numerator.y = (int)(scale_y * 1000) / gcd_y;
    expand_denominator.y = 1000 / gcd_y;

    // Verify the results are safe
    double min_prod, max_x, max_y;
    decompressor.get_safe_expansion_factors(
        codestream, NULL, 0, discard_levels,
        min_prod, max_x, max_y);

    // Adjust if necessary
    double prod = ((double)expand_numerator.x * expand_numerator.y) / ((double)expand_denominator.x * expand_denominator.y);
    if (prod < min_prod) {
        // Scale up to meet minimum
        double scale = std::sqrt(min_prod / prod);
        expand_numerator.x = (int)(expand_numerator.x * scale);
        expand_numerator.y = (int)(expand_numerator.y * scale);
    }

    if (expand_numerator.x > max_x * expand_denominator.x) {
        expand_numerator.x = (int)(max_x * expand_denominator.x);
    }

    if (expand_numerator.y > max_y * expand_denominator.y) {
        expand_numerator.y = (int)(max_y * expand_denominator.y);
    }

    kdu_coords ref_subs;
    codestream.get_subsampling(0, ref_subs, true);

    kdu_core::kdu_dims dims = decompressor.find_render_dims(roi, ref_subs, expand_numerator, expand_denominator);

    decompressor.mem_configure(&ctx->membroker);
    decompressor.start(codestream,
        &this->channel_mapping,
        -1,
        discard_levels,
        8192 * 4,
        dims,
        expand_numerator,
        expand_denominator,
        /* precise = */ false,
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
