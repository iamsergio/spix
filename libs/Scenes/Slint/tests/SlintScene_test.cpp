#include "SlintTestFixture.h"

#include <Spix/Data/PasteboardContent.h>

#include <fstream>
#include <iterator>

using spix::KeyCodes;
using spix::KeyModifiers;
using spix::MouseButtons;

// Path lookup

TEST_F(SlintSceneTest, WindowByName)
{
    auto window = item("win");
    ASSERT_TRUE(window);
    EXPECT_EQ(window->size().width, 400);
    EXPECT_EQ(window->size().height, 300);
    EXPECT_FALSE(item("nope"));
    EXPECT_FALSE(item("win/nope"));
}

TEST_F(SlintSceneTest, FindsByAccessibleId)
{
    auto label = item("win/label");
    ASSERT_TRUE(label);
    EXPECT_EQ(label->stringProperty("text"), "Hello");
}

TEST_F(SlintSceneTest, NestingSelectsTheRightParent)
{
    EXPECT_EQ(prop("win/panel1/ok", "text"), "OK one");
    EXPECT_EQ(prop("win/panel2/ok", "text"), "OK two");
    // First match in tree order wins when not nested
    EXPECT_EQ(prop("win/ok", "text"), "OK one");
}

TEST_F(SlintSceneTest, NestedItemOutsideParentIsNotFound)
{
    // "label" exists, but not inside panel1
    EXPECT_FALSE(item("win/panel1/label"));
}

TEST_F(SlintSceneTest, IdsInRepeater)
{
    EXPECT_EQ(prop("win/row", "text"), "row 0");
    EXPECT_EQ(prop("win/(text=row 1)", "text"), "row 1");
    EXPECT_EQ(prop("win/\"row 2\"", "text"), "row 2");
    EXPECT_FALSE(item("win/\"row 3\""));
}

TEST_F(SlintSceneTest, RepeaterItemsHaveTheirOwnGeometry)
{
    EXPECT_EQ(item("win/\"row 2\"")->bounds().topLeft.x, 120 + item("win")->bounds().topLeft.x);
    EXPECT_EQ(item("win/\"row 2\"")->size().width, 50);
}

TEST_F(SlintSceneTest, HiddenElementsAreNotFound)
{
    EXPECT_FALSE(item("win/hidden"));
}

TEST_F(SlintSceneTest, TypeSelector)
{
    EXPECT_EQ(prop("win/#Text", "text"), "Hello");
    EXPECT_FALSE(item("win/#NoSuchType"));
}

TEST_F(SlintSceneTest, PropertySelectorIsUnsupported)
{
    EXPECT_FALSE(item("win/.something"));
    ASSERT_EQ(hooks->warnings.size(), 1u);
    EXPECT_NE(hooks->warnings[0].find("not supported"), std::string::npos);
}

TEST_F(SlintSceneTest, PropertyValueSelector)
{
    EXPECT_EQ(prop("win/(checked=false)", "accessible.id"), "check");
    EXPECT_EQ(prop("win/(enabled=true)", "accessible.id") != "<not found>", true);
}

// Properties

TEST_F(SlintSceneTest, PropertyMapping)
{
    EXPECT_EQ(prop("win/check", "checked"), "false");
    EXPECT_EQ(prop("win/check", "enabled"), "true");
    EXPECT_EQ(prop("win/check", "text"), "check me");
    EXPECT_EQ(prop("win/check", "accessible.role"), "checkbox");
    EXPECT_EQ(prop("win/check", "accessible.id"), "check");
    EXPECT_EQ(prop("win/slider", "value"), "5");
    EXPECT_EQ(prop("win/slider", "accessible.value-maximum"), "10");
    EXPECT_EQ(prop("win/label", "visible"), "true");
    EXPECT_EQ(prop("win/label", "x"), "0");
    EXPECT_EQ(prop("win/label", "y"), "130");
    EXPECT_EQ(prop("win/panel1", "width"), "200");
    EXPECT_EQ(prop("win/panel1", "height"), "100");
    EXPECT_EQ(prop("win/label", "no-such-property"), "");
}

TEST_F(SlintSceneTest, BoundsAreInWindowCoordinates)
{
    auto window = item("win");
    auto row = item("win/\"row 1\"");
    EXPECT_EQ(row->bounds().topLeft.x - window->bounds().topLeft.x, 60);
    EXPECT_EQ(row->bounds().topLeft.y - window->bounds().topLeft.y, 100);
    EXPECT_TRUE(row->visible());
}

