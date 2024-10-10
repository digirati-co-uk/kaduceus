#ifndef KAKADU_RS_H
#define KAKADU_RS_H

#include "rust/cxx.h"
#include "kakadu_context.h"
#include "kakadu_decompressor.h"
#include "kakadu_image_reader.h"

#include <jpx.h>
#include <kdu_compressed.h>
#include <kdu_region_decompressor.h>
#include <kdu_stripe_decompressor.h>
#include <memory>

namespace digirati::kdurs {
struct AsyncReader;

class AsyncReaderCompressedSource final : public kdu_core::kdu_membacked_compressed_source {
public:
    explicit AsyncReaderCompressedSource(rust::Box<AsyncReader>& reader);

protected:
    kdu_core::kdu_long fetch_data(kdu_core::kdu_long max_bytes, kdu_core::kdu_byte* buffer, bool blocking) override;

private:
    rust::Box<AsyncReader> reader_;
};


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

class KakaduImageReader final {
public:
    explicit KakaduImageReader(std::shared_ptr<KakaduContext> ctx, rust::Box<AsyncReader>& reader);

    struct Info info();
    std::unique_ptr<KakaduDecompressor> decompress(const struct Region& region);

private:
    AsyncReaderCompressedSource compressed_source;
    std::shared_ptr<KakaduContext> ctx;
    kdu_supp::jp2_family_src family_source;
    kdu_supp::jp2_source source;
};

std::unique_ptr<KakaduImageReader> create_kakadu_image_reader(std::shared_ptr<KakaduContext> ctx, rust::Box<AsyncReader> reader);
std::shared_ptr<KakaduContext> create_kakadu_context();
}

#endif // KAKADU_RS_H
