#include "data_source.h"

#include "kaduceus/src/lib.rs.h"

namespace digirati::kaduceus {

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

bool AsyncReaderCompressedSource::seek(kdu_core::kdu_long offset)
{
    return kdu_membacked_compressed_source::seek(offset);
}

kdu_core::kdu_long AsyncReaderCompressedSource::get_pos() {
    return kdu_membacked_compressed_source::get_pos();
}
}
