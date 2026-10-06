#
# Spix Slint Library
#
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

#
# Dependencies
#
message(STATUS "Building Spix Slint")

#
# Sources
#
file(GLOB HEADERS_PUB_ROOT "include/Spix/*.h")
set(HEADERS_PUB ${HEADERS_PUB_ROOT})

set(SOURCES
    ${HEADERS_PUB}
    src/SlintBuildCheck.cpp
)

source_group(TREE ${CMAKE_CURRENT_SOURCE_DIR} PREFIX source FILES ${SOURCES})

#
# Target
#
add_library(SpixSlint ${SOURCES})
target_include_directories(SpixSlint
    PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
        $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}>
        $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>
    PRIVATE
        src
)
target_link_libraries(SpixSlint
    PUBLIC
        Spix::Core
        Slint::Slint
)

#
# Export headers
#
include(GenerateExportHeader)
generate_export_header(SpixSlint
    EXPORT_FILE_NAME "Spix/spix_slint_export.h"
)

#
# Install
#
install(
    TARGETS SpixSlint
    EXPORT SpixSlintTargets
    RUNTIME DESTINATION ${CMAKE_INSTALL_BINDIR}
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
)
install(DIRECTORY include/Spix DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/Spix/spix_slint_export.h DESTINATION ${CMAKE_INSTALL_INCLUDEDIR}/Spix)

set(SPIX_QTQUICK_CMAKE_CONFIG_DIR "${CMAKE_INSTALL_DATADIR}/SpixSlint/cmake")

configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/SpixSlintConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/SpixSlintConfig.cmake"
    INSTALL_DESTINATION "${SPIX_QTQUICK_CMAKE_CONFIG_DIR}"
)

install(
    EXPORT SpixSlintTargets
    FILE SpixSlintTargets.cmake
    DESTINATION ${SPIX_QTQUICK_CMAKE_CONFIG_DIR}
    NAMESPACE Spix::
)
install(
    FILES "${CMAKE_CURRENT_BINARY_DIR}/SpixSlintConfig.cmake"
    DESTINATION ${SPIX_QTQUICK_CMAKE_CONFIG_DIR}
)

export(
    EXPORT SpixSlintTargets
    FILE "${CMAKE_CURRENT_BINARY_DIR}/cmake/SpixSlintTargets.cmake"
    NAMESPACE Spix::
)

# Alias for consistency
add_library(Spix::Slint ALIAS SpixSlint)
