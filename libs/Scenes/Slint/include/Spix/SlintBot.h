/***
 * Copyright (C) Falko Axmann. All rights reserved.
 * Licensed under the MIT license.
 * See LICENSE.txt file in the project root for full license information.
 ****/

#pragma once

#include <Spix/Data/Variant.h>
#include <Spix/TestServer.h>

#include <slint-testing.h>
#include <slint.h>

#include <functional>
#include <memory>
#include <optional>
#include <string>
#include <vector>

#include <Spix/spix_slint_export.h>

namespace spix {

class CommandExecuter;
class SlintScene;

/**
 * Test-helper hooks. Compiled Slint doesn't allow reading or writing arbitrary properties by name, so
 * tests that need a property or method that isn't available through the accessibility API register
 * a handler here. Each handler receives the element the command was addressed to. For a window path
 * (only the window name) that is the first element of the window, as Slint's element search doesn't
 * visit the window's own element: such handlers should rely on what they captured, not on the element.
 *
 * Handlers run on the main thread.
 */
using SlintPropertyGetter = std::function<std::optional<std::string>(const slint::testing::ElementHandle&)>;
using SlintPropertySetter = std::function<bool(const slint::testing::ElementHandle&, const std::string& value)>;
using SlintMethodHandler
    = std::function<bool(const slint::testing::ElementHandle&, const std::vector<Variant>& args, Variant& ret)>;

/**
 * @brief Class that maintains and runs the test environment for Slint applications
 *
 * Create one in `main()` after creating your windows, register each window with `addWindow()`
 * under the name that test paths use as their first component, then call `runTestServer()`.
 *
 * Elements are addressed by their `accessible-id` (which requires a constant `accessible-role`).
 *
 * The bot uses a `slint::Timer` to process commands on the main thread, so it needs a running
 * Slint event loop with real time. Don't call `slint::testing::init()`.
 */
class SPIXSLINT_EXPORT SlintBot {
public:
    SlintBot();
    ~SlintBot();

    SlintBot(const SlintBot&) = delete;
    SlintBot& operator=(const SlintBot&) = delete;

    /// Registers a window under @p name. The bot keeps a reference to the component until removeWindow().
    template <typename T>
    void addWindow(std::string name, const slint::ComponentHandle<T>& component)
    {
        addWindowImpl(std::move(name), &component->window(), [component](const ElementVisitor& visitor) {
            slint::testing::ElementHandle::visit_elements(
                component, [&visitor](slint::testing::ElementHandle element) { return visitor(std::move(element)); });
        });
    }
    void removeWindow(const std::string& name);

    void runTestServer(TestServer& server);

    // Test-helper hooks, see above. Registering the same name again replaces the handler.
    void registerPropertyGetter(std::string name, SlintPropertyGetter getter);
    void registerPropertySetter(std::string name, SlintPropertySetter setter);
    void registerMethod(std::string name, SlintMethodHandler handler);

    /// Messages for problems the test commands can't report themselves (e.g. unsupported selectors).
    std::vector<std::string> warnings() const;

private:
    /// Return true to stop the visit.
    using ElementVisitor = std::function<bool(slint::testing::ElementHandle)>;
    using ElementVisit = std::function<void(const ElementVisitor&)>;

    void addWindowImpl(std::string name, slint::Window* window, ElementVisit visit);

    struct Private;
    std::unique_ptr<Private> d;
};

} // namespace spix
