/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

#include <Spix/Commands/Command.h>
#include <Spix/Data/Geometry.h>
#include <Spix/Data/ItemPath.h>

namespace spix {
namespace cmd {

/// One turn of the mouse wheel over the centre of an item.
class Wheel : public Command {
public:
    Wheel(ItemPath path, Point angleDelta);

    void execute(CommandEnvironment& env) override;

private:
    ItemPath m_path;
    Point m_angleDelta;
};

} // namespace cmd
} // namespace spix
