use ab_glyph::{point, Font, FontRef, Glyph, PxScale, ScaleFont};
use anyhow::Result;
use cogex_core::{ArrowDirection, Phase, StimulusType, TrialState};
use std::collections::HashMap;
use tiny_skia::{
    Color, FillRule, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Transform,
};

/// Glyph cache key (glyph ID + scale).
#[derive(Hash, Eq, PartialEq, Clone, Copy)]
pub struct GlyphCacheKey {
    pub id: u16,
    pub scale_bits: u32,
}

/// Cached glyph bitmap and metrics.
#[derive(Clone)]
pub struct CachedGlyph {
    pub bitmap: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub bearing_x: i32,
    pub bearing_y: i32,
}

/// Core renderer trait for drawing primitives and text.
pub trait Renderer {
    type Canvas;

    fn clear(&mut self, canvas: &mut Self::Canvas);
    fn draw_circle(
        &mut self,
        canvas: &mut Self::Canvas,
        center: (f32, f32),
        radius: f32,
        color: [u8; 4],
    );
    fn draw_rect(
        &mut self,
        canvas: &mut Self::Canvas,
        top_left: (f32, f32),
        size: (f32, f32),
        color: [u8; 4],
    );
    fn draw_arrow(
        &mut self,
        canvas: &mut Self::Canvas,
        position: (f32, f32),
        direction: ArrowDirection,
        size: f32,
        color: [u8; 4],
    );
    fn draw_text(
        &mut self,
        canvas: &mut Self::Canvas,
        text: &str,
        position: (f32, f32),
        size: f32,
        color: [u8; 4],
    );
}

pub trait StimulusRenderer: Renderer {
    fn draw_stimulus(
        &mut self,
        canvas: &mut Self::Canvas,
        stimulus: &StimulusType,
        position: (f32, f32),
    ) -> Result<()>;
}

impl<R> StimulusRenderer for R
where
    R: Renderer,
{
    fn draw_stimulus(
        &mut self,
        canvas: &mut Self::Canvas,
        stimulus: &StimulusType,
        position: (f32, f32),
    ) -> Result<()> {
        match stimulus {
            StimulusType::Circle { radius, color } => {
                self.draw_circle(canvas, position, *radius, *color);
                Ok(())
            }
            StimulusType::Rectangle {
                width,
                height,
                color,
            } => {
                let top_left = (position.0 - width / 2.0, position.1 - height / 2.0);
                self.draw_rect(canvas, top_left, (*width, *height), *color);
                Ok(())
            }
            StimulusType::Arrow {
                direction,
                size,
                color,
            } => {
                self.draw_arrow(canvas, position, *direction, *size, *color);
                Ok(())
            }
            StimulusType::Text {
                content,
                size,
                color,
            } => {
                self.draw_text(canvas, content, position, *size, *color);
                Ok(())
            }
        }
    }
}

/// High-level trait for rendering by phase, trial state, and progress.
pub trait PhaseRenderer<P: Phase>: Renderer {
    fn render_phase(
        &mut self,
        canvas: &mut Self::Canvas,
        phase: &P,
        stimulus: Option<(&StimulusType, (f32, f32))>,
        trial_state: Option<&TrialState>,
        progress: Option<(usize, usize)>,
    ) -> Result<()>;
}

/// Skia-based implementation of both traits.
pub struct SkiaRenderer {
    center: (f32, f32),
    font: FontRef<'static>,
    cache: HashMap<GlyphCacheKey, CachedGlyph>,
}

