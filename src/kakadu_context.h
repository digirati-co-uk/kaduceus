#pragma once

#include <kdu_compressed.h>
#include <kdu_cache.h>

namespace digirati::kaduceus {

class CxxKakaduContext final {
    friend class CxxKakaduImage;
    friend class CxxKakaduDecompressor;

public:
    explicit CxxKakaduContext(ssize_t memory_limit = -1, size_t threads = std::thread::hardware_concurrency());

private:
    kdu_core::kdu_membroker membroker;
    kdu_core::kdu_thread_queue threading_queue;
};

static std::string last_error_message();

}
