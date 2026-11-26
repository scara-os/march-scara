use smithay::{
    backend::{
        allocator::Fourcc,
        renderer::{
            damage::OutputDamageTracker,
            element::{
                render_elements,
                solid::{SolidColorBuffer, SolidColorRenderElement},
                surface::WaylandSurfaceRenderElement,
                texture::TextureRenderElement,
                AsRenderElements, Id, Kind,
            },
            gles::{
                element::TextureShaderElement, GlesRenderer, GlesTarget, GlesTexProgram,
                GlesTexture, Uniform, UniformName, UniformType,
            },
            utils::draw_render_elements,
            Bind, Color32F, Frame, Offscreen, Renderer,
        },
    },
    desktop::{layer_map_for_output, Space, Window},
    output::Output,
    utils::{Buffer as BufferCoord, Physical, Point, Rectangle, Scale, Size, Transform},
    wayland::shell::wlr_layer::Layer as WlrLayer,
};

render_elements! {
    pub ScaraRenderElements<=GlesRenderer>;
    Wayland=WaylandSurfaceRenderElement<GlesRenderer>,
    Glass=TextureShaderElement,
    Cursor=SolidColorRenderElement,
}

const BLUR_FRAG: &str = r#"
#version 100

//_DEFINES_

#if defined(EXTERNAL)
#extension GL_OES_EGL_image_external : require
#endif

precision highp float;

#if defined(EXTERNAL)
uniform samplerExternalOES tex;
#else
uniform sampler2D tex;
#endif

uniform float alpha;
varying vec2 v_coords;

uniform vec2 u_texel_size;
uniform vec2 u_blur_dir;

void main() {
    vec2 step_uv = u_texel_size * u_blur_dir;
    vec4 sum = vec4(0.0);

    sum += texture2D(tex, clamp(v_coords - step_uv * 4.0, 0.001, 0.999)) * 0.051;
    sum += texture2D(tex, clamp(v_coords - step_uv * 3.0, 0.001, 0.999)) * 0.0918;
    sum += texture2D(tex, clamp(v_coords - step_uv * 2.0, 0.001, 0.999)) * 0.12245;
    sum += texture2D(tex, clamp(v_coords - step_uv * 1.0, 0.001, 0.999)) * 0.1531;
    sum += texture2D(tex, clamp(v_coords,                 0.001, 0.999)) * 0.1633;
    sum += texture2D(tex, clamp(v_coords + step_uv * 1.0, 0.001, 0.999)) * 0.1531;
    sum += texture2D(tex, clamp(v_coords + step_uv * 2.0, 0.001, 0.999)) * 0.12245;
    sum += texture2D(tex, clamp(v_coords + step_uv * 3.0, 0.001, 0.999)) * 0.0918;
    sum += texture2D(tex, clamp(v_coords + step_uv * 4.0, 0.001, 0.999)) * 0.051;

    gl_FragColor = vec4(sum.rgb, 1.0) * alpha;
}
"#;

const LIQUID_GLASS_FRAG: &str = include_str!("../../../assets/shaders/liquid-glass.frag");

fn glass_effect_parameters(reduce_transparency: bool, refraction: f32) -> (f32, [f32; 4]) {
    if reduce_transparency {
        (0.0, [0.20, 0.22, 0.28, 1.0])
    } else {
        (refraction, [0.16, 0.19, 0.30, 0.14])
    }
}

pub struct GlassPipeline {
    blur_program: GlesTexProgram,
    glass_program: GlesTexProgram,
    cursor_rows: Vec<CursorRow>,
    tex_size: Size<i32, Physical>,
    backdrop_tex: Option<GlesTexture>,
    blur_tex_a: Option<GlesTexture>,
    blur_tex_b: Option<GlesTexture>,
}

struct CursorRow {
    y: i32,
    x: i32,
    fill_x: i32,
    fill_width: i32,
    outline: SolidColorBuffer,
    fill: SolidColorBuffer,
}

