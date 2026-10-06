/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

#include "SlintHooks.h" // also brings in slint-testing.h, which has no include guard

#include <Spix/Scene/Item.h>


#include <memory>
#include <optional>
#include <string>

namespace spix {

/**
 * @brief Value of a property as a test sees it, or nullopt if the element doesn't have it.
 *
 * Covers `text`, `checked`, `enabled`, `value`, `description`, `count`, `x`/`y`/`width`/`height`
 * (window coordinates, as there is no parent navigation), `visible`, raw `accessible.<name>`,
 * then the getters registered in @p hooks.
 */
std::optional<std::string> slintElementProperty(
    const slint::testing::ElementHandle& element, const std::string& name, const SlintHooks& hooks);

class SlintItem : public Item {
public:
    /**
     * @p element is the element of the item, empty for the window itself: Slint's element search
     * doesn't visit the window's own element. Test-helper handlers need an element to be called with,
     * so for the window they get @p anyElement, the first element of the window (if it has any).
     */
    SlintItem(slint::Window* window, std::optional<slint::testing::ElementHandle> element,
        std::optional<slint::testing::ElementHandle> anyElement, std::shared_ptr<SlintHooks> hooks);

    Size size() const override;
    Point position() const override;
    Rect bounds() const override;
    std::string stringProperty(const std::string& name) const override;
    void setStringProperty(const std::string& name, const std::string& value) override;
    bool invokeMethod(const std::string& method, const std::vector<Variant>& args, Variant& ret) override;
    bool visible() const override;

    slint::Window* window() const { return m_window; }
    /// Top-left of the element in window (logical) coordinates.
    Point windowPosition() const;

private:
    const std::optional<slint::testing::ElementHandle>& handlerElement() const
    {
        return m_element ? m_element : m_anyElement;
    }

    slint::Window* m_window;
    std::optional<slint::testing::ElementHandle> m_element;
    std::optional<slint::testing::ElementHandle> m_anyElement;
    std::shared_ptr<SlintHooks> m_hooks;
};

} // namespace spix
