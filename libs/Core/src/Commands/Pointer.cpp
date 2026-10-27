/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include "Pointer.h"

#include <Spix/Scene/Scene.h>

namespace spix {
namespace cmd {

Pointer::Pointer(ItemPath path, Action action, Point offset, std::shared_ptr<Stroke> stroke)
: m_path(std::move(path))
, m_action(action)
, m_offset(offset)
, m_stroke(std::move(stroke))
{
}

void Pointer::execute(CommandEnvironment& env)
{
    if (m_stroke->failed)
        return;

    auto item = env.scene().itemAtPath(m_path);
    if (!item) {
        m_stroke->failed = true;
        env.state().reportError("Drag: Item not found: " + m_path.string());
        return;
    }

    auto& events = env.scene().events();
    switch (m_action) {
    case Action::Press:
        m_stroke->start = std::chrono::steady_clock::now();
        events.mouseDown(item.get(), m_offset, MouseButtons::Left, KeyModifiers::None);
        break;
    case Action::Move:
        events.mouseMove(item.get(), m_offset);
        break;
    case Action::Release:
        events.mouseUp(item.get(), m_offset, MouseButtons::Left, KeyModifiers::None);
        break;
    }
}

StrokeWait::StrokeWait(std::shared_ptr<Stroke> stroke, std::chrono::milliseconds offset)
: m_stroke(std::move(stroke))
, m_offset(offset)
{
}

void StrokeWait::execute(CommandEnvironment&)
{
}

bool StrokeWait::canExecuteNow(CommandEnvironment&)
{
    // A stroke that failed has nothing to wait for.
    return m_stroke->failed || std::chrono::steady_clock::now() >= m_stroke->start + m_offset;
}

} // namespace cmd
} // namespace spix
