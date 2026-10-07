cmake_minimum_required(VERSION 3.18)
project(Spix LANGUAGES CXX)

option(SPIX_BUILD_EXAMPLES "Build Spix examples." ON)
option(SPIX_BUILD_TESTS "Build Spix unit tests." OFF)
option(SPIX_BUILD_QTQUICK "Build the QtQuick scene library." ON)
option(SPIX_BUILD_SLINT "Build the Slint scene library (needs Slint built with the experimental and testing features)." OFF)
option(SPIX_BUILD_QTWIDGETS "Build the QtWidgets scene library." OFF)
set(SPIX_QT_MAJOR "6" CACHE STRING "Major Qt version to build Spix against")

set(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} "${CMAKE_CURRENT_LIST_DIR}/cmake/modules")
set(CMAKE_CXX_STANDARD 17)

# Hide symbols unless explicitly flagged with SPIX_EXPORT
set(CMAKE_CXX_VISIBILITY_PRESET hidden)
set(CMAKE_VISIBILITY_INLINES_HIDDEN 1)

if(SPIX_BUILD_SLINT)
    # Use the Slint of a parent project (add_subdirectory()/FetchContent), or an installed one,
    # see scripts/build_slint.sh. Prebuilt Slint packages lack the features below, so build it from source.
    if(NOT TARGET Slint::Slint)
        find_package(Slint REQUIRED)
    endif()
    # slint-testing.h needs EXPERIMENTAL and TESTING. ElementHandle needs debug info in the compiled
    # .slint code, which slint_target_sources() only emits with SYSTEM_TESTING (inert unless
    # SLINT_TEST_SERVER is set at runtime).
    get_target_property(_spix_slint_features Slint::Slint SLINT_ENABLED_FEATURES)
    foreach(_feature EXPERIMENTAL TESTING SYSTEM_TESTING)
        if(NOT "${_feature}" IN_LIST _spix_slint_features)
            message(FATAL_ERROR "Spix::Slint needs Slint built with SLINT_FEATURE_${_feature}=ON")
        endif()
    endforeach()
    # The Slint tests and examples run with SLINT_BACKEND=headless, which needs MCP.
    if((SPIX_BUILD_TESTS OR SPIX_BUILD_EXAMPLES) AND NOT "MCP" IN_LIST _spix_slint_features)
        message(WARNING "Slint was built without SLINT_FEATURE_MCP, the Slint tests and examples need a display")
    endif()
endif()

add_subdirectory(libs)

# before the examples, some of them register ctest tests
if(SPIX_BUILD_TESTS)
    enable_testing()
endif()

if(SPIX_BUILD_EXAMPLES)
    add_subdirectory(examples)
endif()

if(SPIX_BUILD_TESTS)
    add_subdirectory(libs/Core/tests)
    if(SPIX_BUILD_QTQUICK)
        add_subdirectory(libs/Scenes/QtQuick/tests)
    endif()
    if(SPIX_BUILD_QTWIDGETS)
        add_subdirectory(libs/Scenes/QtWidgets/tests)
    endif()
    if(SPIX_BUILD_SLINT)
        add_subdirectory(libs/Scenes/Slint/tests)
    endif()
endif()

#
# Install main Spix config
#
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/SpixConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/SpixConfig.cmake"
    INSTALL_DESTINATION "${CMAKE_INSTALL_DATADIR}/Spix/cmake"
)

install(
    FILES "${CMAKE_CURRENT_BINARY_DIR}/SpixConfig.cmake"
    DESTINATION "${CMAKE_INSTALL_DATADIR}/Spix/cmake"
)
