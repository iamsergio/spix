#include <gtest/gtest.h>

#include <cstdlib>

int main(int argc, char** argv)
{
    // No display needed. Needs Slint's "mcp" feature, which the dev-slint preset turns on.
    setenv("SLINT_BACKEND", "headless", 0);
    ::testing::InitGoogleTest(&argc, argv);
    return RUN_ALL_TESTS();
}
