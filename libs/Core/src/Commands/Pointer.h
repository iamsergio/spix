/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

#include <Spix/Commands/Command.h>
#include <Spix/Data/Geometry.h>
#include <Spix/Data/ItemPath.h>

#include <chrono>
#include <memory>

namespace spix {
namespace cmd {

/// What the commands of one stroke share: whether it failed, and when it started.
struct Stroke {
    bool failed = false;
    std::chrono::steady_clock::time_point start;
};

/// One event of a left-button stroke at a point relative to the item's top-left: the press, a
/// move with the button down, or the release. The commands of one stroke share a flag: when one of
/// them can't find the item it reports the error and the others skip, so a stroke that can't
/// start leaves one error and no stray events. The press records the start time that
/// `StrokeWait` counts from.
class Pointer : public Command {
public:
    enum class Action
    {
        Press,
        Move,
        Release
    };

    Pointer(ItemPath path, Action action, Point offset, std::shared_ptr<Stroke> stroke);

    void execute(CommandEnvironment& env) override;

private:
    ItemPath m_path;
    Action m_action;
    Point m_offset;
    std::shared_ptr<Stroke> m_stroke;
};

/// Waits until `offset` after the press of the stroke. Each wait counts from the press and not from
/// the one before, so a command that runs late doesn't push the later ones: the stroke lasts as
/// long as its last wait says, however coarse the executer's ticks are.
class StrokeWait : public Command {
public:
    StrokeWait(std::shared_ptr<Stroke> stroke, std::chrono::milliseconds offset);

    void execute(CommandEnvironment& env) override;
    bool canExecuteNow(CommandEnvironment& env) override;

private:
    std::shared_ptr<Stroke> m_stroke;
    std::chrono::milliseconds m_offset;
};

} // namespace cmd
} // namespace spix
