#ifndef SHUVPN_RUNNER_VIEW_ACCESSIBILITY_H_
#define SHUVPN_RUNNER_VIEW_ACCESSIBILITY_H_

#include <gtk/gtk.h>

inline void shuvpn_release_view_socket_parent(GtkWidget* view,
                                              AtkObject* socket) {
  AtkObject* parent = atk_object_get_parent(socket);
  if (GTK_IS_ACCESSIBLE(parent) &&
      gtk_accessible_get_widget(GTK_ACCESSIBLE(parent)) == view) {
    atk_object_set_parent(socket, nullptr);
  }
}

inline void shuvpn_prepare_view_accessibility(GtkWidget* view) {
  AtkObject* accessible = gtk_widget_get_accessible(view);
  AtkObject* socket = atk_object_ref_accessible_child(accessible, 0);
  if (socket == nullptr) return;
  // Flutter's socket wrapper omits the parent used by GTK's socket geometry.
  // Keep an existing parent when a future embedder supplies it itself.
  if (ATK_IS_SOCKET(socket) && atk_object_get_parent(socket) == nullptr) {
    atk_object_set_parent(socket, accessible);
    // ATK parents are strong references. Break the socket/wrapper cycle before
    // GtkWidget's destroy cleanup releases its accessible object.
    g_signal_connect_object(view, "destroy",
                            G_CALLBACK(shuvpn_release_view_socket_parent),
                            socket, static_cast<GConnectFlags>(0));
  }
  g_object_unref(socket);
}

#endif  // SHUVPN_RUNNER_VIEW_ACCESSIBILITY_H_