TEST_F(SlintSceneTest, SetTextAndChecked)
{
    item("win/edit")->setStringProperty("text", "typed");
    EXPECT_EQ(prop("win/edit", "text"), "typed");

    item("win/check")->setStringProperty("checked", "true");
    EXPECT_EQ(prop("win/check", "checked"), "true");
    // Already checked: stays checked
    item("win/check")->setStringProperty("checked", "true");
    EXPECT_EQ(prop("win/check", "checked"), "true");
    item("win/check")->setStringProperty("checked", "false");
    EXPECT_EQ(prop("win/check", "checked"), "false");
}

TEST_F(SlintSceneTest, UnknownSetterWarns)
{
    item("win/label")->setStringProperty("whatever", "1");
    EXPECT_EQ(hooks->warnings.size(), 1u);
}

// Methods

TEST_F(SlintSceneTest, BuiltInMethods)
{
    spix::Variant ret;
    EXPECT_TRUE(item("win/check")->invokeMethod("click", {}, ret));
    EXPECT_EQ(prop("win/check", "checked"), "true");

    EXPECT_TRUE(item("win/slider")->invokeMethod("increment", {}, ret));
    EXPECT_GT(std::stod(prop("win/slider", "value")), 5);
    EXPECT_TRUE(item("win/slider")->invokeMethod("decrement", {}, ret));
    EXPECT_TRUE(item("win/slider")->invokeMethod("decrement", {}, ret));
    EXPECT_LT(std::stod(prop("win/slider", "value")), 5);

    EXPECT_FALSE(item("win/check")->invokeMethod("unknown", {}, ret));
}

// Test-helper hooks

TEST_F(SlintSceneTest, RegisteredHooks)
{
    hooks->getters["custom"] = [this](const slint::testing::ElementHandle&) -> std::optional<std::string> {
        return std::string(ui->get_custom());
    };
    hooks->setters["custom"] = [this](const slint::testing::ElementHandle&, const std::string& v) {
        ui->set_custom(slint::SharedString(v));
        return true;
    };
    hooks->methods["bump"]
        = [this](const slint::testing::ElementHandle&, const std::vector<spix::Variant>& args, spix::Variant& ret) {
              ui->set_clicks(ui->get_clicks() + (args.empty() ? 1 : static_cast<int>(std::get<long long>(args[0]))));
              ret = true;
              return true;
          };

    EXPECT_EQ(prop("win/label", "custom"), "initial");
    item("win")->setStringProperty("custom", "changed");
    EXPECT_EQ(ui->get_custom(), "changed");
    EXPECT_EQ(prop("win", "custom"), "changed");
    // usable in selectors too
    EXPECT_TRUE(item("win/(custom=changed)"));

    spix::Variant ret;
    EXPECT_TRUE(item("win/label")->invokeMethod("bump", {spix::Variant(3LL)}, ret));
    EXPECT_EQ(ui->get_clicks(), 3);
    EXPECT_TRUE(std::get<bool>(ret));
}

// Input

TEST_F(SlintSceneTest, LeftClickReachesTouchArea)
{
    click("win/panel2/ok");
    EXPECT_EQ(ui->get_clicks(), 1);
    EXPECT_EQ(ui->get_last_click(), "left");
}

TEST_F(SlintSceneTest, ClickOnTheOtherPanelsButtonDoesNothing)
{
    click("win/panel1/ok");
    EXPECT_EQ(ui->get_clicks(), 0);
}

TEST_F(SlintSceneTest, RightClickAndModifiers)
{
    click("win/panel2/ok", MouseButtons::Right);
    EXPECT_EQ(ui->get_last_click(), "right");
    EXPECT_EQ(ui->get_clicks(), 0);

    click("win/panel2/ok", MouseButtons::Left, KeyModifiers::Shift);
    EXPECT_EQ(ui->get_last_click(), "left+shift");
    click("win/panel2/ok", MouseButtons::Left, KeyModifiers::Control);
    EXPECT_EQ(ui->get_last_click(), "left+control");
    click("win/panel2/ok", MouseButtons::Left, KeyModifiers::Shift | KeyModifiers::Control);
    EXPECT_EQ(ui->get_last_click(), "left+shift+control");
    // Modifiers are released afterwards
    click("win/panel2/ok");
    EXPECT_EQ(ui->get_last_click(), "left");
}

TEST_F(SlintSceneTest, ClickLocationIsRelativeToTheItem)
{
    auto ok = item("win/panel2/ok");
    // top-left corner of the button
    scene->events().mouseDown(ok.get(), {1, 1}, MouseButtons::Left, KeyModifiers::None);
    scene->events().mouseUp(ok.get(), {1, 1}, MouseButtons::Left, KeyModifiers::None);
    EXPECT_EQ(ui->get_clicks(), 1);
    // outside of it, on the window root
    auto window = item("win");
    scene->events().mouseDown(window.get(), {5, 90}, MouseButtons::Left, KeyModifiers::None);
    scene->events().mouseUp(window.get(), {5, 90}, MouseButtons::Left, KeyModifiers::None);
    EXPECT_EQ(ui->get_clicks(), 1);
}

