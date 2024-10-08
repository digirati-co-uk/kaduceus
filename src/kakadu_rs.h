#ifndef KAKADU_RS_H
#define KAKADU_RS_H


#include "rust/cxx.h"

#include <kdu_compressed.h>
#include <kdu_region_decompressor.h>
#include <kdu_stripe_decompressor.h>

namespace digirati::kdurs {
    class AsyncReader;

    class AsyncReaderCompressedSource final : public kdu_core::kdu_membacked_compressed_source {
    public:
        explicit AsyncReaderCompressedSource(rust::Box<AsyncReader> &reader);
    protected:
        kdu_core::kdu_long fetch_data(kdu_core::kdu_long max_bytes, kdu_core::kdu_byte *buffer, bool blocking) override;

    private:
        rust::Box<AsyncReader> reader_;
    };

    class Jp2Decompressor final {
    private:
        kdu_supp::kdu_stripe_decompressor region_decompressor_;
    };
}

#endif //KAKADU_RS_H
