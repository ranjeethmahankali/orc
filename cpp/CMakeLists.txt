cmake_minimum_required(VERSION 3.15)
project(orc_cpp CXX)

set(CMAKE_CXX_STANDARD 23)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

set(CMAKE_RUNTIME_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
set(CMAKE_LIBRARY_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY ${CMAKE_BINARY_DIR})
foreach(config DEBUG RELEASE RELWITHDEBINFO MINSIZEREL)
    set(CMAKE_RUNTIME_OUTPUT_DIRECTORY_${config} ${CMAKE_BINARY_DIR})
    set(CMAKE_LIBRARY_OUTPUT_DIRECTORY_${config} ${CMAKE_BINARY_DIR})
    set(CMAKE_ARCHIVE_OUTPUT_DIRECTORY_${config} ${CMAKE_BINARY_DIR})
endforeach()

if(MSVC)
    add_compile_options(/W4 /WX /Zc:preprocessor)
else()
    add_compile_options(
        -Wall -Wextra -Werror -Wconversion -Wsign-conversion
        -Wcast-align -Wpointer-arith -Wshadow -pedantic
    )
endif()

# orc_sdk — static library
add_library(orc_sdk_cpp STATIC
    orc_sdk/error.cpp
    orc_sdk/deck.cpp
)
target_include_directories(orc_sdk_cpp PUBLIC . ../c)

# orc_sdk_cpp_tests — executable
add_executable(orc_sdk_cpp_tests
    orc_sdk/test.cpp
    ../c/third_party/unity.c
)
target_compile_definitions(orc_sdk_cpp_tests PRIVATE UNITY_INCLUDE_DOUBLE)
target_link_libraries(orc_sdk_cpp_tests PRIVATE orc_sdk_cpp)
