#include "kakadu_context.h"

#include "kakadu_logger_private.h"
#include <mutex>

namespace digirati::kaduceus {

std::once_flag logging_initialized;

static KakaduLogger warnings(LogLevel::Warning);
static KakaduLogger errors(LogLevel::Error);

CxxKakaduContext::CxxKakaduContext(ssize_t memory_limit, size_t threads)
    : membroker(memory_limit)
{
    std::call_once(logging_initialized, []() {
        kdu_core::kdu_customize_warnings(&warnings);
        kdu_core::kdu_customize_errors(&errors);
    });
}

static std::string last_error_message()
{
    return errors.buffered();
}

}