impl SkiaRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let font_bytes = include_bytes!("../../assets/DejaVuSans.ttf") as &[u8];
        let font = FontRef::try_from_slice(font_bytes).unwrap();
        Self {
            center: (width as f32 / 2.0, height as f32 / 2.0),
            font,
            cache: HashMap::with_capacity(256),
        }
    }

    /// Internal: rasterize and cache a glyph.
    fn cache_glyph(&mut self, gid: ab_glyph::GlyphId, scale: PxScale, key: GlyphCacheKey) {
        if self.cache.contains_key(&key) {
            return;
        }
        let scaled = self.font.as_scaled(scale);
        if let Some(outlined) = scaled.outline_glyph(Glyph {
            id: gid,
            scale,
            position: point(0.0, 0.0),
        }) {
            let bounds = outlined.px_bounds();
            let w = bounds.width().ceil() as u32;
            let h = bounds.height().ceil() as u32;
            if w > 0 && h > 0 {
                let mut bitmap = vec![0u8; (w * h) as usize];
                outlined.draw(|x, y, alpha| {
                    bitmap[(y * w + x) as usize] = (alpha * 255.0) as u8;
                });
                self.cache.insert(
                    key,
                    CachedGlyph {
                        bitmap,
                        width: w,
                        height: h,
                        bearing_x: bounds.min.x.floor() as i32,
                        bearing_y: bounds.min.y.floor() as i32,
                    },
                );
            }
        }
    }

    /// Internal: composite a cached glyph onto the pixmap.
    #[allow(clippy::too_many_arguments)]
    fn blit_glyph(
        &self,
        pixels: &mut [PremultipliedColorU8],
        pw: u32,
        ph: u32,
        gx: f32,
        gy: f32,
        cached: &CachedGlyph,
        color: [u8; 4],
    ) {
        let [cr, cg, cb, ca] = color;
        let cr_f = cr as f32 / 255.0;
        let cg_f = cg as f32 / 255.0;
        let cb_f = cb as f32 / 255.0;
        let ca_f = ca as f32 / 255.0;
        let px = gx as i32 + cached.bearing_x;
        let py = gy as i32 + cached.bearing_y;
        let wi = pw as i32;
        let hi = ph as i32;

        for row in 0..cached.height as i32 {
            let dst_row = py + row;
            if dst_row < 0 || dst_row >= hi {
                continue;
            }
            let base_src = (row * cached.width as i32) as usize;
            let base_dst = (dst_row as u32 * pw) as usize;
            for col in 0..cached.width as i32 {
                let dst_col = px + col;
                if dst_col < 0 || dst_col >= wi {
                    continue;
                }
                let alpha = cached.bitmap[base_src + col as usize] as f32 / 255.0 * ca_f;
                if alpha <= 0.0 {
                    continue;
                }
                let idx = base_dst + dst_col as usize;
                let dst = pixels[idx];
                let inv = 1.0 - alpha;
                let r = (cr_f * alpha * 255.0) as u8;
                let g = (cg_f * alpha * 255.0) as u8;
                let b = (cb_f * alpha * 255.0) as u8;
                let a = (alpha * 255.0) as u8;
                let out_r = (r as f32 + dst.red() as f32 * inv) as u8;
                let out_g = (g as f32 + dst.green() as f32 * inv) as u8;
                let out_b = (b as f32 + dst.blue() as f32 * inv) as u8;
                let out_a = a.max(dst.alpha());
                pixels[idx] = PremultipliedColorU8::from_rgba(
                    out_r.min(out_a),
                    out_g.min(out_a),
                    out_b.min(out_a),
                    out_a,
                )
                .unwrap();
            }
        }
    }
}

impl Renderer for SkiaRenderer {
    type Canvas = Pixmap;

    fn clear(&mut self, canvas: &mut Pixmap) {
        canvas.fill(Color::BLACK);
    }

    fn draw_circle(
        &mut self,
        canvas: &mut Pixmap,
        center: (f32, f32),
        radius: f32,
        color: [u8; 4],
    ) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        paint.anti_alias = true;
        let mut pb = PathBuilder::new();
        pb.push_circle(center.0, center.1, radius);
        canvas.fill_path(
            &pb.finish().unwrap(),
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn draw_rect(
        &mut self,
        canvas: &mut Pixmap,
        top_left: (f32, f32),
        size: (f32, f32),
        color: [u8; 4],
    ) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        let rect = Rect::from_xywh(top_left.0, top_left.1, size.0, size.1).unwrap();
        canvas.fill_rect(rect, &paint, Transform::identity(), None);
    }

