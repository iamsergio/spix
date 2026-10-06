/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include <Spix/SlintBot.h>

#include "SlintHooks.h"
#include "SlintScene.h"

#include <Spix/CommandExecuter/CommandExecuter.h>

#include <chrono>
#include <iostream>

namespace spix {

struct SlintBot::Private {
    std::shared_ptr<SlintHooks> hooks = std::make_shared<SlintHooks>();
    SlintScene scene {hooks};
    CommandExecuter cmdExec;
    slint::Timer timer;
};

SlintBot::SlintBot()
: d(std::make_unique<Private>())
{
    d->timer.start(slint::TimerMode::Repeated, std::chrono::milliseconds(10),
        [this] { d->cmdExec.processCommands(d->scene); });
}

SlintBot::~SlintBot() = default;

void SlintBot::addWindowImpl(std::string name, slint::Window* window, ElementVisit visit)
{
    d->scene.addWindow(std::move(name), SlintScene::Window {window, std::move(visit)});
}

void SlintBot::removeWindow(const std::string& name)
{
    d->scene.removeWindow(name);
}

void SlintBot::runTestServer(TestServer& server)
{
    std::cout << "Spix server is enabled. Only use this in a safe environment." << std::endl;
    server.setCommandExecuter(&d->cmdExec);
    server.start();
}

void SlintBot::registerPropertyGetter(std::string name, SlintPropertyGetter getter)
{
    d->hooks->getters[std::move(name)] = std::move(getter);
}

void SlintBot::registerPropertySetter(std::string name, SlintPropertySetter setter)
{
    d->hooks->setters[std::move(name)] = std::move(setter);
}

void SlintBot::registerMethod(std::string name, SlintMethodHandler handler)
{
    d->hooks->methods[std::move(name)] = std::move(handler);
}

std::vector<std::string> SlintBot::warnings() const
{
    return d->hooks->warnings;
}

} // namespace spix
