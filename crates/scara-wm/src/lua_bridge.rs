use mlua::prelude::*;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct ScaraConfig {
    pub rounding: f32,
    pub glass_refraction: f32,
    pub blur_passes: u32,
    pub default_layout: String, // "tiling", "floating", "hybrid"
    pub keybinds: Vec<(String, String)>,
}

impl Default for ScaraConfig {
    fn default() -> Self {
        Self {
            rounding: 18.0,
            glass_refraction: 0.015,
            blur_passes: 4,
            default_layout: "hybrid".into(),
            keybinds: vec![],
        }
    }
}

pub fn load_user_config(path: &str) -> LuaResult<Arc<Mutex<ScaraConfig>>> {
    let lua = Lua::new();
    let cfg = Arc::new(Mutex::new(ScaraConfig::default()));
    let scara_tbl = lua.create_table()?;

    // scara.setup({ rounding = 18, glass_refraction = 0.018, layout = "hybrid" })
    {
        let cfg_ref = Arc::clone(&cfg);
        let setup_fn = lua.create_function(move |_, opts: LuaTable| {
            let mut c = cfg_ref.lock().unwrap();
            if let Ok(r) = opts.get::<f32>("rounding") {
                c.rounding = r;
            }
            if let Ok(g) = opts.get::<f32>("glass_refraction") {
                c.glass_refraction = g;
            }
            if let Ok(b) = opts.get::<u32>("blur_passes") {
                c.blur_passes = b;
            }
            if let Ok(l) = opts.get::<String>("layout") {
                c.default_layout = l;
            }
            Ok(())
        })?;
        scara_tbl.set("setup", setup_fn)?;
    }

    // scara.bind("SUPER+Return", "exec:kitty")
    {
        let cfg_ref = Arc::clone(&cfg);
        let bind_fn = lua.create_function(move |_, (combo, action): (String, String)| {
            cfg_ref.lock().unwrap().keybinds.push((combo, action));
            Ok(())
        })?;
        scara_tbl.set("bind", bind_fn)?;
    }

    lua.globals().set("scara", scara_tbl)?;

    if let Ok(script) = std::fs::read_to_string(path) {
        lua.load(&script).exec()?;
    }
    Ok(cfg)
}
