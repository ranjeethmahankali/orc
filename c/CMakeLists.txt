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

# deck_ops_plugin — shared library
add_library(deck_ops_plugin SHARED
    deck_ops_plugin/plugin.c
    deck_ops_plugin/functions.c
    $<TARGET_OBJECTS:orc_sdk_obj>
)
target_include_directories(deck_ops_plugin PRIVATE .)

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
    target_link_libraries(deck_ops_plugin PRIVATE pthread)
endif()

# deck_ops_plugin_tests — executable
add_executable(deck_ops_plugin_tests
    deck_ops_plugin/test.c
    deck_ops_plugin/functions.c
    $<TARGET_OBJECTS:orc_sdk_obj>
)
target_include_directories(deck_ops_plugin_tests PRIVATE .)
target_link_libraries(deck_ops_plugin_tests PRIVATE unity)
if(NOT MSVC)
    target_link_libraries(deck_ops_plugin_tests PRIVATE m)
endif()
if(NOT WIN32)
    target_link_libraries(deck_ops_plugin_tests PRIVATE pthread)
endif()
