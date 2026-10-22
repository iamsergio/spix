/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

#include <Spix/Data/Geometry.h>
#include <Spix/Data/PasteboardContent.h>
#include <Spix/Events/Identifiers.h>
#include <Spix/Scene/Item.h>

#include <string>

namespace spix {

class Events {
public:
    virtual ~Events() = default;

    virtual void mouseDown(Item* item, Point loc, MouseButton button, KeyModifier mod) = 0;
    virtual void mouseUp(Item* item, Point loc, MouseButton button, KeyModifier mod) = 0;
    /// Double-click with the given button. Scenes whose toolkit does not count clicks itself
    /// (QtQuick, QtWidgets) override this to also post the double-click event.
    virtual void mouseDoubleClick(Item* item, Point loc, MouseButton button, KeyModifier mod)
    {
        mouseDown(item, loc, button, mod);
        mouseUp(item, loc, button, mod);
        mouseDown(item, loc, button, mod);
        mouseUp(item, loc, button, mod);
    }
    virtual void mouseMove(Item* item, Point loc) = 0;
    virtual void stringInput(Item* item, const std::string& text) = 0;
    virtual void keyPress(Item* item, int keyCode, KeyModifier mod) = 0;
    virtual void keyRelease(Item* item, int keyCode, KeyModifier mod) = 0;
    virtual void extMouseDrop(Item* item, Point loc, PasteboardContent& content) = 0;
    virtual void quit() = 0;
};

} // namespace spix
