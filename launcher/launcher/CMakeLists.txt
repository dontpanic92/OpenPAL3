# CMakeList.txt : CMake project for launcher, include source and define
# project specific logic here.
#
cmake_minimum_required (VERSION 3.8)

# Add source to this project's executable.
add_executable (launcher WIN32 "Main.cpp" "Dialogs.cpp" "Dialogs.h" "OpenPal3Config.hpp" "wxSR.hpp")

set(VCPKG_TARGET_TRIPLET "x64-windows-static")

if("${CMAKE_CXX_COMPILER_ID}" MATCHES "MSVC")
	set(CMAKE_CXX_FLAGS_RELEASE "${CMAKE_CXX_FLAGS_RELEASE} /MT /utf-8")
	set(CMAKE_CXX_FLAGS_DEBUG "${CMAKE_CXX_FLAGS_DEBUG} /MTd /utf-8")
endif()

if("${CMAKE_BUILD_TYPE}" MATCHES "Debug")
	add_definitions(-D_DEBUG)
	include_directories(${_VCPKG_INSTALLED_DIR}/${VCPKG_TARGET_TRIPLET}/include)
	set(wxWidgets_LIB_DIR ${_VCPKG_INSTALLED_DIR}/${VCPKG_TARGET_TRIPLET}/debug/lib)
	set(wxWidgets_LIBRARIES
		${wxWidgets_LIB_DIR}/wxmsw31ud_core.lib
		${wxWidgets_LIB_DIR}/wxbase31ud.lib
		${wxWidgets_LIB_DIR}/libpng16d.lib
		${wxWidgets_LIB_DIR}/zlibd.lib
		comctl32
		Rpcrt4
		)
elseif("${CMAKE_BUILD_TYPE}" MATCHES "Release")
	include_directories(${_VCPKG_INSTALLED_DIR}/${VCPKG_TARGET_TRIPLET}/include)
	set(wxWidgets_LIB_DIR ${_VCPKG_INSTALLED_DIR}/${VCPKG_TARGET_TRIPLET}/lib)
	set(wxWidgets_LIBRARIES
		${wxWidgets_LIB_DIR}/wxmsw31u_core.lib
		${wxWidgets_LIB_DIR}/wxbase31u.lib
		${wxWidgets_LIB_DIR}/libpng16.lib
		${wxWidgets_LIB_DIR}/zlib.lib
		comctl32
		Rpcrt4
		)
endif()

target_link_libraries(${PROJECT_NAME} ${wxWidgets_LIBRARIES})
