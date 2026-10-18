#include <flutter_linux/flutter_linux.h>
#include <gtk/gtk.h>

#include "view_accessibility.h"

namespace {

class UnrealizedView {
 public:
  UnrealizedView() {
    g_autoptr(FlDartProject) project = fl_dart_project_new();
    gchar absent[] = "/nonexistent-shuvpn-accessibility-test";
    fl_dart_project_set_aot_library_path(project, absent);
    fl_dart_project_set_assets_path(project, absent);
    fl_dart_project_set_icu_data_path(project, absent);
    view = GTK_WIDGET(fl_view_new(project));
    g_object_ref_sink(view);
    gtk_widget_show(view);
    GtkAllocation allocation{0, 0, 640, 480};
    gtk_widget_size_allocate(view, &allocation);
    g_assert_false(gtk_widget_get_realized(view));
    accessible = gtk_widget_get_accessible(view);
    socket = atk_object_ref_accessible_child(accessible, 0);
    g_assert_true(ATK_IS_SOCKET(socket));
  }

  ~UnrealizedView() {
    g_assert_false(gtk_widget_get_realized(view));
    g_object_unref(socket);
    gtk_widget_destroy(view);
    g_object_unref(view);
  }

  GtkWidget* view;
  AtkObject* accessible;
  AtkObject* socket;
};

void socket_inherits_wrapper_geometry() {
  UnrealizedView fixture;
  shuvpn_prepare_view_accessibility(fixture.view);
  shuvpn_prepare_view_accessibility(fixture.view);
  g_assert_true(atk_object_get_parent(fixture.socket) == fixture.accessible);
  int actual[4], expected[4];
  atk_component_get_extents(ATK_COMPONENT(fixture.socket), &actual[0], &actual[1],
                           &actual[2], &actual[3], ATK_XY_SCREEN);
  atk_component_get_extents(ATK_COMPONENT(fixture.accessible), &expected[0],
                           &expected[1], &expected[2], &expected[3],
                           ATK_XY_SCREEN);
  for (int axis = 0; axis < 4; ++axis) {
    g_assert_cmpint(actual[axis], ==, expected[axis]);
  }
  g_assert_cmpint(actual[2], ==, 640);
  g_assert_cmpint(actual[3], ==, 480);
}

void an_existing_socket_parent_is_preserved() {
  UnrealizedView fixture;
  AtkObject* existing = atk_no_op_object_new(G_OBJECT(fixture.view));
  atk_object_set_parent(fixture.socket, existing);
  shuvpn_prepare_view_accessibility(fixture.view);
  g_assert_true(atk_object_get_parent(fixture.socket) == existing);
  atk_object_set_parent(fixture.socket, nullptr);
  g_object_unref(existing);
}

void ordinary_widget_accessibility_is_unchanged() {
  GtkWidget* label = gtk_label_new("ordinary widget");
  g_object_ref_sink(label);
  AtkObject* accessible = gtk_widget_get_accessible(label);
  shuvpn_prepare_view_accessibility(label);
  g_assert_true(gtk_widget_get_accessible(label) == accessible);
  gtk_widget_destroy(label);
  g_object_unref(label);
}

void destroying_a_view_releases_accessibility_objects() {
  gpointer accessible = nullptr;
  gpointer socket = nullptr;
  {
    UnrealizedView fixture;
    accessible = fixture.accessible;
    socket = fixture.socket;
    g_object_add_weak_pointer(G_OBJECT(fixture.accessible), &accessible);
    g_object_add_weak_pointer(G_OBJECT(fixture.socket), &socket);
    shuvpn_prepare_view_accessibility(fixture.view);
  }
  g_assert_null(accessible);
  g_assert_null(socket);
}

}  // namespace

int main(int argc, char** argv) {
  gtk_init(&argc, &argv);
  g_test_init(&argc, &argv, nullptr);
  g_test_add_func("/view-accessibility/socket-geometry",
                  socket_inherits_wrapper_geometry);
  g_test_add_func("/view-accessibility/preserve-existing-parent",
                  an_existing_socket_parent_is_preserved);
  g_test_add_func("/view-accessibility/ordinary-widget",
                  ordinary_widget_accessibility_is_unchanged);
  g_test_add_func("/view-accessibility/destroy-releases-objects",
                  destroying_a_view_releases_accessibility_objects);
  return g_test_run();
}
