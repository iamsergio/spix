/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include "SlintEvents.h"

#include "SlintItem.h"

#include <Spix/Data/PasteboardContent.h>

#include <slint.h>

#include <string>
#include <vector>

namespace spix {

namespace {

namespace kc = slint::platform::key_codes;

std::string utf8(std::u8string_view s)
{
    return std::string(reinterpret_cast<const char*>(s.data()), s.size());
}

/// Spix key codes are Qt's. Returns the text Slint expects in key events, empty if unsupported.
std::string slintKeyText(int keyCode, KeyModifier mod)
{
    switch (keyCode) {
    case KeyCodes::Escape: return utf8(kc::Escape);
    case KeyCodes::Tab: return utf8(kc::Tab);
    case KeyCodes::Backtab: return utf8(kc::Backtab);
    case KeyCodes::Backspace: return utf8(kc::Backspace);
    case KeyCodes::Return:
    case KeyCodes::Enter: return utf8(kc::Return);
    case KeyCodes::Insert: return utf8(kc::Insert);
    case KeyCodes::Delete: return utf8(kc::Delete);
    case KeyCodes::Pause: return utf8(kc::Pause);
    case KeyCodes::SysReq: return utf8(kc::SysReq);
    case KeyCodes::Home: return utf8(kc::Home);
    case KeyCodes::End: return utf8(kc::End);
    case KeyCodes::Left: return utf8(kc::LeftArrow);
    case KeyCodes::Up: return utf8(kc::UpArrow);
    case KeyCodes::Right: return utf8(kc::RightArrow);
    case KeyCodes::Down: return utf8(kc::DownArrow);
    case KeyCodes::PageUp: return utf8(kc::PageUp);
    case KeyCodes::PageDown: return utf8(kc::PageDown);
    case KeyCodes::Shift: return utf8(kc::Shift);
    case KeyCodes::Control: return utf8(kc::Control);
    case KeyCodes::Meta: return utf8(kc::Meta);
    case KeyCodes::Alt: return utf8(kc::Alt);
    case KeyCodes::CapsLock: return utf8(kc::CapsLock);
    case KeyCodes::ScrollLock: return utf8(kc::ScrollLock);
    case KeyCodes::Menu: return utf8(kc::Menu);
    default: break;
    }

    if (keyCode >= KeyCodes::F1 && keyCode <= KeyCodes::F24) {
        // The function keys are consecutive in Slint's private use area, F1 = U+F704.
        char32_t cp = 0xf704 + (keyCode - KeyCodes::F1);
        std::string s;
        s += static_cast<char>(0xe0 | (cp >> 12));
        s += static_cast<char>(0x80 | ((cp >> 6) & 0x3f));
        s += static_cast<char>(0x80 | (cp & 0x3f));
        return s;
    }

    // Printable ASCII: Qt's key codes for letters are the uppercase letters.
    if (keyCode >= 0x20 && keyCode <= 0x7e) {
        char c = static_cast<char>(keyCode);
        if (c >= 'A' && c <= 'Z' && !(mod & KeyModifiers::Shift))
            c = static_cast<char>(c - 'A' + 'a');
        return std::string(1, c);
    }
    return {};
}

std::vector<std::string> modifierKeys(KeyModifier mod)
{
    std::vector<std::string> keys;
    if (mod & KeyModifiers::Shift)
        keys.push_back(utf8(kc::Shift));
    if (mod & KeyModifiers::Control)
        keys.push_back(utf8(kc::Control));
    if (mod & KeyModifiers::Alt)
        keys.push_back(utf8(kc::Alt));
    if (mod & KeyModifiers::Meta)
        keys.push_back(utf8(kc::Meta));
    return keys;
}

void pressModifiers(slint::Window& window, KeyModifier mod)
{
    for (const auto& key : modifierKeys(mod))
        window.dispatch_key_press_event(slint::SharedString(key));
}

void releaseModifiers(slint::Window& window, KeyModifier mod)
{
    auto keys = modifierKeys(mod);
    for (auto it = keys.rbegin(); it != keys.rend(); ++it)
        window.dispatch_key_release_event(slint::SharedString(*it));
}

std::optional<slint::PointerEventButton> slintButton(MouseButton button)
{
    if (button & MouseButtons::Left)
        return slint::PointerEventButton::Left;
    if (button & MouseButtons::Right)
        return slint::PointerEventButton::Right;
    if (button & MouseButtons::Middle)
        return slint::PointerEventButton::Middle;
    return std::nullopt;
}

slint::LogicalPosition windowPos(const SlintItem& item, Point loc)
{
    auto origin = item.windowPosition();
    return slint::LogicalPosition({static_cast<float>(origin.x + loc.x), static_cast<float>(origin.y + loc.y)});
}

} // namespace

void SlintEvents::mouseDown(Item* item, Point loc, MouseButton button, KeyModifier mod)
{
    auto* slintItem = dynamic_cast<SlintItem*>(item);
    auto slintButtonValue = slintButton(button);
    if (!slintItem || !slintButtonValue)
        return;
    auto& window = *slintItem->window();
    // Pointer events carry no modifiers, Slint tracks them from key events.
    pressModifiers(window, mod);
    window.dispatch_pointer_press_event(windowPos(*slintItem, loc), *slintButtonValue);
}

void SlintEvents::mouseUp(Item* item, Point loc, MouseButton button, KeyModifier mod)
{
    auto* slintItem = dynamic_cast<SlintItem*>(item);
    auto slintButtonValue = slintButton(button);
    if (!slintItem || !slintButtonValue)
        return;
    auto& window = *slintItem->window();
    window.dispatch_pointer_release_event(windowPos(*slintItem, loc), *slintButtonValue);
    releaseModifiers(window, mod);
}

void SlintEvents::mouseMove(Item* item, Point loc)
{
    auto* slintItem = dynamic_cast<SlintItem*>(item);
    if (!slintItem)
        return;
    slintItem->window()->dispatch_pointer_move_event(windowPos(*slintItem, loc));
}

void SlintEvents::stringInput(Item* item, const std::string& text)
{
    auto* slintItem = dynamic_cast<SlintItem*>(item);
    if (!slintItem)
        return;
    auto& window = *slintItem->window();

    // One key event per code point, as a keyboard would send them.
    size_t i = 0;
    while (i < text.size()) {
        unsigned char lead = static_cast<unsigned char>(text[i]);
        size_t len = lead < 0x80 ? 1 : lead >= 0xf0 ? 4 : lead >= 0xe0 ? 3 : 2;
        slint::SharedString key(std::string_view(text).substr(i, len));
        window.dispatch_key_press_event(key);
        window.dispatch_key_release_event(key);
        i += len;
    }
}

void SlintEvents::keyPress(Item* item, int keyCode, KeyModifier mod)
{
    auto* slintItem = dynamic_cast<SlintItem*>(item);
    if (!slintItem)
        return;
    auto text = slintKeyText(keyCode, mod);
    if (text.empty())
        return;
    auto& window = *slintItem->window();
    pressModifiers(*slintItem->window(), mod);
    window.dispatch_key_press_event(slint::SharedString(text));
}

void SlintEvents::keyRelease(Item* item, int keyCode, KeyModifier mod)
{
    auto* slintItem = dynamic_cast<SlintItem*>(item);
    if (!slintItem)
        return;
    auto text = slintKeyText(keyCode, mod);
    if (text.empty())
        return;
    auto& window = *slintItem->window();
    window.dispatch_key_release_event(slint::SharedString(text));
    releaseModifiers(window, mod);
}

void SlintEvents::extMouseDrop(Item*, Point, PasteboardContent&)
{
    // External drops (file URLs from outside the app) can't be injected through Slint's window API.
}

void SlintEvents::quit()
{
    slint::quit_event_loop();
}

} // namespace spix
