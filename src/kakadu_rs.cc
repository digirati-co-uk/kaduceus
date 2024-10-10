#include "kakadu_rs.h"
#include "kdu_compressed.h"
#include "kdu_region_decompressor.h"
#include "kdurs/src/lib.rs.h"

#include <format>
#include <stdexcept>


namespace digirati::kdurs {


AsyncReaderCompressedSource::AsyncReaderCompressedSource(rust::Box<AsyncReader>& reader)
    : reader_(std::move(reader))
{
    open(true);
}

kdu_core::kdu_long AsyncReaderCompressedSource::fetch_data(kdu_core::kdu_long max_bytes,
    kdu_core::kdu_byte* buffer, bool blocking)
{
    if (!blocking) {
        return -1;
    }

    const size_t read = reader_->read(rust::Slice(buffer, max_bytes));
    if (read > SSIZE_MAX) {
        throw std::runtime_error("reader returned too many bytes, this is an implementation error");
    }

    return static_cast<ssize_t>(read);
}



std::unique_ptr<KakaduDecompressor> KakaduImageReader::decompress(const struct Region& region)
{
    kdu_core::kdu_dims dims;
    dims.from_u32(region.x, region.y, region.width, region.height);

    kdu_core::kdu_codestream codestream;
    codestream.create(&source, &ctx->threading_env, &ctx->membroker);
    codestream.set_fast();

    kdu_supp::kdu_channel_mapping channel_mapping;
    channel_mapping.configure(&source, true);

    return std::make_unique<KakaduDecompressor>(ctx, codestream, dims, channel_mapping);
}

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

KakaduImageReader::KakaduImageReader(std::shared_ptr<KakaduContext> ctx, rust::Box<AsyncReader>& reader)
    : ctx(std::move(ctx))
    , compressed_source(reader)
{
    family_source.open(&compressed_source, &ctx->membroker);

    if (!source.open(&family_source)) {
        throw std::runtime_error(std::format("JP2 source is not a valid image object"));
    }
}

Info KakaduImageReader::info()
{
    if (!source.read_header()) {
        throw std::runtime_error("Unable to handle JP2 contents");
    }

    kdu_core::kdu_codestream codestream;
    codestream.create(&source, &ctx->threading_env, &ctx->membroker);
    codestream.set_resilient();

    kdu_supp::kdu_channel_mapping channel_mapping;
    channel_mapping.configure(&source, true);

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

std::unique_ptr<KakaduImageReader> create_kakadu_image_reader(std::shared_ptr<KakaduContext> ctx, rust::Box<AsyncReader> reader)
{
    return std::make_unique<KakaduImageReader>(ctx, reader);
}

std::shared_ptr<KakaduContext> create_kakadu_context()
{
    return std::make_shared<KakaduContext>();
}

}
