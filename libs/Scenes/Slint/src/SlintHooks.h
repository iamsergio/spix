#pragma once

#include <Spix/SlintBot.h>

#include <map>
#include <string>
#include <vector>

namespace spix {

/// Registered test-helper handlers, shared between the bot and the items it hands out.
struct SlintHooks {
    std::map<std::string, SlintPropertyGetter> getters;
    std::map<std::string, SlintPropertySetter> setters;
    std::map<std::string, SlintMethodHandler> methods;
    std::vector<std::string> warnings;

    void warn(std::string message);
};

} // namespace spix
