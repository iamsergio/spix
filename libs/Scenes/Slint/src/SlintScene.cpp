/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include "SlintScene.h"

#include "SlintItem.h"

#include <Spix/Data/ItemPath.h>

#include <algorithm>
#include <cmath>
#include <fstream>
#include <variant>

#define STB_IMAGE_WRITE_IMPLEMENTATION
#include <stb_image_write.h>

namespace spix {

namespace {

using slint::testing::ElementHandle;

struct Box {
    double x = 0, y = 0, w = 0, h = 0;
};

Box boxOf(const ElementHandle& e)
{
    auto p = e.absolute_position();
    auto s = e.size();
    return Box {p.x, p.y, s.width, s.height};
}

/// Elements are laid out inside their parents, so a descendant's rect is within its ancestor's.
/// Tolerate rounding.
bool inside(const Box& inner, const Box& outer)
{
    constexpr double eps = 0.5;
    return inner.x >= outer.x - eps && inner.y >= outer.y - eps && inner.x + inner.w <= outer.x + outer.w + eps
        && inner.y + inner.h <= outer.y + outer.h + eps;
}

bool matches(const ElementHandle& e, const path::Selector& selector, SlintHooks& hooks)
{
    struct Visitor {
        const ElementHandle& e;
        SlintHooks& hooks;

        bool operator()(const path::NameSelector& s) const
        {
            auto id = e.accessible_id();
            return id && std::string_view(*id) == s.name();
        }
        bool operator()(const path::TypeSelector& s) const
        {
            auto type = e.type_name();
            if (type && std::string_view(*type) == s.type())
                return true;
            if (auto bases = e.bases()) {
                for (const auto& base : *bases) {
                    if (std::string_view(base) == s.type())
                        return true;
                }
            }
            return false;
        }
        bool operator()(const path::ValueSelector& s) const
        {
            auto v = slintElementProperty(e, "text", hooks);
            return v && *v == s.value();
        }
        bool operator()(const path::PropertyValueSelector& s) const
        {
            auto v = slintElementProperty(e, s.propertyName(), hooks);
            return v && *v == s.propertyValue();
        }
        bool operator()(const path::PropertySelector& s) const
        {
            // Needs to read a property holding another item, which compiled Slint can't do.
            hooks.warn("Property selectors ('." + s.name() + "') are not supported for Slint");
            return false;
        }
    };
    return std::visit(Visitor {e, hooks}, selector);
}

/// Finds the first chain of elements, in tree order, matching components[index..], each inside the previous.
std::optional<ElementHandle> findChain(const std::vector<ElementHandle>& elements,
    const std::vector<path::Component>& components, size_t componentIndex, size_t firstElement, const Box& area,
    SlintHooks& hooks)
{
    for (size_t i = firstElement; i < elements.size(); ++i) {
        const auto& e = elements[i];
        auto box = boxOf(e);
        if (!inside(box, area))
            continue;
        if (!matches(e, components[componentIndex].selector(), hooks))
            continue;
        if (componentIndex + 1 == components.size())
            return e;
        if (auto rest = findChain(elements, components, componentIndex + 1, i + 1, box, hooks))
            return rest;
    }
    return std::nullopt;
}

std::string base64(const std::vector<unsigned char>& data)
{
    static const char table[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    std::string out;
    out.reserve((data.size() + 2) / 3 * 4);
    for (size_t i = 0; i < data.size(); i += 3) {
        unsigned v = data[i] << 16;
        if (i + 1 < data.size())
            v |= data[i + 1] << 8;
        if (i + 2 < data.size())
            v |= data[i + 2];
        out += table[(v >> 18) & 63];
        out += table[(v >> 12) & 63];
        out += i + 1 < data.size() ? table[(v >> 6) & 63] : '=';
        out += i + 2 < data.size() ? table[v & 63] : '=';
    }
    return out;
}

} // namespace

SlintScene::SlintScene(std::shared_ptr<SlintHooks> hooks)
: m_hooks(std::move(hooks))
{
}

void SlintScene::addWindow(std::string name, Window window)
{
    m_windows[std::move(name)] = std::move(window);
}

void SlintScene::removeWindow(const std::string& name)
{
    m_windows.erase(name);
}

std::unique_ptr<Item> SlintScene::itemAtPath(const ItemPath& path)
{
    if (path.length() == 0)
        return {};
    const auto* windowName = std::get_if<path::NameSelector>(&path.rootComponent().selector());
    if (!windowName)
        return {};
    auto it = m_windows.find(windowName->name());
    if (it == m_windows.end())
        return {};
    const auto& win = it->second;

    std::vector<ElementHandle> elements;
    win.visit([&elements](ElementHandle e) {
        elements.push_back(std::move(e));
        return false;
    });

    if (path.length() == 1) {
        std::optional<ElementHandle> anyElement;
        if (!elements.empty())
            anyElement = elements.front();
        return std::make_unique<SlintItem>(win.window, std::nullopt, std::move(anyElement), m_hooks);
    }

    auto scale = win.window->scale_factor();
    auto size = win.window->size();
    Box windowBox {0, 0, size.width / scale, size.height / scale};
    auto components = path.components();
    components.erase(components.begin());

    auto found = findChain(elements, components, 0, 0, windowBox, *m_hooks);
    if (!found)
        return {};
    return std::make_unique<SlintItem>(win.window, std::move(found), std::nullopt, m_hooks);
}

std::vector<unsigned char> SlintScene::screenshotPng(const ItemPath& targetItem)
{
    auto item = itemAtPath(targetItem);
    auto* slintItem = dynamic_cast<SlintItem*>(item.get());
    if (!slintItem)
        return {};

    auto snapshot = slintItem->window()->take_snapshot();
    if (!snapshot)
        return {};

    // The snapshot is in physical pixels, the item geometry in logical ones.
    const double scale = slintItem->window()->scale_factor();
    const int imgW = snapshot->width();
    const int imgH = snapshot->height();
    auto pos = slintItem->windowPosition();
    auto size = slintItem->size();
    int x = std::clamp(static_cast<int>(std::lround(pos.x * scale)), 0, imgW);
    int y = std::clamp(static_cast<int>(std::lround(pos.y * scale)), 0, imgH);
    int x2 = std::clamp(static_cast<int>(std::lround((pos.x + size.width) * scale)), 0, imgW);
    int y2 = std::clamp(static_cast<int>(std::lround((pos.y + size.height) * scale)), 0, imgH);
    int w = x2 - x;
    int h = y2 - y;
    if (w <= 0 || h <= 0)
        return {};

    const auto* pixels = reinterpret_cast<const unsigned char*>(snapshot->begin());
    std::vector<unsigned char> png;
    auto write = [](void* ctx, void* data, int len) {
        auto* out = static_cast<std::vector<unsigned char>*>(ctx);
        auto* bytes = static_cast<unsigned char*>(data);
        out->insert(out->end(), bytes, bytes + len);
    };
    if (!stbi_write_png_to_func(write, &png, w, h, 4, pixels + (static_cast<size_t>(y) * imgW + x) * 4, imgW * 4))
        return {};
    return png;
}

void SlintScene::takeScreenshot(const ItemPath& targetItem, const std::string& filePath)
{
    auto png = screenshotPng(targetItem);
    if (png.empty())
        return;
    std::ofstream file(filePath, std::ios::binary);
    file.write(reinterpret_cast<const char*>(png.data()), static_cast<std::streamsize>(png.size()));
}

std::string SlintScene::takeScreenshotAsBase64(const ItemPath& targetItem)
{
    return base64(screenshotPng(targetItem));
}

} // namespace spix
