#pragma once

#include <kdu_compressed.h>

namespace digirati::kaduceus {

class KakaduContext final {
    friend class KakaduImageReader;
    friend class KakaduDecompressor;

public:
    explicit KakaduContext(ssize_t memory_limit = -1, size_t threads = std::thread::hardware_concurrency());

private:
    kdu_core::kdu_membroker membroker;
    kdu_core::kdu_thread_queue threading_queue;
    kdu_core::kdu_thread_env threading_env;
};

}
