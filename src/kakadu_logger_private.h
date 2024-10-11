#pragma once

#include <kdu_messaging.h>
#include <sstream>
#include <sys/types.h>

#include "kaduceus/src/lib.rs.h"

namespace digirati::kaduceus {

class KakaduLogger : public kdu_core::kdu_thread_safe_message {
public:
    explicit KakaduLogger(LogLevel level)
        : level(level)
    {
    }

    void put_text(const char* string)
    {
        std::string message(string);
        std::string::size_type newline_offset;

        while ((newline_offset = message.find('\n')) != std::string::npos) {
            message.replace(newline_offset, 1, " ");
        }

        buffer << message;
    }

    void flush(bool end_of_message = false)
    {
        if (end_of_message) {
            log(level, rust::Str(buffer.str()));
        }

        buffer.clear();
    }

private:
    std::stringstream buffer;
    LogLevel level;
};

}
