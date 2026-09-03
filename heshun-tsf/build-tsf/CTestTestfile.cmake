# CMake generated Testfile for 
# Source directory: D:/Proj/Heshun-ime/heshun-tsf
# Build directory: D:/Proj/Heshun-ime/heshun-tsf/build-tsf
# 
# This file includes the relevant testing commands required for 
# testing this directory and lists subdirectories to be tested as well.
add_test("tsf_binary_exports" "powershell" "-NoProfile" "-ExecutionPolicy" "Bypass" "-File" "D:/Proj/Heshun-ime/heshun-tsf/tests/verify_exports.ps1" "D:/Proj/Heshun-ime/heshun-tsf/build-tsf/bin/heshun_tsf.dll")
set_tests_properties("tsf_binary_exports" PROPERTIES  _BACKTRACE_TRIPLES "D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;100;add_test;D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;0;")
add_test("tsf_abi_header_contract" "D:/Proj/Heshun-ime/heshun-tsf/build-tsf/bin/heshun_tsf_abi_header_contract.exe")
set_tests_properties("tsf_abi_header_contract" PROPERTIES  _BACKTRACE_TRIPLES "D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;104;add_test;D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;0;")
add_test("tsf_key_event_contract" "D:/Proj/Heshun-ime/heshun-tsf/build-tsf/bin/heshun_tsf_key_event_contract.exe")
set_tests_properties("tsf_key_event_contract" PROPERTIES  _BACKTRACE_TRIPLES "D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;107;add_test;D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;0;")
add_test("tsf_tsf_interface_contract" "D:/Proj/Heshun-ime/heshun-tsf/build-tsf/bin/heshun_tsf_tsf_interface_contract.exe")
set_tests_properties("tsf_tsf_interface_contract" PROPERTIES  _BACKTRACE_TRIPLES "D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;111;add_test;D:/Proj/Heshun-ime/heshun-tsf/CMakeLists.txt;0;")
