#include "kaduceus.h"

namespace digirati::kaduceus {

std::unique_ptr<KakaduImageReader> create_kakadu_image_reader(std::shared_ptr<KakaduContext> ctx, rust::Box<AsyncReader> reader)
{
    return std::make_unique<KakaduImageReader>(ctx, reader);
}

std::shared_ptr<KakaduContext> create_kakadu_context()
{
    return std::make_shared<KakaduContext>();
}

}
