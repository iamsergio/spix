/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

/**
 * The test body of the "Basic" example, shared by the QtQuick and the Slint versions.
 * It only uses the Spix API, so it runs unchanged against both UIs.
 *
 * The UI must have a window named "mainWindow" with two buttons, "Button_1" and "Button_2",
 * which append a line to the "results" text when clicked with the left or right mouse button.
 */

#include <Spix/Events/Identifiers.h>
#include <Spix/TestServer.h>

#include <atomic>
#include <chrono>
#include <iostream>
#include <string>

class BasicTests : public spix::TestServer {
public:
    /// If true, the application quits once the test is done.
    explicit BasicTests(bool quitWhenDone = false)
    : m_quitWhenDone(quitWhenDone)
    {
    }

    bool passed() const { return m_passed.load(); }

protected:
    void executeTest() override
    {
        const auto delay = std::chrono::milliseconds(100);
        mouseClick(spix::ItemPath("mainWindow/Button_1"));
        wait(delay);
        mouseClick(spix::ItemPath("mainWindow/Button_2"));
        wait(delay);
        mouseClick(spix::ItemPath("mainWindow/Button_2"));
        wait(delay);
        mouseClick(spix::ItemPath("mainWindow/Button_1"));
        wait(delay);
        mouseClick(spix::ItemPath("mainWindow/Button_2"));
        wait(delay);
        mouseClick(spix::ItemPath("mainWindow/Button_1"));
        wait(delay);
        mouseClick(spix::ItemPath("mainWindow/Button_1"), spix::MouseButtons::Right);
        wait(delay);

        auto result = getStringProperty("mainWindow/results", "text");
        std::cout << "-------\nResult:\n-------\n" << result << "\n-------" << std::endl;

        const std::string expected = "Button 1 clicked\nButton 2 clicked\nButton 2 clicked\nButton 1 clicked\n"
                                     "Button 2 clicked\nButton 1 clicked\nButton 1 right clicked";
        m_passed = (result == expected) && getErrors().empty();
        if (!m_passed)
            std::cout << "FAILED, expected:\n" << expected << std::endl;

        if (m_quitWhenDone)
            quit();
    }

private:
    bool m_quitWhenDone;
    std::atomic<bool> m_passed {false};
};
