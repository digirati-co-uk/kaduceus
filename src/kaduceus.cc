#include "kaduceus.h"

namespace digirati::kaduceus {

std::unique_ptr<CxxKakaduImage> create_kakadu_image_reader(std::shared_ptr<CxxKakaduContext> ctx, rust::Box<AsyncReader> reader)
{
    return std::make_unique<CxxKakaduImage>(ctx, reader);
}

std::shared_ptr<CxxKakaduContext> create_kakadu_context()
{
    return std::make_shared<CxxKakaduContext>();
}

}
