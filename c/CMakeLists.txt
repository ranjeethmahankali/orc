cmake_minimum_required(VERSION 3.15)
project(orc C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)
# Output binaries directly into CMAKE_BINARY_DIR.
# Use -B build/debug or -B build/release to control the output location.
# The per-config variables override the multi-config generator behaviour
# (MSVC/VS) that would otherwise append a Debug/ or Release/ subdirectory.
set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
foreach(config DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY_${config} ${CMAKE_BINARY_DIR})
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY_${config} ${CMAKE_BINARY_DIR})
    set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY_${config} ${CMAKE_BINARY_DIR})
endforeach()

# Compiler warnings
if(MSVC)
    add_compile_options(/W4 /WX /wd4130 /wd4127 /wd4310 /Zc:preprocessor)
else()
    add_compile_options(
        -Wall -Wextra -Werror -Wconversion -Wsign-conversion
        -Wcast-align -Wpointer-arith -Wshadow -pedantic
    )
endif()

# unity — static library
add_library(unity STATIC third_party/unity.c)
target_include_directories(unity PUBLIC third_party)
target_compile_definitions(unity PUBLIC UNITY_INCLUDE_DOUBLE)

# orc_sdk_obj — object library
add_library(orc_sdk_obj OBJECT orc_sdk/orc_sdk.c)
target_include_directories(orc_sdk_obj PUBLIC .)
set_target_properties(orc_sdk_obj PROPERTIES POSITION_INDEPENDENT_CODE ON)

# example_c_plugin — shared library
add_library(example_c_plugin SHARED
    example_c_plugin/plugin.c
    example_c_plugin/functions.c
    $<TARGET_OBJECTS:orc_sdk_obj>
)
target_include_directories(example_c_plugin PRIVATE .)

# orc_sdk_tests — executable
add_executable(orc_sdk_tests
    orc_sdk/test.c
    $<TARGET_OBJECTS:orc_sdk_obj>
)
target_include_directories(orc_sdk_tests PRIVATE .)
target_link_libraries(orc_sdk_tests PRIVATE unity)
if(NOT MSVC)
    target_link_libraries(orc_sdk_tests PRIVATE m)
endif()
if(NOT WIN32)
    target_link_libraries(orc_sdk_tests PRIVATE pthread)
    target_link_libraries(example_c_plugin PRIVATE pthread)
endif()

# example_c_plugin_tests — executable
add_executable(example_c_plugin_tests
    example_c_plugin/test.c
    example_c_plugin/plugin.c
    example_c_plugin/functions.c
    $<TARGET_OBJECTS:orc_sdk_obj>
)
target_include_directories(example_c_plugin_tests PRIVATE .)
target_link_libraries(example_c_plugin_tests PRIVATE unity)
if(NOT MSVC)
    target_link_libraries(example_c_plugin_tests PRIVATE m)
endif()
if(NOT WIN32)
    target_link_libraries(example_c_plugin_tests PRIVATE pthread)
endif()

# cli_client — executable
add_executable(cli_client
    cli_client/main.c
    $<TARGET_OBJECTS:orc_sdk_obj>
)
target_include_directories(cli_client PRIVATE .)
if(MSVC)
    target_compile_definitions(cli_client PRIVATE _CRT_SECURE_NO_WARNINGS)
endif()
if(WIN32)
    target_link_libraries(cli_client PRIVATE ws2_32)
endif()
