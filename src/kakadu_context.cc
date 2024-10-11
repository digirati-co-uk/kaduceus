#include "kakadu_context.h"

#include "kakadu_logger_private.h"
#include <mutex>

namespace digirati::kaduceus {

std::once_flag logging_initialized;

static KakaduLogger warnings(LogLevel::Warning);
static KakaduLogger errors(LogLevel::Error);

KakaduContext::KakaduContext(ssize_t memory_limit, size_t threads)
    : membroker(memory_limit)
{
    std::call_once(logging_initialized, []() {
        kdu_core::kdu_customize_warnings(&warnings);
        kdu_core::kdu_customize_errors(&errors);
    });

    if (threads > 0) {
        threading_env.create();

        for (auto i = 0; i < threads; i++) {
            threading_env.add_thread();
        }

        threading_env.attach_queue(&threading_queue, nullptr, "root_work_queue");
    }
}

}
