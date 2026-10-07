add_executable(SpixSlintBasicExample main.cpp)
slint_target_sources(SpixSlintBasicExample main.slint)
target_include_directories(SpixSlintBasicExample PRIVATE ${CMAKE_CURRENT_SOURCE_DIR}/../../shared)
target_link_libraries(SpixSlintBasicExample PRIVATE Spix::Slint Slint::Slint)

if(SPIX_BUILD_TESTS)
    add_test(NAME SpixSlintBasicExample COMMAND SpixSlintBasicExample)
    # headless needs Slint's MCP feature (set by scripts/build_slint.sh), and works without a display
    set_tests_properties(SpixSlintBasicExample PROPERTIES ENVIRONMENT "SLINT_BACKEND=headless" TIMEOUT 60)
endif()
