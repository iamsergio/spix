find_package(GTest REQUIRED)

add_executable(SpixSlintGTestExample main.cpp)
# Same UI as the Basic example
slint_target_sources(SpixSlintGTestExample ../Basic/main.slint)
target_link_libraries(SpixSlintGTestExample PRIVATE GTest::GTest Spix::Slint Slint::Slint)

if(SPIX_BUILD_TESTS)
    add_test(NAME SpixSlintGTestExample COMMAND SpixSlintGTestExample)
    set_tests_properties(SpixSlintGTestExample PROPERTIES ENVIRONMENT "SLINT_BACKEND=headless" TIMEOUT 60)
endif()
