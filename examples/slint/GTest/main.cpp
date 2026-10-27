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
        quit();
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
}

// How far the list has scrolled towards its end, in pixels, once it has come to rest: Slint
// animates the scroll.
static int scrolled()
{
    srv->wait(std::chrono::milliseconds(500));
    return std::stoi(srv->getStringProperty("mainWindow/scrollPosition", "text"));
}

TEST(GTestExample, MouseWheelScrollsList)
{
    const spix::ItemPath list("mainWindow/scrollList");
    const auto noWait = std::chrono::milliseconds(0);
    ASSERT_EQ(scrolled(), 0);

    // A negative y turns the wheel towards the user, which moves the content towards its end.
    srv->mouseWheel(list, 0, -120, 1, noWait);
    const int oneStep = scrolled();
    EXPECT_GT(oneStep, 0);

    // A positive y goes back.
    srv->mouseWheel(list, 0, 120, 1, noWait);
    EXPECT_EQ(scrolled(), 0);

    // Three steps, spread over an interval, go further than one.
    srv->mouseWheel(list, 0, -120, 3, std::chrono::milliseconds(50));
    EXPECT_GT(scrolled(), oneStep);

    // Bad arguments are errors.
    srv->mouseWheel(list, 0, -120, 0, noWait);
    srv->mouseWheel(list, 0, -120, 1, std::chrono::milliseconds(-1));
    srv->mouseWheel(spix::ItemPath("mainWindow/nope"), 0, -120, 1, noWait);
    EXPECT_EQ(srv->getErrors().size(), 3u);
}

TEST(GTestExample, MouseDragScrollsList)
{
    const spix::ItemPath list("mainWindow/scrollList");
    const auto noWait = std::chrono::milliseconds(0);
    const auto interval = std::chrono::milliseconds(10);
    const auto hold = std::chrono::milliseconds(200);
    const size_t errorsBefore = srv->getErrors().size();

    // The wheel test left the list scrolled, somewhere: everything below is relative to where it is.
    const auto start = scrolled();

    // Dragging the content up moves it towards its end, by the distance. The hold at the end is
    // longer than the 100 ms after which Slint's flickable ignores the pointer's velocity, so it
    // stays where the drag ends.
    srv->mouseDrag(list, spix::Point(100, 180), spix::Point(100, 80), 20, interval, hold);
    const int dragged = scrolled();
    EXPECT_GT(dragged - start, 70);
    EXPECT_LE(dragged - start, 100);
    EXPECT_EQ(scrolled(), dragged);

    // Dragging down moves it back.
    srv->mouseDrag(list, spix::Point(100, 80), spix::Point(100, 160), 16, interval, hold);
    EXPECT_LT(scrolled(), dragged - 30);
    EXPECT_GE(scrolled(), start);

    // Each move is due at its own time after the press, so the stroke lasts steps * interval +
    // hold whatever the executer's tick: never less, and not more by the lateness of every step
    // (the Qt scenes' ticks are 10 ms). The next blocking call returns after the release. The
    // margin upwards is for load.
    const auto started = std::chrono::steady_clock::now();
    srv->mouseDrag(list, spix::Point(100, 80), spix::Point(100, 160), 30, std::chrono::milliseconds(16),
        std::chrono::milliseconds(100));
    srv->getStringProperty("mainWindow/scrollPosition", "text");
    const auto took = std::chrono::steady_clock::now() - started;
    EXPECT_GE(took, std::chrono::milliseconds(580));
    EXPECT_LT(took, std::chrono::milliseconds(900));

    // Bad arguments and an unknown path are one error each.
    const int rest = scrolled();
    srv->mouseDrag(list, spix::Point(100, 180), spix::Point(100, 80), 0, noWait, noWait);
    srv->mouseDrag(list, spix::Point(100, 180), spix::Point(100, 80), 1, std::chrono::milliseconds(-1), noWait);
    srv->mouseDrag(list, spix::Point(100, 180), spix::Point(100, 80), 1, noWait, std::chrono::milliseconds(-1));
    srv->mouseDrag(spix::ItemPath("mainWindow/nope"), spix::Point(1, 1), spix::Point(1, 50), 3, noWait, noWait);
    EXPECT_EQ(scrolled(), rest);
    EXPECT_EQ(srv->getErrors().size(), errorsBefore + 4);
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
