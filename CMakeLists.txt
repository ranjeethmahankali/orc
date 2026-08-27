cmake_minimum_required(VERSION 3.15)
cmake_policy(SET CMP0091 NEW)

# Static CRT on MSVC so executables have no runtime DLL dependencies.
if(MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif()

project(orc C CXX)

add_subdirectory(c)
add_subdirectory(cpp)
