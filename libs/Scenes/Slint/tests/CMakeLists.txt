#
# Spix Slint Tests
#
find_package(GTest REQUIRED)

add_executable(SpixSlintTests
    unittests_main.cpp
    SlintScene_test.cpp
    SlintTestFixture.h
)
slint_target_sources(SpixSlintTests test_window.slint)
target_link_libraries(SpixSlintTests
    PRIVATE
        Spix::Slint
        Slint::Slint
        GTest::gtest
)
target_include_directories(SpixSlintTests
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}
        ${CMAKE_CURRENT_SOURCE_DIR}/../src
        ${STB_INCLUDE_DIR}
)

add_test(NAME SpixSlintTests COMMAND SpixSlintTests)
set_tests_properties(SpixSlintTests PROPERTIES TIMEOUT 120)
