#pragma once

#include "rust/cxx.h"

#include <iostream>
#include <jpx.h>
#include <kdu_compressed.h>

namespace digirati::kaduceus {
struct AsyncReader;

class AsyncReaderCompressedSource final : public kdu_core::kdu_membacked_compressed_source {
public:
    explicit AsyncReaderCompressedSource(rust::Box<AsyncReader>& reader);
    virtual ~AsyncReaderCompressedSource() { close(); }

protected:
    kdu_core::kdu_long fetch_data(kdu_core::kdu_long max_bytes, kdu_core::kdu_byte* buffer, bool blocking) override;
    bool seek(kdu_core::kdu_long offset) override;
    kdu_core::kdu_long get_pos() override;
private:
    rust::Box<AsyncReader> reader_;
};
}
