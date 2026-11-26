# Post-install system services

MARCH SCARA installs NetworkManager and BlueZ tools, but package installation does
not enable system services or replace services already configured on the host.

If you want MARCH SCARA's Wi-Fi controls to use NetworkManager, and you are not
already using another network manager, enable NetworkManager:

```sh
sudo systemctl enable --now NetworkManager.service
```

For Bluetooth device discovery and management, enable BlueZ:

```sh
sudo systemctl enable --now bluetooth.service
```

Power-mode controls use `power-profiles-daemon` when supported by the host. The
package is installed as a dependency, but its service is not enabled by MARCH
SCARA. If the host has no other power-profile manager and supports this service,
you can enable it explicitly:

```sh
sudo systemctl enable --now power-profiles-daemon.service
```

These are optional, administrator-authorized system changes. The Settings UI
reports when the corresponding service is unavailable; it does not request or
store administrator passwords.

## Portals, permissions, and host integrations

The MARCH SCARA portal configuration routes file selection and settings to the
GTK portal, and screenshot/screen-sharing requests to the wlroots portal. Both
portal backends are installed as dependencies; portal services are activated by
D-Bus and are not enabled by MARCH SCARA. GTK native file choosers are configured
to use the portal.

Applications request supported access through XDG Desktop Portal. The portal
backend owns permission prompts and its PermissionStore; MARCH SCARA does not
keep a second permission database or directly change another application's
permission records. The Privacy & Security panel reports portal interfaces,
Polkit, and available system utilities, including package names for missing
optional utilities.

The package installs a handler for `mactahoe://` settings links. To make it the
current user's default handler, run:

```sh
xdg-mime default march-scara-settings-handler.desktop x-scheme-handler/mactahoe
```

MARCH SCARA does not emulate True Tone, ProMotion, or biometric authentication
on hardware that does not expose a supported Linux interface. Equivalent
controls are offered only when the host's hardware and services report support.

The Notifications & Focus panel controls Mako's Do Not Disturb mode and opens
its per-user configuration for persistent notification rules. The printer panel
reads status and queues from CUPS; it does not add printers or change system
configuration. If you choose to use CUPS, install the optional `cups` package
and enable its service yourself. Software updates open GNOME Software when the
optional `gnome-software` package is installed. Credential availability is
detected through the Secret Service D-Bus API; secrets are never displayed or
copied into MARCH SCARA preferences.
