cmake_minimum_required(VERSION 3.21)
project(ShanjieLinuxBasicTests LANGUAGES NONE)
enable_testing()

find_program(CARGO_EXECUTABLE NAMES cargo REQUIRED)
find_program(BASH_EXECUTABLE NAMES bash REQUIRED)
get_filename_component(SHANJIE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../.." ABSOLUTE)
set(SHANJIE_LINUX_PAYLOAD "" CACHE PATH "Built Linux package tree to verify before archiving")
if(NOT SHANJIE_LINUX_PAYLOAD)
  message(FATAL_ERROR "Pass the built package tree with -DSHANJIE_LINUX_PAYLOAD=...")
endif()
set(target_dir "${SHANJIE_ROOT}/build/linux/target")

add_test(NAME linux.rust.shared COMMAND "${CARGO_EXECUTABLE}" test --release --locked
  --manifest-path "${SHANJIE_ROOT}/shared/Cargo.toml" --target-dir "${target_dir}" --lib -- --test-threads=1)
add_test(NAME linux.rust.common COMMAND "${CARGO_EXECUTABLE}" test --release --locked
  --manifest-path "${SHANJIE_ROOT}/linux/common/Cargo.toml" --target-dir "${target_dir}" --lib -- --test-threads=1)
add_test(NAME linux.rust.ibus COMMAND "${CARGO_EXECUTABLE}" test --release --locked
  --manifest-path "${SHANJIE_ROOT}/linux/ibus/Cargo.toml" --target-dir "${target_dir}" --bin ibus-engine-shanjie -- --test-threads=1)
add_test(NAME linux.rust.settings COMMAND "${CARGO_EXECUTABLE}" test --release --locked
  --manifest-path "${SHANJIE_ROOT}/linux/settings/Cargo.toml" --target-dir "${target_dir}" --bin shanjie-settings -- --test-threads=1)
set_tests_properties(linux.rust.shared linux.rust.common linux.rust.ibus linux.rust.settings
  PROPERTIES LABELS "linux;basic;rust" TIMEOUT 600 WORKING_DIRECTORY "${SHANJIE_ROOT}")

file(GLOB scenarios "${CMAKE_CURRENT_SOURCE_DIR}/scenarios/*.json")
foreach(scenario IN LISTS scenarios)
  get_filename_component(name "${scenario}" NAME_WE)
  add_test(NAME "linux.ibus.${name}" COMMAND "${BASH_EXECUTABLE}"
    "${CMAKE_CURRENT_SOURCE_DIR}/ibus-harness.sh"
    "${SHANJIE_LINUX_PAYLOAD}/files/libexec/ibus-engine-shanjie" "${scenario}")
  set_tests_properties("linux.ibus.${name}" PROPERTIES LABELS "linux;basic;ibus" TIMEOUT 120
    WORKING_DIRECTORY "${SHANJIE_ROOT}"
    ENVIRONMENT "SHANJIE_DATA=${SHANJIE_LINUX_PAYLOAD}/files/share/shanjie/data")
endforeach()
