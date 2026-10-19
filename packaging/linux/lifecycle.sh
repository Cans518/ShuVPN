# Embedded in each maintainer script, with operation set by the package builder.
# Only Essential utilities are needed by preinst.
PATH=/usr/sbin:/usr/bin:/sbin:/bin
export PATH

trusted_directory() {
  path="$1"
  [ -d "$path" ] && [ ! -L "$path" ] || {
    echo "Untrusted installation directory: $path" >&2
    exit 1
  }
  [ "$(stat -c %u "$path")" = 0 ] && [ "$((0$(stat -c %a "$path") & 022))" = 0 ] || {
    echo "Installation directories must be root-owned and not writable by other users: $path" >&2
    exit 1
  }
}

check_parents() {
  for path in / /opt /usr /usr/lib /usr/share /usr/share/applications; do
    if [ -e "$path" ] || [ -L "$path" ]; then
      trusted_directory "$path"
    fi
  done
}

check_paths() {
  check_parents
  for path in /opt/shuvpn /usr/lib/shuvpn; do
    if [ -e "$path" ] || [ -L "$path" ]; then
      trusted_directory "$path"
      [ ! -L "$path/.package-manager" ] && [ -f "$path/.package-manager" ] &&
        [ "$(cat "$path/.package-manager")" = dpkg ] || {
          echo "An unmanaged ShuVPN installation exists at $path. Remove it with its installer first; account data is retained." >&2
          exit 1
        }
    fi
  done
  [ ! -L /usr/share/applications/work.shuvpn.app.desktop ] || {
    echo "Refusing a symlink at the application launcher" >&2
    exit 1
  }
  if [ -e /usr/share/applications/work.shuvpn.app.desktop ] &&
     [ ! -f /opt/shuvpn/.package-manager ]; then
    echo "Refusing an unmanaged application launcher" >&2
    exit 1
  fi
}

retain_residual_ownership() {
  check_parents
  for path in /opt/shuvpn /usr/lib/shuvpn; do
    if [ -e "$path" ] || [ -L "$path" ]; then
      trusted_directory "$path"
      [ ! -L "$path/.package-manager" ] || exit 1
      if [ -n "$(find "$path" -mindepth 1 -maxdepth 1 ! -name .package-manager -print -quit)" ]; then
        # Unowned files must survive remove/purge and remain reinstallable.
        printf 'dpkg\n' > "$path/.package-manager"
        chmod 0644 "$path/.package-manager"
      elif [ -f "$path/.package-manager" ]; then
        rm "$path/.package-manager"
        rmdir "$path"
      fi
    fi
  done
}

check_stopped() {
  for entry in /proc/[0-9]*/exe; do
    executable="$(readlink "$entry" 2>/dev/null || true)"
    case "$executable" in
      /opt/shuvpn/*|/usr/lib/shuvpn/shuvpn_tun_helper*)
        echo "Exit ShuVPN from its tray menu and wait for its tunnel helper to exit before installing or removing it." >&2
        exit 1
        ;;
    esac
  done
}

configure_helper() {
  check_paths
  if ! getent group shuvpn >/dev/null; then
    addgroup --system shuvpn
  fi
  group_id="$(getent group shuvpn | cut -d: -f3)"
  [ -n "$group_id" ] && [ "$group_id" != 0 ] || {
    echo "The shuvpn group must not be root" >&2
    exit 1
  }
  helper=/usr/lib/shuvpn/shuvpn_tun_helper
  [ -f "$helper" ] && [ ! -L "$helper" ]
  chown root:shuvpn "$helper"
  chmod 0750 "$helper"
  setcap cap_net_admin,cap_net_raw+ep "$helper"
  [ "$(getcap "$helper")" = "$helper cap_net_admin,cap_net_raw=ep" ]
  echo "ShuVPN installed. To authorize TUN for an account: sudo usermod -aG shuvpn USER"
  echo "Sign out and sign in after authorization. Account/settings data is retained on removal."
}

case "$operation:${1:-}" in
  preinst:install|preinst:upgrade)
    check_paths
    check_stopped
    ;;
  postinst:configure|postinst:abort-upgrade|postinst:abort-remove|postinst:abort-deconfigure)
    configure_helper
    ;;
  prerm:remove|prerm:upgrade|prerm:deconfigure)
    check_paths
    check_stopped
    ;;
  preinst:abort-upgrade|prerm:failed-upgrade)
    ;;
  postrm:remove|postrm:purge)
    retain_residual_ownership
    ;;
  postrm:upgrade|postrm:failed-upgrade|postrm:abort-install|postrm:abort-upgrade|postrm:disappear)
    ;;
  *)
    echo "Unsupported maintainer-script action: $operation ${1:-}" >&2
    exit 1
    ;;
esac
