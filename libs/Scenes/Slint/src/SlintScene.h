/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

#include "SlintEvents.h"
#include "SlintHooks.h" // also brings in slint-testing.h, which has no include guard

#include <Spix/Scene/Scene.h>

#include <functional>
#include <map>
#include <memory>
#include <string>

namespace spix {

class SlintScene : public Scene {
public:
    using ElementVisitor = std::function<bool(slint::testing::ElementHandle)>;
    using ElementVisit = std::function<void(const ElementVisitor&)>;

    struct Window {
        slint::Window* window = nullptr;
        ElementVisit visit;
    };

    explicit SlintScene(std::shared_ptr<SlintHooks> hooks);

    void addWindow(std::string name, Window window);
    void removeWindow(const std::string& name);

    std::unique_ptr<Item> itemAtPath(const ItemPath& path) override;
    Events& events() override { return m_events; }
    void takeScreenshot(const ItemPath& targetItem, const std::string& filePath) override;
    std::string takeScreenshotAsBase64(const ItemPath& targetItem) override;

private:
    std::vector<unsigned char> screenshotPng(const ItemPath& targetItem);

    std::shared_ptr<SlintHooks> m_hooks;
    std::map<std::string, Window> m_windows;
    SlintEvents m_events;
};

} // namespace spix
