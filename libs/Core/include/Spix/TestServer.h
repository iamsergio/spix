/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

#include <chrono>
#include <functional>
#include <memory>
#include <thread>

#include <Spix/Data/Geometry.h>
#include <Spix/Data/ItemPath.h>
#include <Spix/Data/Variant.h>
#include <Spix/Events/Identifiers.h>

#include <Spix/spix_core_export.h>

namespace spix {

class CommandExecuter;

/**
 * @brief Base class that serves a test to the bot.
 *
 * A `TestServer` can be run by the bot and serves the
 * test to the bot. It can be a simple test case written
 * in C++ or it can be an actual server that handles
 * incoming RPC calls.
 * For basic C++ based tests, derive from this class and overide
 * the `executeTest` method.
 *
 * The code in `executeTest` is executed in its own thread so that
 * it does not affect the execution of the main application.
 */
class SPIXCORE_EXPORT TestServer {
public:
    virtual ~TestServer();

    void start();
    void setCommandExecuter(CommandExecuter* exec);

    void setGenericCommandHandler(std::function<void(std::string, std::string)> handler);

    // Commands
    void wait(std::chrono::milliseconds waitTime);
    void mouseClick(ItemPath path);

    void mouseClick(ItemPath path, Point proportion);
    void mouseClick(ItemPath path, Point proportion, Point offset);
    void mouseClick(ItemPath path, MouseButton mouseButton, KeyModifier keyModifier = spix::KeyModifiers::None);

    void mouseDoubleClick(ItemPath path);

    /// Turns the mouse wheel `steps` times (at least 1) over the centre of the item, `interval`
    /// apart while the app keeps running. `angleDelta` is in Qt's `QWheelEvent::angleDelta()`
    /// units, see Events::mouseWheel. Returns once the commands are queued.
    void mouseWheel(ItemPath path, int angleDeltaX, int angleDeltaY, int steps, std::chrono::milliseconds interval);

    /// Drags with the left button over the item: presses at `from`, moves `steps` times (at least 1)
    /// along the straight line to `to`, each by the same distance, holds still at `to` for `hold` so
    /// that a flickable sees no velocity on release, and releases. The points are in logical pixels
    /// from the item's top-left. Move k is due `k * interval` after the press and the release
    /// `steps * interval + hold` after it, so the velocity is constant and the stroke lasts that
    /// long however coarse the executer's ticks are: a move can run up to one tick late, but a late
    /// move doesn't delay the later ones. The whole gesture runs inside the app. Returns once the
    /// commands are queued.
    void mouseDrag(ItemPath path, Point from, Point to, int steps, std::chrono::milliseconds interval,
        std::chrono::milliseconds hold);

    void mouseBeginDrag(ItemPath path);
    void mouseEndDrag(ItemPath path);
    void mouseDropUrls(ItemPath path, const std::vector<std::string>& urls);
    void genericCommand(std::string command, std::string payload);

    void inputText(ItemPath path, std::string text);
    void enterKey(ItemPath path, int keyCode, unsigned modifiers);

    std::string getStringProperty(ItemPath path, std::string propertyName);
    void setStringProperty(ItemPath path, std::string propertyName, std::string propertyValue);
    Variant invokeMethod(ItemPath path, std::string method, std::vector<Variant> args);
    Rect getBoundingBox(ItemPath path);
    bool existsAndVisible(ItemPath path);
    std::vector<std::string> getErrors();
    bool waitForItem(ItemPath path, std::chrono::milliseconds maxWaitTime);

    void takeScreenshot(ItemPath targetItem, std::string filePath);
    std::string takeScreenshotAsBase64(ItemPath targetItem);
    void quit();

protected:
    virtual void executeTest() = 0;

private:
    CommandExecuter* m_cmdExec;
    std::thread m_thread;
    std::function<void(std::string, std::string)> m_handler;
};

} // namespace spix
