#include <flutter/method_result_functions.h>

#include <cstdio>
#include <memory>
#include <string>

#include "menu.h"

namespace {

using flutter::EncodableList;
using flutter::EncodableMap;
using flutter::EncodableValue;

bool Expect(bool condition, const char* message) {
  if (!condition) {
    std::fprintf(stderr, "FAIL: %s\n", message);
  }
  return condition;
}

EncodableMap Checkbox(int id, bool enabled, bool checked) {
  return {{EncodableValue("id"), EncodableValue(id)},
          {EncodableValue("type"), EncodableValue("checkbox")},
          {EncodableValue("label"), EncodableValue("aTrust")},
          {EncodableValue("enabled"), EncodableValue(enabled)},
          {EncodableValue("checked"), EncodableValue(checked)}};
}

bool Create(Menu& menu, bool enabled, bool checked) {
  EncodableMap group{
      {EncodableValue("id"), EncodableValue(10)},
      {EncodableValue("type"), EncodableValue("submenu")},
      {EncodableValue("label"), EncodableValue("Protocol")},
      {EncodableValue("submenu"),
       EncodableValue(EncodableList{EncodableValue(Checkbox(21, enabled, checked))})}};
  auto args = std::make_unique<EncodableValue>(EncodableMap{
      {EncodableValue("menu_list"),
       EncodableValue(EncodableList{EncodableValue(Checkbox(1, enabled, checked)),
                                   EncodableValue(group)})}});
  return Expect(menu.CreateContextMenu(flutter::MethodCall<>("create", std::move(args))),
                "create actual HMENU");
}

bool State(Menu& menu, int id, UINT expected) {
  MENUITEMINFO state{};
  state.cbSize = sizeof(state);
  state.fMask = MIIM_STATE;
  if (!Expect(GetMenuItemInfo(menu.GetMenu(), id, FALSE, &state) != FALSE,
              "read native item by command ID including submenu")) {
    return false;
  }
  if (state.fState != expected) {
    std::fprintf(stderr, "id=%d native state=0x%x expected=0x%x\n", id,
                 state.fState, expected);
    return false;
  }
  return true;
}

using Setter = void (Menu::*)(const flutter::MethodCall<>&,
                             flutter::MethodResult<>&);

bool Update(Menu& menu, Setter setter, int id, const char* key,
            EncodableValue value) {
  auto args = std::make_unique<EncodableValue>(EncodableMap{
      {EncodableValue("menu_item_id"), EncodableValue(id)},
      {EncodableValue(key), std::move(value)}});
  bool success = false;
  flutter::MethodResultFunctions<> result(
      [&success](const EncodableValue* response) {
        success = response && std::get_if<bool>(response) &&
                  std::get<bool>(*response);
      },
      [](const std::string& code, const std::string&, const EncodableValue*) {
        std::fprintf(stderr, "method error: %s\n", code.c_str());
      },
      [] { std::fprintf(stderr, "method not implemented\n"); });
  (menu.*setter)(flutter::MethodCall<>("update", std::move(args)), result);
  return Expect(success, "native method reports success");
}

bool InitialState() {
  bool passed = true;
  for (bool enabled : {false, true}) {
    for (bool checked : {false, true}) {
      Menu menu({}, 100);
      if (!Create(menu, enabled, checked)) {
        return false;
      }
      const UINT expected = (enabled ? MFS_ENABLED : MFS_DISABLED) |
                            (checked ? MFS_CHECKED : MFS_UNCHECKED);
      passed = State(menu, 1, expected) && passed;
      passed = State(menu, 21, expected) && passed;
    }
  }
  return passed;
}

bool EnablePreservesCheck() {
  Menu menu({}, 100);
  if (!Create(menu, true, true)) {
    return false;
  }
  bool passed = true;
  for (int id : {1, 21}) {
    for (bool enabled : {false, true, false}) {
      passed = Update(menu, &Menu::SetEnable, id, "enabled",
                      EncodableValue(enabled)) && passed;
      passed = State(menu, id, MFS_CHECKED | (enabled ? MFS_ENABLED : MFS_DISABLED)) &&
               passed;
    }
  }
  return passed;
}

bool CheckPreservesDisabled() {
  Menu menu({}, 100);
  if (!Create(menu, true, false)) {
    return false;
  }
  bool passed = true;
  for (int id : {1, 21}) {
    // Seed independent native bits so this also detects a full-state overwrite.
    MENUITEMINFO seed{};
    seed.cbSize = sizeof(seed);
    seed.fMask = MIIM_STATE;
    seed.fState = MFS_DISABLED | MFS_DEFAULT;
    passed = Expect(SetMenuItemInfo(menu.GetMenu(), id, FALSE, &seed) != FALSE,
                    "seed disabled/default native state") && passed;
    for (bool checked : {true, false, true}) {
      passed = Update(menu, &Menu::SetCheck, id, "checked",
                      EncodableValue(checked)) && passed;
      passed = State(menu, id, MFS_DISABLED | MFS_DEFAULT |
                                  (checked ? MFS_CHECKED : MFS_UNCHECKED)) &&
               passed;
    }
    passed = Update(menu, &Menu::SetEnable, id, "enabled", EncodableValue(true)) &&
             passed;
    passed = State(menu, id, MFS_DEFAULT | MFS_CHECKED) && passed;
  }
  return passed;
}

bool LabelPreservesIdentity() {
  Menu menu({}, 100);
  if (!Create(menu, true, true)) {
    return false;
  }
  const HMENU original = menu.GetMenu();
  const HMENU submenu = GetSubMenu(original, 1);
  bool passed = true;
  for (int id : {1, 21}) {
    passed = Update(menu, &Menu::SetLabel, id, "label", EncodableValue("Selected")) &&
             passed;
    wchar_t label[32]{};
    MENUITEMINFO item{};
    item.cbSize = sizeof(item);
    item.fMask = MIIM_STRING | MIIM_ID | MIIM_DATA | MIIM_STATE;
    item.dwTypeData = label;
    item.cch = 32;
    passed = Expect(GetMenuItemInfo(original, id, FALSE, &item) != FALSE &&
                        std::wstring(label) == L"Selected" &&
                        item.wID == static_cast<UINT>(id) &&
                        item.dwItemData == 100 && item.fState == MFS_CHECKED,
                    "label keeps ID, owner and checked state") && passed;
  }
  return Expect(menu.GetMenu() == original && GetSubMenu(original, 1) == submenu,
                "native menu and submenu handles remain unchanged") && passed;
}

}  // namespace

int main() {
  int failures = 0;
  const struct {
    const char* name;
    bool (*run)();
  } cases[] = {{"initial-enabled-checked-matrix", InitialState},
               {"enable-preserves-checked", EnablePreservesCheck},
               {"check-preserves-disabled-and-default", CheckPreservesDisabled},
               {"label-preserves-native-identity", LabelPreservesIdentity}};
  for (const auto& test : cases) {
    const bool passed = test.run();
    std::printf("%s %s\n", passed ? "PASS" : "FAIL", test.name);
    failures += passed ? 0 : 1;
  }
  std::printf("WINDOWS_NATIVE_TRAY_MENU tests=4 failures=%d\n", failures);
  return failures == 0 ? 0 : 1;
}
