#include "data_source.h"

#include "kaduceus/src/lib.rs.h"

namespace digirati::kaduceus {

AsyncReaderCompressedSource::AsyncReaderCompressedSource(rust::Box<AsyncReader>& reader)
    : reader_(std::move(reader))
{

    std::thread([&]() {
        enable(KDU_MEMBACKED_SOURCE_FULLY_SERVICED, NULL, 0, 0, -1.0, 0, 0);
        size_t total_fetched_bytes = 0;
        kdu_core::kdu_int32 term_code = 0;
        service(total_fetched_bytes, term_code);
    }).detach();

    open(false);
}

kdu_core::kdu_long AsyncReaderCompressedSource::fetch_data(kdu_core::kdu_long max_bytes,
    kdu_core::kdu_byte* buffer, bool blocking)
{
    if (!blocking) {
        return -1;
    }

    const ssize_t read = reader_->read(rust::Slice(buffer, max_bytes));
    if (read > SSIZE_MAX) {
        throw std::runtime_error("reader returned too many bytes, this is an implementation error");
    }

    return static_cast<ssize_t>(read);
}

// bool AsyncReaderCompressedSource::seek(kdu_core::kdu_long offset)
// {
//     reader_->seek(offset);
//     pos = offset;
//     return true;
// }

// int AsyncReaderCompressedSource::read(kdu_core::kdu_byte* buf, int num_bytes)
// {
//     auto read = reader_->read(rust::Slice(buf, num_bytes));
//     if (read > 0) {
//         pos += (unsigned long)read;
//     }

//     return read;
// }

// kdu_core::kdu_long AsyncReaderCompressedSource::get_pos() {
//     return pos;
// }
}
