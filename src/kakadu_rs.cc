#include "kakadu_rs.h"
#include "kdurs/src/lib.rs.h"

digirati::kdurs::AsyncReaderCompressedSource::AsyncReaderCompressedSource(rust::Box<AsyncReader> &reader) : reader_(
    std::move(reader)) {
}

kdu_core::kdu_long digirati::kdurs::AsyncReaderCompressedSource::fetch_data(kdu_core::kdu_long max_bytes,
                                                                            kdu_core::kdu_byte *buffer, bool blocking) {
    if (!blocking) {
        return -1;
    }

    try {
        const auto read = reader_->read(rust::Slice(buffer, max_bytes));
        if (read > SSIZE_MAX) {
            throw std::runtime_error("reader returned too many bytes, this is an implementation error");
        }

        return static_cast<ssize_t>(read);
    } catch (rust::Error &e) {
        finish_fetching(-1);
        return 0;
    }
}
