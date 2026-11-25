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

These are optional, administrator-authorized system changes. The Settings UI
reports when the corresponding service is unavailable; it does not request or
store administrator passwords.
