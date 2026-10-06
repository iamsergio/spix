// Phase 0 placeholder: proves that slint-testing.h is available and links.
// Replaced by the real backend in Phase 1.

#include <Spix/SlintBuildCheck.h>

#include <slint-testing.h>

#include <optional>

namespace spix::slint_detail {

bool hasElementHandleOnNothing()
{
    std::optional<slint::testing::ElementHandle> handle;
    return handle.has_value();
}

} // namespace spix::slint_detail