impl GlassPipeline {
    pub fn new(renderer: &mut GlesRenderer) -> Result<Self, Box<dyn std::error::Error>> {
        let blur_program = renderer.compile_custom_texture_shader(
            BLUR_FRAG,
            &[
                UniformName::new("u_texel_size", UniformType::_2f),
                UniformName::new("u_blur_dir", UniformType::_2f),
            ],
        )?;

        let glass_program = renderer.compile_custom_texture_shader(
            LIQUID_GLASS_FRAG,
            &[
                UniformName::new("u_resolution", UniformType::_2f),
                UniformName::new("u_panel_uv", UniformType::_4f),
                UniformName::new("u_radius", UniformType::_1f),
                UniformName::new("u_refraction", UniformType::_1f),
                UniformName::new("u_tint", UniformType::_4f),
            ],
        )?;

        let cursor_shape = [
            (0, 0, 0, 0, 2),
            (1, 0, 1, 1, 3),
            (2, 0, 1, 2, 4),
            (3, 0, 1, 3, 5),
            (4, 0, 1, 4, 6),
            (5, 0, 1, 5, 7),
            (6, 0, 1, 6, 8),
            (7, 0, 1, 7, 9),
            (8, 0, 1, 8, 10),
            (9, 0, 1, 9, 11),
            (10, 0, 1, 10, 12),
            (11, 0, 1, 11, 13),
            (12, 0, 1, 12, 14),
            (13, 5, 6, 4, 8),
            (14, 6, 6, 4, 8),
            (15, 7, 7, 4, 9),
            (16, 8, 7, 4, 9),
            (17, 9, 7, 4, 9),
            (18, 10, 7, 4, 9),
        ];
        let cursor_rows = cursor_shape
            .into_iter()
            .map(|(y, x, fill_x, fill_width, width)| CursorRow {
                y,
                x,
                fill_x,
                fill_width,
                outline: SolidColorBuffer::new((width, 1), [0.02, 0.02, 0.02, 1.0]),
                fill: SolidColorBuffer::new((fill_width.max(1), 1), [1.0, 1.0, 1.0, 1.0]),
            })
            .collect();

        Ok(Self {
            blur_program,
            glass_program,
            cursor_rows,
            tex_size: (0, 0).into(),
            backdrop_tex: None,
            blur_tex_a: None,
            blur_tex_b: None,
        })
    }

    fn ensure_textures(
        &mut self,
        renderer: &mut GlesRenderer,
        size: Size<i32, Physical>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.tex_size == size
            && self.backdrop_tex.is_some()
            && self.blur_tex_a.is_some()
            && self.blur_tex_b.is_some()
        {
            return Ok(());
        }

        let buf_size: Size<i32, BufferCoord> = (size.w.max(1), size.h.max(1)).into();
        self.backdrop_tex = Some(renderer.create_buffer(Fourcc::Abgr8888, buf_size)?);
        self.blur_tex_a = Some(renderer.create_buffer(Fourcc::Abgr8888, buf_size)?);
        self.blur_tex_b = Some(renderer.create_buffer(Fourcc::Abgr8888, buf_size)?);
        self.tex_size = size;
        Ok(())
    }

    fn collect_backdrop(
        renderer: &mut GlesRenderer,
        output: &Output,
        space: &Space<Window>,
    ) -> Vec<WaylandSurfaceRenderElement<GlesRenderer>> {
        let scale = Scale::from(1.0);
        let mut backdrop = Vec::new();

        for window in space.elements().rev() {
            if let Some(loc) = space.element_location(window) {
                let render_loc = (loc - window.geometry().loc).to_physical_precise_round(1.0);
                let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                    window.render_elements(renderer, render_loc, scale, 1.0);
                backdrop.extend(elems);
            }
        }

        let map = layer_map_for_output(output);
        for level in [WlrLayer::Bottom, WlrLayer::Background] {
            for layer in map.layers() {
                if layer.layer() != level {
                    continue;
                }
                if let Some(geo) = map.layer_geometry(layer) {
                    let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> = layer
                        .render_elements(
                            renderer,
                            geo.loc.to_physical_precise_round(1.0),
                            scale,
                            1.0,
                        );
                    backdrop.extend(elems);
                }
            }
        }

        backdrop
    }

