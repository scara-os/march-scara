-- ~/.config/march-scara/init.lua
-- Konfigurasi Kebebasan ala Hyprland untuk MARCH SCARA

scara.setup({
    layout = "hybrid",         -- Pilihan: "tiling", "floating", atau "hybrid"
    rounding = 18.0,
    glass_refraction = 0.016,
    blur_passes = 4,
})

scara.bind("SUPER+Return",   "exec:kitty")
scara.bind("SUPER+E",        "exec:thunar")
scara.bind("SUPER+Q",        "window:close")
scara.bind("SUPER+F",        "window:toggle_floating")
scara.bind("SUPER+Left",     "window:tile_left")
scara.bind("SUPER+Right",    "window:tile_right")
scara.bind("SUPER+M",        "window:center")
scara.bind("SUPER+T",        "layout:toggle")
scara.bind("SUPER+SHIFT+R",  "config:reload")

for i = 1, 5 do
    scara.bind("SUPER+" .. i,       "workspace:" .. i)
    scara.bind("SUPER+SHIFT+" .. i, "movetoworkspace:" .. i)
end
