#include <flutter_linux/flutter_linux.h>
#include <gtk/gtk.h>

#include <cstdint>
#include <initializer_list>

#include "menu.h"

namespace {

FlValue* item(int64_t identifier, const char* type, const char* label) {
  FlValue* value = fl_value_new_map();
  fl_value_set_string_take(value, "id", fl_value_new_int(identifier));
  fl_value_set_string_take(value, "type", fl_value_new_string(type));
  fl_value_set_string_take(value, "label", fl_value_new_string(label));
  return value;
}

FlValue* initial_menu() {
  FlValue* items = fl_value_new_list();
  fl_value_append_take(items, item(1, "label", "Disconnected"));
  g_autoptr(FlValue) group = item(2, "submenu", "Protocol");
  FlValue* children = fl_value_new_list();
  g_autoptr(FlValue) checkbox = item(INT64_C(1099511627776), "checkbox", "aTrust");
  fl_value_set_string_take(checkbox, "checked", fl_value_new_bool(false));
  fl_value_append(children, checkbox);
  fl_value_set_string_take(group, "submenu", children);
  fl_value_append(items, group);
  FlValue* args = fl_value_new_map();
  fl_value_set_string_take(args, "menu_list", items);
  return args;
}

FlValue* edit(int64_t identifier, const char* key, FlValue* value) {
  FlValue* args = fl_value_new_map();
  fl_value_set_string_take(args, "menu_item_id", fl_value_new_int(identifier));
  fl_value_set_string_take(args, key, value);
  return args;
}

GtkWidget* child(GtkWidget* menu, guint index) {
  g_autoptr(GList) children = gtk_container_get_children(GTK_CONTAINER(menu));
  return GTK_WIDGET(g_list_nth_data(children, index));
}

class MenuFixture {
 public:
  MenuFixture() : menu(nullptr, 1) {
    g_autoptr(FlValue) args = initial_menu();
    g_assert_true(menu.create_context_menu(args));
  }
  ~MenuFixture() {
    gtk_widget_destroy(menu.get_menu());
    g_object_unref(menu.get_menu());
  }
  Menu menu;
};

void label_updates_preserve_the_native_menu() {
  MenuFixture fixture;
  GtkWidget* original = fixture.menu.get_menu();
  g_autoptr(FlValue) args = edit(1, "label", fl_value_new_string("Connected"));
  g_autoptr(FlMethodResponse) response = fixture.menu.set_label(args);
  g_assert_true(FL_IS_METHOD_SUCCESS_RESPONSE(response));
  g_assert_true(original == fixture.menu.get_menu());
  g_assert_cmpstr(gtk_menu_item_get_label(GTK_MENU_ITEM(child(original, 0))),
                  ==, "Connected");
  g_autoptr(FlValue) nested = edit(2, "label", fl_value_new_string("Protocols"));
  g_autoptr(FlMethodResponse) nested_response = fixture.menu.set_label(nested);
  g_assert_true(FL_IS_METHOD_SUCCESS_RESPONSE(nested_response));
  g_assert_cmpstr(gtk_menu_item_get_label(GTK_MENU_ITEM(child(original, 1))),
                  ==, "Protocols");
}

void sensitivity_updates_accept_false_and_true() {
  MenuFixture fixture;
  GtkWidget* target = child(fixture.menu.get_menu(), 0);
  for (bool enabled : {false, true}) {
    g_autoptr(FlValue) args = edit(1, "enabled", fl_value_new_bool(enabled));
    g_autoptr(FlMethodResponse) response = fixture.menu.set_enable(args);
    g_assert_true(FL_IS_METHOD_SUCCESS_RESPONSE(response));
    g_assert_cmpint(gtk_widget_get_sensitive(target), ==, enabled);
  }
}

void nested_check_updates_do_not_activate_user_callbacks() {
  MenuFixture fixture;
  GtkWidget* submenu =
      gtk_menu_item_get_submenu(GTK_MENU_ITEM(child(fixture.menu.get_menu(), 1)));
  GtkCheckMenuItem* checkbox = GTK_CHECK_MENU_ITEM(child(submenu, 0));
  for (bool checked : {true, false}) {
    g_autoptr(FlValue) args =
        edit(INT64_C(1099511627776), "checked", fl_value_new_bool(checked));
    // The fixture intentionally has no Flutter channel. An accidental user
    // callback during the programmatic update is a fatal GLib test error.
    g_autoptr(FlMethodResponse) response = fixture.menu.set_check(args);
    g_assert_true(FL_IS_METHOD_SUCCESS_RESPONSE(response));
    g_assert_cmpint(gtk_check_menu_item_get_active(checkbox), ==, checked);
  }
}

void invalid_updates_leave_the_menu_unchanged() {
  MenuFixture fixture;
  g_autoptr(FlValue) invalid = edit(1, "enabled", fl_value_new_string("false"));
  g_autoptr(FlMethodResponse) type_response = fixture.menu.set_enable(invalid);
  g_assert_true(FL_IS_METHOD_ERROR_RESPONSE(type_response));
  g_assert_true(gtk_widget_get_sensitive(child(fixture.menu.get_menu(), 0)));
  g_autoptr(FlValue) unknown = edit(99, "label", fl_value_new_string("Other"));
  g_autoptr(FlMethodResponse) unknown_response = fixture.menu.set_label(unknown);
  g_assert_true(FL_IS_METHOD_ERROR_RESPONSE(unknown_response));
  g_assert_cmpstr(gtk_menu_item_get_label(
                      GTK_MENU_ITEM(child(fixture.menu.get_menu(), 0))),
                  ==, "Disconnected");
  g_autoptr(FlValue) not_checkbox = edit(1, "checked", fl_value_new_bool(true));
  g_autoptr(FlMethodResponse) check_response = fixture.menu.set_check(not_checkbox);
  g_assert_true(FL_IS_METHOD_ERROR_RESPONSE(check_response));
}

}  // namespace

int main(int argc, char** argv) {
  gtk_test_init(&argc, &argv, nullptr);
  g_test_add_func("/tray/label-updates", label_updates_preserve_the_native_menu);
  g_test_add_func("/tray/sensitivity-updates", sensitivity_updates_accept_false_and_true);
  g_test_add_func("/tray/check-updates", nested_check_updates_do_not_activate_user_callbacks);
  g_test_add_func("/tray/invalid-updates", invalid_updates_leave_the_menu_unchanged);
  return g_test_run();
}