    fn draw_arrow(
        &mut self,
        canvas: &mut Pixmap,
        position: (f32, f32),
        direction: ArrowDirection,
        size: f32,
        color: [u8; 4],
    ) {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        paint.anti_alias = true;

        let mut pb = PathBuilder::new();
        let (x, y) = position;

        // Simple arrow shape construction depending on direction
        match direction {
            ArrowDirection::Up => {
                pb.move_to(x, y - size);
                pb.line_to(x - size / 2.0, y + size / 2.0);
                pb.line_to(x + size / 2.0, y + size / 2.0);
                pb.close();
            }
            ArrowDirection::Down => {
                pb.move_to(x, y + size);
                pb.line_to(x - size / 2.0, y - size / 2.0);
                pb.line_to(x + size / 2.0, y - size / 2.0);
                pb.close();
            }
            ArrowDirection::Left => {
                pb.move_to(x - size, y);
                pb.line_to(x + size / 2.0, y - size / 2.0);
                pb.line_to(x + size / 2.0, y + size / 2.0);
                pb.close();
            }
            ArrowDirection::Right => {
                pb.move_to(x + size, y);
                pb.line_to(x - size / 2.0, y - size / 2.0);
                pb.line_to(x - size / 2.0, y + size / 2.0);
                pb.close();
            }
        }

        canvas.fill_path(
            &pb.finish().unwrap(),
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    fn draw_text(
        &mut self,
        canvas: &mut Pixmap,
        text: &str,
        (x, y): (f32, f32),
        size: f32,
        color: [u8; 4],
    ) {
        let w = canvas.width();
        let h = canvas.height();
        let scale = PxScale::from(size);
        let mut pen = x;
        let mut prev = None;
        // Stage 1: layout
        let mut glyphs = Vec::new();

        for ch in text.chars() {
            let gid = self.font.glyph_id(ch);
            if let Some(pg) = prev {
                pen += self.font.as_scaled(scale).kern(pg, gid);
            }
            let gl = Glyph {
                id: gid,
                scale,
                position: point(pen, y),
            };
            pen += self.font.as_scaled(scale).h_advance(gid);
            let key = GlyphCacheKey {
                id: gid.0,
                scale_bits: size.to_bits(),
            };
            glyphs.push((gl, key));
            prev = Some(gid);
        }

        // Stage 2: cache misses
        for (_gl, key) in &glyphs {
            let gid = ab_glyph::GlyphId(key.id);
            self.cache_glyph(gid, scale, *key);
        }

        // Stage 3: blit
        let pixels = canvas.pixels_mut();
        for (gl, key) in glyphs {
            if let Some(cg) = self.cache.get(&key) {
                self.blit_glyph(pixels, w, h, gl.position.x, gl.position.y, cg, color);
            }
        }
    }
}

impl<P> PhaseRenderer<P> for SkiaRenderer
where
    P: Phase,
{
    fn render_phase(
        &mut self,
        canvas: &mut Pixmap,
        phase: &P,
        stim: Option<(&StimulusType, (f32, f32))>,
        ts: Option<&TrialState>,
        prog: Option<(usize, usize)>,
    ) -> Result<()> {
        self.clear(canvas);
        match phase {
            p if p.is_welcome() /* Welcome */ => {
                self.draw_text(canvas, "WELCOME", self.center, 32.0, [255,255,255,255]);
            }
            p if p.requires_calibration() /* Calibration */ => {
                self.draw_text(canvas, "CALIBRATING...", self.center, 18.0, [255,255,255,255]);
            }
            p if p.is_practice() || p.is_experiment() => {
                if let Some((s,pos)) = stim {
                    self.draw_stimulus(canvas, s, pos)?;
                }
                if let Some((n,max)) = prog {
                    self.draw_text(
                        canvas,
                        &format!("{}/{}", n, max),
                        (10.0, 20.0),
                        14.0,
                        [200,200,200,255],
                    );
                }
                if let Some(ts) = ts {
                    let msg = match ts {
                        TrialState::Fixation  => "+",
                        TrialState::Stimulus  => ".",
                        TrialState::Response  => "?",
                        TrialState::Feedback  => "!",
                        TrialState::Complete  => "",
                    };
                    self.draw_text(canvas, msg, self.center, 24.0, [255,255,0,255]);
                }
            }
            p if matches!(p.next(), None) /* Debrief */ => {
                self.draw_text(canvas, "EXPERIMENT COMPLETE", self.center, 28.0, [255,255,255,255]);
                // draw stats…
            }
            _ => {}
        }
        Ok(())
    }
}
