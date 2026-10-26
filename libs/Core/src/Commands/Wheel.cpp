/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include "Wheel.h"

#include <Spix/Scene/Scene.h>

namespace spix {
namespace cmd {

Wheel::Wheel(ItemPath path, Point angleDelta)
: m_path(std::move(path))
, m_angleDelta(angleDelta)
{
}

void Wheel::execute(CommandEnvironment& env)
{
    auto item = env.scene().itemAtPath(m_path);

    if (!item) {
        env.state().reportError("Wheel: Item not found: " + m_path.string());
        return;
    }

    auto size = item->size();
    Point midPoint(size.width / 2.0, size.height / 2.0);
    env.scene().events().mouseWheel(item.get(), midPoint, m_angleDelta);
}

} // namespace cmd
} // namespace spix
