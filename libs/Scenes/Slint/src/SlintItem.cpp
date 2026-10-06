/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include "SlintItem.h"

#include <algorithm>
#include <sstream>

namespace spix {

namespace {

using slint::testing::ElementHandle;

std::string str(const slint::SharedString& s)
{
    return std::string(std::string_view(s));
}

std::string boolStr(bool b)
{
    return b ? "true" : "false";
}

std::string numStr(double d)
{
    std::ostringstream os;
    os << d;
    return os.str();
}

template <typename T, typename F>
std::optional<std::string> mapOpt(const std::optional<T>& o, F&& f)
{
    if (!o)
        return std::nullopt;
    return f(*o);
}

const char* roleName(slint::language::AccessibleRole role)
{
    using R = slint::language::AccessibleRole;
    switch (role) {
    case R::None: return "none";
    case R::Button: return "button";
    case R::Checkbox: return "checkbox";
    case R::Combobox: return "combobox";
    case R::Groupbox: return "groupbox";
    case R::Image: return "image";
    case R::List: return "list";
    case R::Slider: return "slider";
    case R::Spinbox: return "spinbox";
    case R::Tab: return "tab";
    case R::TabList: return "tab-list";
    case R::TabPanel: return "tab-panel";
    case R::Text: return "text";
    case R::Table: return "table";
    case R::Tree: return "tree";
    case R::ProgressIndicator: return "progress-indicator";
    case R::TextInput: return "text-input";
    case R::Switch: return "switch";
    case R::ListItem: return "list-item";
    case R::RadioButton: return "radio-button";
    default: return "other";
    }
}

std::optional<std::string> accessibleProperty(const ElementHandle& e, const std::string& name)
{
    auto s = [](const slint::SharedString& v) { return str(v); };
    if (name == "label")
        return mapOpt(e.accessible_label(), s);
    if (name == "value")
        return mapOpt(e.accessible_value(), s);
    if (name == "placeholder-text")
        return mapOpt(e.accessible_placeholder_text(), s);
    if (name == "description")
        return mapOpt(e.accessible_description(), s);
    if (name == "id")
        return mapOpt(e.accessible_id(), s);
    if (name == "role")
        return mapOpt(e.accessible_role(), [](auto r) { return std::string(roleName(r)); });
    if (name == "enabled")
        return mapOpt(e.accessible_enabled(), boolStr);
    if (name == "checked")
        return mapOpt(e.accessible_checked(), boolStr);
    if (name == "checkable")
        return mapOpt(e.accessible_checkable(), boolStr);
    if (name == "item-selected")
        return mapOpt(e.accessible_item_selected(), boolStr);
    if (name == "item-selectable")
        return mapOpt(e.accessible_item_selectable(), boolStr);
    if (name == "expanded")
        return mapOpt(e.accessible_expanded(), boolStr);
    if (name == "expandable")
        return mapOpt(e.accessible_expandable(), boolStr);
    if (name == "read-only")
        return mapOpt(e.accessible_read_only(), boolStr);
    if (name == "value-maximum")
        return mapOpt(e.accessible_value_maximum(), numStr);
    if (name == "value-minimum")
        return mapOpt(e.accessible_value_minimum(), numStr);
    if (name == "value-step")
        return mapOpt(e.accessible_value_step(), numStr);
    if (name == "item-index")
        return mapOpt(e.accessible_item_index(), numStr);
    if (name == "item-count")
        return mapOpt(e.accessible_item_count(), numStr);
    if (name == "orientation")
        return mapOpt(e.accessible_orientation(), [](auto o) {
            return std::string(o == slint::language::Orientation::Horizontal ? "horizontal" : "vertical");
        });
    return std::nullopt;
}

} // namespace

void SlintHooks::warn(std::string message)
{
    // Lookups run per element and per command, report each problem once.
    if (std::find(warnings.begin(), warnings.end(), message) == warnings.end())
        warnings.push_back(std::move(message));
}

std::optional<std::string> slintElementProperty(
    const ElementHandle& element, const std::string& name, const SlintHooks& hooks)
{
    auto s = [](const slint::SharedString& v) { return str(v); };

    if (name == "text") {
        // Text inputs expose their text as accessible value, everything else (Text, Button) as label.
        if (auto value = element.accessible_value())
            return str(*value);
        return mapOpt(element.accessible_label(), s);
    }
    if (name == "value")
        return mapOpt(element.accessible_value(), s);
    if (name == "description")
        return mapOpt(element.accessible_description(), s);
    if (name == "checked")
        return mapOpt(element.accessible_checked(), boolStr);
    if (name == "enabled")
        return mapOpt(element.accessible_enabled(), boolStr);
    if (name == "count")
        return mapOpt(element.accessible_item_count(), numStr);
    // The element search only visits visible elements.
    if (name == "visible")
        return std::string("true");
    if (name == "x")
        return numStr(element.absolute_position().x);
    if (name == "y")
        return numStr(element.absolute_position().y);
    if (name == "width")
        return numStr(element.size().width);
    if (name == "height")
        return numStr(element.size().height);
    if (name.rfind("accessible.", 0) == 0)
        return accessibleProperty(element, name.substr(11));

    auto it = hooks.getters.find(name);
    if (it != hooks.getters.end())
        return it->second(element);
    return std::nullopt;
}

SlintItem::SlintItem(
    slint::Window* window, std::optional<ElementHandle> element, std::optional<ElementHandle> anyElement,
    std::shared_ptr<SlintHooks> hooks)
: m_window(window)
, m_element(std::move(element))
, m_anyElement(std::move(anyElement))
, m_hooks(std::move(hooks))
{
}

Size SlintItem::size() const
{
    if (m_element) {
        auto s = m_element->size();
        return Size {s.width, s.height};
    }
    auto s = m_window->size();
    auto scale = m_window->scale_factor();
    return Size {s.width / scale, s.height / scale};
}

Point SlintItem::windowPosition() const
{
    if (!m_element)
        return Point {0, 0};
    auto p = m_element->absolute_position();
    return Point {p.x, p.y};
}

Point SlintItem::position() const
{
    // Same as Qt's mapToGlobal: window position on screen plus position in the window.
    auto winPos = m_window->position();
    auto scale = m_window->scale_factor();
    auto local = windowPosition();
    return Point {winPos.x / scale + local.x, winPos.y / scale + local.y};
}

Rect SlintItem::bounds() const
{
    Rect r;
    r.topLeft = position();
    r.size = size();
    return r;
}

std::string SlintItem::stringProperty(const std::string& name) const
{
    if (!m_element) {
        // The window: no accessibility properties, but its geometry and the registered getters.
        if (name == "width")
            return numStr(size().width);
        if (name == "height")
            return numStr(size().height);
        if (name == "visible")
            return boolStr(visible());
        if (name == "x" || name == "y")
            return "0";
        auto getter = m_hooks->getters.find(name);
        if (m_anyElement && getter != m_hooks->getters.end())
            return getter->second(*m_anyElement).value_or(std::string());
        return {};
    }
    return slintElementProperty(*m_element, name, *m_hooks).value_or(std::string());
}

void SlintItem::setStringProperty(const std::string& name, const std::string& value)
{
    if (!m_element) {
        auto it = m_hooks->setters.find(name);
        if (m_anyElement && it != m_hooks->setters.end()) {
            if (!it->second(*m_anyElement, value))
                m_hooks->warn("setStringProperty('" + name + "'): handler rejected the value");
        } else {
            m_hooks->warn("setStringProperty: property '" + name + "' is not settable on a Slint window");
        }
        return;
    }
    if (name == "text" || name == "value") {
        m_element->set_accessible_value(slint::SharedString(value));
        return;
    }
    if (name == "checked") {
        auto checked = m_element->accessible_checked();
        if (!checked) {
            m_hooks->warn("setStringProperty('checked'): element is not checkable");
            return;
        }
        if (*checked != (value == "true"))
            m_element->invoke_accessible_default_action();
        return;
    }
    auto it = m_hooks->setters.find(name);
    if (it != m_hooks->setters.end()) {
        if (!it->second(*m_element, value))
            m_hooks->warn("setStringProperty('" + name + "'): handler rejected the value");
        return;
    }
    m_hooks->warn("setStringProperty: property '" + name + "' is not settable on a Slint element");
}

bool SlintItem::invokeMethod(const std::string& method, const std::vector<Variant>& args, Variant& ret)
{
    if (!m_element) {
        auto it = m_hooks->methods.find(method);
        return m_anyElement && it != m_hooks->methods.end() && it->second(*m_anyElement, args, ret);
    }
    if (method == "click") {
        m_element->invoke_accessible_default_action();
        return true;
    }
    if (method == "increment") {
        m_element->invoke_accessible_increment_action();
        return true;
    }
    if (method == "decrement") {
        m_element->invoke_accessible_decrement_action();
        return true;
    }
    if (method == "expand") {
        m_element->invoke_accessible_expand_action();
        return true;
    }
    auto it = m_hooks->methods.find(method);
    if (it != m_hooks->methods.end())
        return it->second(*m_element, args, ret);
    return false;
}

bool SlintItem::visible() const
{
    // Hidden elements are never found in the first place, so only the window itself can be hidden.
    return m_window->is_visible();
}

} // namespace spix