    pub fn render_frame(
        &mut self,
        renderer: &mut GlesRenderer,
        framebuffer: &mut GlesTarget<'_>,
        output: &Output,
        space: &Space<Window>,
        damage_tracker: &mut OutputDamageTracker,
        size: Size<i32, Physical>,
        refraction: f32,
        blur_passes: u32,
        reduce_transparency: bool,
        cursor_location: Option<Point<i32, Physical>>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.ensure_textures(renderer, size)?;

        let full_damage = Rectangle::from_size(size);
        let scale = Scale::from(1.0);

        // 1. Pass 1: Render backdrop (Windows + Bottom + Background) ke offscreen backdrop_tex
        let backdrop_elems = Self::collect_backdrop(renderer, output, space);
        {
            let backdrop_tex = self.backdrop_tex.as_mut().unwrap();
            let mut target = renderer.bind(backdrop_tex)?;
            let mut frame = renderer.render(&mut target, size, Transform::Normal)?;
            frame.clear(Color32F::new(0.12, 0.13, 0.16, 1.0), &[full_damage])?;
            draw_render_elements(&mut frame, 1.0, &backdrop_elems, &[full_damage])?;
            let _ = frame.finish()?;
        }

        // 2. Pass 2: Multi-pass GPU Separable Gaussian Blur (backdrop -> A <-> B)
        let texel_x = 1.0 / (size.w.max(1) as f32);
        let texel_y = 1.0 / (size.h.max(1) as f32);
        let src_rect_buf: Rectangle<f64, BufferCoord> =
            Rectangle::from_size((size.w as f64, size.h as f64).into());

        if !reduce_transparency {
            let passes = blur_passes.clamp(1, 6);
            for pass in 0..passes {
                let spread = (pass + 1) as f32 * 1.35;
                {
                    let src_tex = if pass == 0 {
                        self.backdrop_tex.as_ref().unwrap()
                    } else {
                        self.blur_tex_b.as_ref().unwrap()
                    };
                    let dst_tex = self.blur_tex_a.as_mut().unwrap();
                    let mut target = renderer.bind(dst_tex)?;
                    let mut frame = renderer.render(&mut target, size, Transform::Normal)?;
                    frame.render_texture_from_to(
                        src_tex,
                        src_rect_buf,
                        full_damage,
                        &[full_damage],
                        &[],
                        Transform::Normal,
                        1.0,
                        Some(&self.blur_program),
                        &[
                            Uniform::new("u_texel_size", [texel_x * spread, texel_y * spread]),
                            Uniform::new("u_blur_dir", [1.0, 0.0]),
                        ],
                    )?;
                    let _ = frame.finish()?;
                }

                {
                    let src_tex = self.blur_tex_a.as_ref().unwrap();
                    let dst_tex = self.blur_tex_b.as_mut().unwrap();
                    let mut target = renderer.bind(dst_tex)?;
                    let mut frame = renderer.render(&mut target, size, Transform::Normal)?;
                    frame.render_texture_from_to(
                        src_tex,
                        src_rect_buf,
                        full_damage,
                        &[full_damage],
                        &[],
                        Transform::Normal,
                        1.0,
                        Some(&self.blur_program),
                        &[
                            Uniform::new("u_texel_size", [texel_x * spread, texel_y * spread]),
                            Uniform::new("u_blur_dir", [0.0, 1.0]),
                        ],
                    )?;
                    let _ = frame.finish()?;
                }
            }
        }

        // 3. Kumpulkan elemen layer + posisi panel kaca
        let mut overlay_elems: Vec<ScaraRenderElements> = Vec::new();
        let mut top_elems: Vec<ScaraRenderElements> = Vec::new();
        let mut glass_panels: Vec<(Rectangle<i32, Physical>, f32)> = Vec::new();

        {
            let map = layer_map_for_output(output);
            for layer in map.layers() {
                let Some(geo) = map.layer_geometry(layer) else {
                    continue;
                };
                let loc_phys = geo.loc.to_physical_precise_round(1.0);

                match layer.layer() {
                    WlrLayer::Overlay => {
                        let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                            layer.render_elements(renderer, loc_phys, scale, 1.0);
                        overlay_elems.extend(elems.into_iter().map(ScaraRenderElements::Wayland));
                    }
                    WlrLayer::Top => {
                        let ns = layer.namespace();
                        if ns == "scara-menubar" {
                            glass_panels.push((
                                Rectangle::new(loc_phys, (geo.size.w, geo.size.h).into()),
                                0.0,
                            ));
                        } else if ns == "scara-dock" {
                            let pad_x = 18;
                            let pad_top = 10;
                            let pad_bot = 6;
                            if geo.size.w > pad_x * 2 && geo.size.h > pad_top + pad_bot {
                                glass_panels.push((
                                    Rectangle::new(
                                        (loc_phys.x + pad_x, loc_phys.y + pad_top).into(),
                                        (geo.size.w - pad_x * 2, geo.size.h - pad_top - pad_bot)
                                            .into(),
                                    ),
                                    21.0,
                                ));
                            }
                        }
                        let elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
                            layer.render_elements(renderer, loc_phys, scale, 1.0);
                        top_elems.extend(elems.into_iter().map(ScaraRenderElements::Wayland));
                    }
                    _ => {}
                }
            }
        }

        // 4. Pass 3: Bangun elemen Liquid Glass di belakang Menu Bar & Dock
        let ctx_id = renderer.context_id();
        let glass_backdrop = if reduce_transparency {
            self.backdrop_tex.as_ref().unwrap()
        } else {
            self.blur_tex_b.as_ref().unwrap()
        };
        let blur_tex = glass_backdrop.clone();
        let mut glass_elems: Vec<ScaraRenderElements> = Vec::with_capacity(glass_panels.len());
        let (panel_refraction, panel_tint) =
            glass_effect_parameters(reduce_transparency, refraction);

        for (rect, radius) in glass_panels {
            let uv_x = (rect.loc.x as f32) / (size.w.max(1) as f32);
            let uv_y = (rect.loc.y as f32) / (size.h.max(1) as f32);
            let uv_w = (rect.size.w as f32) / (size.w.max(1) as f32);
            let uv_h = (rect.size.h as f32) / (size.h.max(1) as f32);

            let tex_elem = TextureRenderElement::from_static_texture(
                Id::new(),
                ctx_id.clone(),
                Point::from((rect.loc.x as f64, rect.loc.y as f64)),
                blur_tex.clone(),
                1,
                Transform::Normal,
                Some(1.0),
                Some(Rectangle::new(
                    (rect.loc.x as f64, rect.loc.y as f64).into(),
                    (rect.size.w as f64, rect.size.h as f64).into(),
                )),
                Some((rect.size.w, rect.size.h).into()),
                None,
                Kind::Unspecified,
            );

            let glass_shader_elem = TextureShaderElement::new(
                tex_elem,
                self.glass_program.clone(),
                vec![
                    Uniform::new("u_resolution", [rect.size.w as f32, rect.size.h as f32]),
                    Uniform::new("u_panel_uv", [uv_x, uv_y, uv_w, uv_h]),
                    Uniform::new("u_radius", radius),
                    Uniform::new("u_refraction", panel_refraction),
                    Uniform::new("u_tint", panel_tint),
                ],
            );

            glass_elems.push(ScaraRenderElements::Glass(glass_shader_elem));
        }

        // 5. Gabungkan seluruh adegan akhir (Front-to-Back) lalu render ke layar utama
        let backdrop_final = Self::collect_backdrop(renderer, output, space);
        let mut final_elems: Vec<ScaraRenderElements> = Vec::with_capacity(
            overlay_elems.len()
                + top_elems.len()
                + glass_elems.len()
                + backdrop_final.len()
                + self.cursor_rows.len() * 2,
        );
        if let Some(location) = cursor_location {
            for row in &self.cursor_rows {
                if row.fill_width > 0 {
                    final_elems.push(
                        SolidColorRenderElement::from_buffer(
                            &row.fill,
                            (location.x + row.fill_x, location.y + row.y),
                            1.0,
                            1.0,
                            Kind::Unspecified,
                        )
                        .into(),
                    );
                }
                final_elems.push(
                    SolidColorRenderElement::from_buffer(
                        &row.outline,
                        (location.x + row.x, location.y + row.y),
                        1.0,
                        1.0,
                        Kind::Unspecified,
                    )
                    .into(),
                );
            }
        }

        final_elems.extend(overlay_elems);
        final_elems.extend(top_elems);
        final_elems.extend(glass_elems);
        final_elems.extend(backdrop_final.into_iter().map(ScaraRenderElements::Wayland));

        damage_tracker.render_output(
            renderer,
            framebuffer,
            0,
            &final_elems,
            [0.12, 0.13, 0.16, 1.0],
        )?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::glass_effect_parameters;

    #[test]
    fn reduced_transparency_uses_opaque_non_refractive_glass() {
        let (refraction, tint) = glass_effect_parameters(true, 0.02);
        assert_eq!(refraction, 0.0);
        assert_eq!(tint[3], 1.0);
    }

    #[test]
    fn normal_glass_keeps_configured_refraction_and_translucent_tint() {
        let (refraction, tint) = glass_effect_parameters(false, 0.02);
        assert_eq!(refraction, 0.02);
        assert_eq!(tint[3], 0.14);
    }
}
