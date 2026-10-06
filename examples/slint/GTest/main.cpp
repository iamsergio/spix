/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

/**
 * Slint version of examples/qtquick/GTest: runs the UI tests using GTest.
 *
 * Keep in mind that GTest is not designed for UI testing and that the
 * order of the test execution is not guaranteed. Thus, you should only
 * have one test per executable.
 *
 * The Qt example also clicks items found through property selectors (".propertyWithTarget"),
 * which can't be resolved in compiled Slint, so that part isn't ported.
 */

#include "main.h"

#include <Spix/Events/Identifiers.h>
#include <Spix/SlintBot.h>

#include <atomic>
#include <gtest/gtest.h>

class SpixGTest;
static SpixGTest* srv;

class SpixGTest : public spix::TestServer {
public:
    SpixGTest(int argc, char* argv[])
    {
        m_argc = argc;
        m_argv = argv;
    }

    int testResult() { return m_result.load(); }

protected:
    int m_argc;
    char** m_argv;
    std::atomic<int> m_result {0};

    void executeTest() override
    {
        srv = this;
        ::testing::InitGoogleTest(&m_argc, m_argv);
        auto testResult = RUN_ALL_TESTS();
        m_result.store(testResult);
    }
};

TEST(GTestExample, BasicUITest)
{
    const auto delay = std::chrono::milliseconds(100);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"));
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"));
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"));
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"));
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"));
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"));
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"), spix::MouseButtons::Right);
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"), spix::MouseButtons::Left, spix::KeyModifiers::Shift);
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"), spix::MouseButtons::Left, spix::KeyModifiers::Control);
    srv->wait(delay);
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"), spix::MouseButtons::Left,
        spix::KeyModifiers::Shift | spix::KeyModifiers::Control);
    srv->wait(delay);

    auto result = srv->getStringProperty("mainWindow/results", "text");

    auto expected_result = R"RSLT(Button 1 clicked
Button 2 clicked
Button 2 clicked
Button 1 clicked
Button 2 clicked
Button 1 clicked
Button 1 right clicked
Button 2 shift clicked
Button 2 control clicked
Button 2 shift control clicked)RSLT";

    EXPECT_EQ(result, expected_result);

    EXPECT_EQ(srv->getErrors(), std::vector<std::string> {});

    srv->quit();
}

int main(int argc, char* argv[])
{
    auto ui = MainWindow::create();
    ui->show();

    // Instantiate and run tests
    SpixGTest tests(argc, argv);
    spix::SlintBot bot;
    bot.addWindow("mainWindow", ui);
    bot.runTestServer(tests);

    slint::run_event_loop();
    return tests.testResult();
}
