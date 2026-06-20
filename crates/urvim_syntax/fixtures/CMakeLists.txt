# CMake syntax fixture
cmake_minimum_required(VERSION 3.20)
project(Urvim
  VERSION 1.0.0
  DESCRIPTION "Terminal text editor"
  LANGUAGES C CXX
)

set(NAME "urvim")
set(VERSION_MAJOR 1)
set(VERSION_MINOR 0)
set(COPYRIGHT "Copyright 2024")
message(STATUS "hello ${NAME}")
message(STATUS "Version: ${VERSION_MAJOR}.${VERSION_MINOR}")

set(BLOCK [[one
two]])

option(BUILD_TESTING "Build tests" ON)
option(ENABLE_DEBUG "Enable debug" OFF)

if(BUILD_TESTING)
  enable_testing()
  add_subdirectory(tests)
endif()

foreach(LANG C CXX)
  string(TOLOWER "${LANG}" lang_lower)
  message(STATUS "Language: ${lang_lower}")
endforeach()

function(urvim_add_executable NAME)
  add_executable(${NAME} ${ARGN})
  target_compile_features(${NAME} PRIVATE cxx_std_20)
  set_target_properties(${NAME} PROPERTIES
    RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}/bin"
  )
  install(TARGETS ${NAME} RUNTIME DESTINATION bin)
endfunction()

urvim_add_executable(urvim src/main.cpp src/editor.cpp)

set(SOURCES
  src/buffer.cpp
  src/syntax.cpp
  src/render.cpp
  PARENT_SCOPE
)

add_library(urvim_core ${SOURCES})
target_link_libraries(urvim_core PUBLIC fmt::fmt)
target_include_directories(urvim_core PUBLIC include)
