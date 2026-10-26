/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#include <gtest/gtest.h>

#include <Commands/DoubleClickOnItem.h>
#include <Scene/Mock/MockScene.h>
#include <Spix/CommandExecuter/CommandExecuter.h>
#include <Spix/Scene/Events.h>

#include <vector>

TEST(DoubleClickOnItemTest, ErrorOnMissingItem)
{
    auto command = std::make_unique<spix::cmd::DoubleClickOnItem>(
        spix::ItemPosition("window/some/item"), spix::MouseButtons::Left);

    spix::MockScene scene;
    spix::CommandExecuter exec;
    exec.enqueueCommand(std::move(command));

    // Try a couple of times to process all commands
    exec.processCommands(scene);
    exec.processCommands(scene);
    exec.processCommands(scene);

    // Check that an error is available
    EXPECT_TRUE(exec.state().hasErrors());
}

TEST(DoubleClickOnItemTest, DoubleClickItem)
{
    int doubleClicks = 0;

    auto command = std::make_unique<spix::cmd::DoubleClickOnItem>(
        spix::ItemPosition("window/some/item"), spix::MouseButtons::Left);

    spix::MockScene scene;
    scene.addItemAtPath(spix::Size(100.0, 30.0), "window/some/item");

    scene.mockEvents().onMouseDoubleClickEvent
        = [&doubleClicks](spix::Item*, spix::Point loc, spix::MouseButton button) {
              // With the left button, in the center of the item
              EXPECT_EQ(button, spix::MouseButtons::Left);
              EXPECT_TRUE(loc.x > 49.0 && loc.x < 51.0);
              EXPECT_TRUE(loc.y > 14.0 && loc.y < 16.0);
              ++doubleClicks;
          };

    spix::CommandExecuter exec;
    exec.enqueueCommand(std::move(command));
    exec.processCommands(scene);

    EXPECT_EQ(doubleClicks, 1);
}

TEST(DoubleClickOnItemTest, DefaultImplementationPressesAndReleasesTwice)
{
    std::vector<bool> presses;

    // The base implementation is what scenes that count clicks themselves (Slint) get
    struct Plain : spix::Events {
        std::vector<bool>* presses;
        void mouseDown(spix::Item*, spix::Point, spix::MouseButton, spix::KeyModifier) override
        {
            presses->push_back(true);
        }
        void mouseUp(spix::Item*, spix::Point, spix::MouseButton, spix::KeyModifier) override
        {
            presses->push_back(false);
        }
        void mouseMove(spix::Item*, spix::Point) override {}
        void mouseWheel(spix::Item*, spix::Point, spix::Point) override {}
        void stringInput(spix::Item*, const std::string&) override {}
        void keyPress(spix::Item*, int, spix::KeyModifier) override {}
        void keyRelease(spix::Item*, int, spix::KeyModifier) override {}
        void extMouseDrop(spix::Item*, spix::Point, spix::PasteboardContent&) override {}
        void quit() override {}
    } events;
    events.presses = &presses;

    events.mouseDoubleClick(nullptr, spix::Point(1, 1), spix::MouseButtons::Left, spix::KeyModifiers::None);

    EXPECT_EQ(presses, (std::vector<bool> {true, false, true, false}));
}
