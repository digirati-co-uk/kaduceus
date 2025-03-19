#include "kakadu_image.h"
#include "kaduceus/src/lib.rs.h"

#include <format>

namespace digirati::kaduceus {

std::unique_ptr<CxxKakaduDecompressor> CxxKakaduImage::open(const struct Region& region)
{
    kdu_core::kdu_dims dims;
    dims.from_u32(region.x, region.y, region.width, region.height);

    return std::make_unique<CxxKakaduDecompressor>(ctx, codestream, dims);
}

CxxKakaduImage::CxxKakaduImage(std::shared_ptr<CxxKakaduContext> ctx, rust::Box<AsyncReader>& reader)
    : ctx(std::move(ctx))
    , compressed_source(reader)
    , codestream()
{
    family_source.open(&compressed_source, &ctx->membroker);

    if (!jpx_source.open(&family_source, true)) {
        throw std::runtime_error(std::format("JP2 source is not a valid image object"));
    }

    auto codestream_source = jpx_source.access_codestream(0);
    auto codestream_source_stream = codestream_source.open_stream();

    codestream.create(codestream_source_stream, nullptr, &this->ctx->membroker);
    codestream.set_fast();
    codestream.set_resilient();
}

CxxKakaduImage::~CxxKakaduImage()
{
    codestream.destroy();
    jpx_source.close();
    family_source.close();
}

Info CxxKakaduImage::info()
{
    auto layer = jpx_source.access_layer(0);
    auto cs = jpx_source.access_codestream(0);

    kdu_supp::kdu_channel_mapping channel_mapping;
    channel_mapping.configure(codestream);

    const int ref_component = channel_mapping.get_source_component(0);
    kdu_core::kdu_dims image_size;

    codestream.get_dims(ref_component, image_size);
    int dwt_levels = codestream.get_min_dwt_levels();

    kdu_core::kdu_dims tile_size;
    codestream.get_tile_dims(kdu_core::kdu_coords(0, 0), -1, tile_size);

    return Info {
        .width = image_size.access_size()->get_x(),
        .height = image_size.access_size()->get_y(),
        .tile_width = tile_size.access_size()->get_x(),
        .tile_height = tile_size.access_size()->get_y(),
        .dwt_levels = dwt_levels,
    };
}

}
