# Linux menu adaptation

`menu.cc` derives from the MIT-licensed `system_tray` 2.0.3 package by AnTler.
The upstream file has SHA256
`8f682c66fdaf2de8726d1b72a6c3252c78ec53dc5fb8bd60749ab404c34b6337`.
The accompanying upstream headers and remaining implementation come from the
locked package. Linux CMake replaces only this translation unit and refuses
unreviewed upstream source changes. The shared package cache is not modified.

Upstream's runtime label, sensitivity and checked-state setters are empty.
The adaptation updates existing GTK menu items, validates boolean values and
suppresses user-selection callbacks while applying a checked-state snapshot.
It preserves the existing menu and its native IDs, including nested items.
Runtime menu image replacement is unchanged and is not used by ShuVPN.

The complete upstream MIT license is retained here and installed in the bundle.
Real StatusNotifier/dbusmenu verification is provided by
`tool/desktop_tray_acceptance.py`; Flutter's mocked platform channel alone
cannot verify GTK state updates.
