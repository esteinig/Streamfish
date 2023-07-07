cmake_minimum_required(VERSION 3.20)

project(test)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_EXTENSIONS OFF)

add_executable(test basecall.cpp)

set(BOOST_DIR build/boost_1_82_0)
target_include_directories(test PRIVATE ${BOOST_DIR})
