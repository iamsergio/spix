/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

/**
 * This is a very basic example to demonstrate how to run your UI tests
 * using GTest. It can be useful when you have to make sure that your
 * UI tests work well in an existing, GTest based environment.
 *
 * Keep in mind that GTest is not designed for UI testing and that the
 * order of the test execution is not guaranteed. Thus, you should only
 * have one test per executable.
 */

#include <QGuiApplication>
#include <QQmlApplicationEngine>
#include <Spix/Events/Identifiers.h>
#include <Spix/QtQmlBot.h>

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
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_1"), spix::MouseButtons::Right);
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"), spix::MouseButtons::Left, spix::KeyModifiers::Shift);
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"), spix::MouseButtons::Left, spix::KeyModifiers::Control);
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/Button_2"), spix::MouseButtons::Left,
        spix::KeyModifiers::Shift | spix::KeyModifiers::Control);
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/propItem/.propertyWithTarget"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/.propertyWithTarget"));
    srv->wait(std::chrono::milliseconds(500));
    srv->mouseClick(spix::ItemPath("mainWindow/.propertyWithParent/Button_2"));
    srv->wait(std::chrono::milliseconds(500));

    srv->wait(std::chrono::milliseconds(500));

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
Button 2 shift control clicked
Button 1 clicked
Button 1 clicked
Button 2 clicked)RSLT";

    EXPECT_EQ(result, expected_result);

    EXPECT_EQ(srv->getErrors(), std::vector<std::string> {});
}

// The position of the list's content, in pixels from its start, once it has come to rest: the wheel
// events are posted to the window, and a Flickable glides on after the last one.
static double contentY()
{
    srv->wait(std::chrono::milliseconds(1000));
    return std::stod(srv->getStringProperty("mainWindow/scrollList", "contentY"));
}

TEST(GTestExample, MouseWheelScrollsList)
{
    const spix::ItemPath list("mainWindow/scrollList");
    const auto noWait = std::chrono::milliseconds(0);
    ASSERT_EQ(contentY(), 0.0);

    // A negative y turns the wheel towards the user, which moves the content towards its end.
    srv->mouseWheel(list, 0, -120, 1, noWait);
    const double oneStep = contentY();
    EXPECT_GT(oneStep, 0.0);

    // A positive y goes back.
    srv->mouseWheel(list, 0, 120, 1, noWait);
    EXPECT_LT(contentY(), oneStep);

    // Back at the start, three steps, spread over an interval, go further than one.
    srv->mouseWheel(list, 0, 120, 5, noWait);
    ASSERT_EQ(contentY(), 0.0);
    srv->mouseWheel(list, 0, -120, 3, std::chrono::milliseconds(50));
    EXPECT_GT(contentY(), oneStep);

    // Bad arguments are errors.
    srv->mouseWheel(list, 0, -120, 0, noWait);
    srv->mouseWheel(list, 0, -120, 1, std::chrono::milliseconds(-1));
    srv->mouseWheel(spix::ItemPath("mainWindow/nope"), 0, -120, 1, noWait);
    EXPECT_EQ(srv->getErrors().size(), 3u);
}

int main(int argc, char* argv[])
{
    // Init Qt Qml Application
    QGuiApplication app(argc, argv);
    QQmlApplicationEngine engine;
    engine.load(QUrl(QStringLiteral("qrc:/main.qml")));
    if (engine.rootObjects().isEmpty())
        return -1;

    // Instantiate and run tests
    SpixGTest tests(argc, argv);
    auto bot = new spix::QtQmlBot();
    bot->runTestServer(tests);

    app.exec();
    return tests.testResult();
}
