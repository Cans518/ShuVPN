/// Presentation state shared by the connection page and desktop tray.
enum ConnectionStatus {
  disconnected,
  connecting,
  authenticated,
  connected,
  disconnecting,
  error,
}

/// Categories used to present command failures; native errors retain their text.
enum ConnectionErrorCode {
  invalidOptions,
  authenticationFailed,
  tunnelFailed,
  unsupported,
}
