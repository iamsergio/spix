#pragma once

#include "SlintScene.h"
#include "test_window.h"

#include <Spix/Data/ItemPath.h>

#include <gtest/gtest.h>

#include <memory>

/// A TestWindow registered as "win" in a SlintScene.
class SlintSceneTest : public ::testing::Test {
protected:
    void SetUp() override
    {
        ui->show();
        hooks = std::make_shared<spix::SlintHooks>();
        scene = std::make_unique<spix::SlintScene>(hooks);
        scene->addWindow("win",
            {&ui->window(), [ui = ui](const spix::SlintScene::ElementVisitor& visitor) {
                 slint::testing::ElementHandle::visit_elements(ui,
                     [&visitor](slint::testing::ElementHandle e) { return visitor(std::move(e)); });
             }});
    }

    void TearDown() override
    {
        scene.reset();
        ui->hide();
    }

    std::unique_ptr<spix::Item> item(const char* path) { return scene->itemAtPath(spix::ItemPath(path)); }

    std::string prop(const char* path, const char* name)
    {
        auto i = item(path);
        return i ? i->stringProperty(name) : "<not found>";
    }

    void click(const char* path, spix::MouseButton button = spix::MouseButtons::Left,
        spix::KeyModifier mod = spix::KeyModifiers::None)
    {
        auto i = item(path);
        ASSERT_TRUE(i) << path;
        auto size = i->size();
        spix::Point center {size.width / 2, size.height / 2};
        scene->events().mouseDown(i.get(), center, button, mod);
        scene->events().mouseUp(i.get(), center, button, mod);
    }

    slint::ComponentHandle<TestWindow> ui = TestWindow::create();
    std::shared_ptr<spix::SlintHooks> hooks;
    std::unique_ptr<spix::SlintScene> scene;
};
