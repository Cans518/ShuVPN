# Windows menu adaptation

`menu.cpp` derives from the MIT-licensed `system_tray` 2.0.3 package by AnTler.
The upstream file has SHA256
`6cf1efb4b7371f372eb278f9387b8dc9ad62776b134fcd275cbcfced0ed2c7cd`.
The locked package provides its original headers and remaining implementation.
Windows CMake replaces only this translation unit and rejects an upstream source
change until it is reviewed. The shared package cache is not modified.

Upstream checkbox creation overwrites the enabled state with the checked state.
Runtime enable/check updates also replace each other's state bits. The adaptation
combines initial states and reads the native item before changing only the
requested bits, preserving the same HMENU, item IDs and callback behavior.
It supports command IDs in submenus through the existing Win32 lookup.
Other setters, argument validation and resource ownership remain upstream.

`test/native/windows_tray_menu_test.cpp` calls the plugin's public method handlers
against real Win32 menus without displaying a window or accessing user settings.
The test target is excluded from normal builds and is not installed.
These tests do not establish real tray pointer interaction or connected GUI
acceptance. The full upstream MIT license is installed in the bundle.