TEST_F(SlintSceneTest, ClickChecksTheCheckBox)
{
    click("win/check");
    EXPECT_EQ(prop("win/check", "checked"), "true");
}

TEST_F(SlintSceneTest, KeyInputIntoLineEdit)
{
    click("win/edit");
    auto edit = item("win/edit");
    scene->events().stringInput(edit.get(), "héllo");
    EXPECT_EQ(prop("win/edit", "text"), "héllo");

    scene->events().keyPress(edit.get(), KeyCodes::Backspace, KeyModifiers::None);
    scene->events().keyRelease(edit.get(), KeyCodes::Backspace, KeyModifiers::None);
    EXPECT_EQ(prop("win/edit", "text"), "héll");

    // Letters are lowercase unless shift is held
    scene->events().keyPress(edit.get(), KeyCodes::Char_A, KeyModifiers::None);
    scene->events().keyRelease(edit.get(), KeyCodes::Char_A, KeyModifiers::None);
    scene->events().keyPress(edit.get(), KeyCodes::Char_B, KeyModifiers::Shift);
    scene->events().keyRelease(edit.get(), KeyCodes::Char_B, KeyModifiers::Shift);
    EXPECT_EQ(prop("win/edit", "text"), "héllaB");

    scene->events().keyPress(edit.get(), KeyCodes::Num_7, KeyModifiers::None);
    scene->events().keyRelease(edit.get(), KeyCodes::Num_7, KeyModifiers::None);
    EXPECT_EQ(prop("win/edit", "text"), "héllaB7");
}

TEST_F(SlintSceneTest, SpecialKeys)
{
    click("win/edit");
    auto edit = item("win/edit");
    scene->events().stringInput(edit.get(), "abc");
    for (auto key : {KeyCodes::Left, KeyCodes::Left, KeyCodes::Delete}) {
        scene->events().keyPress(edit.get(), key, KeyModifiers::None);
        scene->events().keyRelease(edit.get(), key, KeyModifiers::None);
    }
    EXPECT_EQ(prop("win/edit", "text"), "ac");
}

TEST_F(SlintSceneTest, UnsupportedKeysAreIgnored)
{
    click("win/edit");
    auto edit = item("win/edit");
    scene->events().keyPress(edit.get(), KeyCodes::NumLock, KeyModifiers::None);
    scene->events().keyRelease(edit.get(), KeyCodes::NumLock, KeyModifiers::None);
    EXPECT_EQ(prop("win/edit", "text"), "");
}

TEST_F(SlintSceneTest, ExternalDropDoesNothing)
{
    auto window = item("win");
    spix::PasteboardContent content;
    scene->events().extMouseDrop(window.get(), {1, 1}, content);
}

// Screenshots

namespace {
std::string readFile(const std::string& path)
{
    std::ifstream f(path, std::ios::binary);
    return std::string(std::istreambuf_iterator<char>(f), {});
}
unsigned be32(const std::string& s, size_t offset)
{
    return (unsigned char)s[offset] << 24 | (unsigned char)s[offset + 1] << 16 | (unsigned char)s[offset + 2] << 8
        | (unsigned char)s[offset + 3];
}
} // namespace

TEST_F(SlintSceneTest, ScreenshotIsCroppedToTheItem)
{
    const auto path = (std::filesystem::temp_directory_path() / "spix_slint_screenshot.png").string();
    std::remove(path.c_str());
    scene->takeScreenshot(spix::ItemPath("win/panel1"), path);
    auto png = readFile(path);
    ASSERT_GT(png.size(), 24u);
    EXPECT_EQ(png.substr(1, 3), "PNG");
    const double scale = ui->window().scale_factor();
    EXPECT_EQ(be32(png, 16), static_cast<unsigned>(200 * scale));
    EXPECT_EQ(be32(png, 20), static_cast<unsigned>(100 * scale));
    std::remove(path.c_str());
}

TEST_F(SlintSceneTest, ScreenshotAsBase64)
{
    auto b64 = scene->takeScreenshotAsBase64(spix::ItemPath("win/label"));
    EXPECT_EQ(b64.substr(0, 8), "iVBORw0K"); // base64 of the PNG signature
    EXPECT_EQ(scene->takeScreenshotAsBase64(spix::ItemPath("win/nope")), "");
}
