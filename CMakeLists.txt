cmake_minimum_required(VERSION 3.18)
project(Spix LANGUAGES CXX)

option(SPIX_BUILD_EXAMPLES "Build Spix examples." ON)
option(SPIX_BUILD_TESTS "Build Spix unit tests." OFF)
option(SPIX_BUILD_QTQUICK "Build the QtQuick scene library." ON)
option(SPIX_BUILD_SLINT "Build the Slint scene library (builds Slint from 3rdparty/slint with Rust/cargo)." OFF)
option(SPIX_BUILD_QTWIDGETS "Build the QtWidgets scene library." OFF)
set(SPIX_QT_MAJOR "6" CACHE STRING "Major Qt version to build Spix against")

set(CMAKE_MODULE_PATH ${CMAKE_MODULE_PATH} "${CMAKE_CURRENT_LIST_DIR}/cmake/modules")
set(CMAKE_CXX_STANDARD 17)

# Hide symbols unless explicitly flagged with SPIX_EXPORT
set(CMAKE_CXX_VISIBILITY_PRESET hidden)
set(CMAKE_VISIBILITY_INLINES_HIDDEN 1)

if(SPIX_BUILD_SLINT)
    # slint-testing.h needs the experimental + testing features, so Slint is built from source.
    set(SLINT_FEATURE_EXPERIMENTAL ON CACHE BOOL "Required by Spix::Slint" FORCE)
    set(SLINT_FEATURE_TESTING ON CACHE BOOL "Required by Spix::Slint" FORCE)
    # ElementHandle needs debug info in the compiled .slint code. slint_target_sources() only
    # emits it when SYSTEM_TESTING is on (inert unless SLINT_TEST_SERVER is set at runtime).
    set(SLINT_FEATURE_SYSTEM_TESTING ON CACHE BOOL "Required by Spix::Slint" FORCE)
    # Not needed by Spix, skipping them shortens the Slint build.
    set(SLINT_FEATURE_INTERPRETER OFF CACHE BOOL "Not needed by Spix")
    set(SLINT_FEATURE_SYSTEM_TRAY OFF CACHE BOOL "Not needed by Spix")
    set(SLINT_FEATURE_GETTEXT OFF CACHE BOOL "Not needed by Spix")
    # Slint is linked as a shared library (the upstream default). The static variant needs
    # fontconfig & co. listed by hand. Don't let that leak into Spix's own libraries though,
    # as shared Spix libs break the unit tests (hidden symbols), so restore it afterwards.
    set(_spix_shared_libs OFF)
    if(BUILD_SHARED_LIBS)
        set(_spix_shared_libs ON)
    endif()
    set(BUILD_SHARED_LIBS ON)
    add_subdirectory(3rdparty/slint/api/cpp EXCLUDE_FROM_ALL)
    set(BUILD_SHARED_LIBS ${_spix_shared_libs})
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
