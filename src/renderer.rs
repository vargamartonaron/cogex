use crate::experiment::{
    ArrowDirection, ExperimentPhase, ExperimentState, StimulusType, TrialState,
};
use ab_glyph::{point, Font, FontRef, Glyph, PxScale, ScaleFont};
use anyhow::Result;
use std::collections::HashMap;
use tiny_skia::{
    Color, FillRule, Paint, PathBuilder, Pixmap, PremultipliedColorU8, Rect, Stroke, Transform,
};

/// High-performance renderer for cognitive experiment stimuli
// #[derive(Default)]
pub struct ExperimentRenderer {
    width: u32,
    height: u32,
    center_x: f32,
    center_y: f32,
    font: FontRef<'static>,
    glyph_cache: HashMap<GlyphCacheKey, CachedGlyph>,
}

#[derive(Clone)]
struct CachedGlyph {
    bitmap: Vec<u8>,
    width: u32,
    height: u32,
    bearing_x: i32,
    bearing_y: i32,
}

#[derive(Hash, Eq, PartialEq, Clone, Copy)]
struct GlyphCacheKey {
    glyph_id: u16,
    scale_bits: u32, // f32 bits for exact scale matching
}

impl ExperimentRenderer {
    pub fn new(width: u32, height: u32) -> Self {
        let font_bytes = include_bytes!("../assets/DejaVuSans.ttf") as &[u8];
        let font = FontRef::try_from_slice(font_bytes).unwrap();

        Self {
            width,
            height,
            center_x: width as f32 / 2.0,
            center_y: height as f32 / 2.0,
            font,
            glyph_cache: HashMap::with_capacity(256),
        }
    }

    /// Render a complete frame based on experiment state
    pub fn render_frame(&mut self, pixmap: &mut Pixmap, state: &ExperimentState) -> Result<()> {
        // Clear background to black
        pixmap.fill(Color::BLACK);

        match state.phase {
            ExperimentPhase::Welcome => {
                self.render_welcome_screen(pixmap)?;
            }
            ExperimentPhase::Calibration => {
                self.render_calibration_screen(pixmap)?;
            }
            ExperimentPhase::Practice => {
                self.render_trial_screen(pixmap, state)?;
                self.render_practice_indicator(pixmap)?;
            }
            ExperimentPhase::Experiment => {
                self.render_trial_screen(pixmap, state)?;
            }
            ExperimentPhase::Debrief => {
                self.render_debrief_screen(pixmap, state)?;
            }
        }

        Ok(())
    }

    fn render_welcome_screen(&mut self, pixmap: &mut Pixmap) -> Result<()> {
        // Draw welcome text
        self.draw_text(
            pixmap,
            "COGNITIVE EXPERIMENT",
            self.center_x,
            self.center_y - 60.0,
            32.0,
            Color::WHITE,
        )?;

        self.draw_text(
            pixmap,
            "Press SPACE to begin practice",
            self.center_x,
            self.center_y + 20.0,
            18.0,
            Color::from_rgba8(200, 200, 200, 255),
        )?;

        self.draw_text(
            pixmap,
            "Press ESC to exit",
            self.center_x,
            self.center_y + 50.0,
            14.0,
            Color::from_rgba8(150, 150, 150, 255),
        )?;

        Ok(())
    }

    fn render_calibration_screen(&mut self, pixmap: &mut Pixmap) -> Result<()> {
        self.draw_text(
            pixmap,
            "Calibrating... Please wait",
            self.center_x,
            self.center_y + 50.0,
            14.0,
            Color::WHITE,
        )?;

        Ok(())
    }

    fn render_trial_screen(&mut self, pixmap: &mut Pixmap, state: &ExperimentState) -> Result<()> {
        if let Some(trial) = &state.current_trial {
            match trial.state {
                TrialState::Fixation => {
                    self.render_fixation_cross(pixmap)?;
                }
                TrialState::Stimulus => {
                    self.render_stimulus(pixmap, &trial.stimulus, trial.position)?;
                }
                TrialState::Response => {
                    // Keep stimulus visible during response window
                    self.render_stimulus(pixmap, &trial.stimulus, trial.position)?;
                    self.render_response_prompt(pixmap)?;
                }
                TrialState::Feedback => {
                    self.render_feedback(pixmap, trial.response_ns.is_some())?;
                }
                TrialState::Complete => {
                    // Blank screen between trials
                }
            }

            // Show trial progress
            self.render_trial_info(pixmap, state)?;
        }

        Ok(())
    }

    fn render_fixation_cross(&self, pixmap: &mut Pixmap) -> Result<()> {
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        paint.anti_alias = true;

        let cross_size = 20.0;
        let stroke_width = 2.0;

        // Horizontal line
        let mut path = PathBuilder::new();
        path.move_to(self.center_x - cross_size, self.center_y);
        path.line_to(self.center_x + cross_size, self.center_y);
        let horizontal_path = path.finish().unwrap();

        let stroke = Stroke {
            width: stroke_width,
            ..Default::default()
        };

        pixmap.stroke_path(
            &horizontal_path,
            &paint,
            &stroke,
            Transform::identity(),
            None,
        );

        // Vertical line
        let mut path = PathBuilder::new();
        path.move_to(self.center_x, self.center_y - cross_size);
        path.line_to(self.center_x, self.center_y + cross_size);
        let vertical_path = path.finish().unwrap();

        pixmap.stroke_path(&vertical_path, &paint, &stroke, Transform::identity(), None);

        Ok(())
    }

    fn render_stimulus(
        &mut self,
        pixmap: &mut Pixmap,
        stimulus: &StimulusType,
        position: (f32, f32),
    ) -> Result<()> {
        let (x, y) = position;

        match stimulus {
            StimulusType::Circle { radius, color } => {
                self.draw_circle(pixmap, x, y, *radius, *color)?;
            }
            StimulusType::Rectangle {
                width,
                height,
                color,
            } => {
                self.draw_rectangle(pixmap, x, y, *width, *height, *color)?;
            }
            StimulusType::Arrow {
                direction,
                size,
                color,
            } => {
                self.draw_arrow(pixmap, x, y, direction.clone(), *size, *color)?;
            }
            StimulusType::Text {
                content,
                size,
                color,
            } => {
                let text_color = Color::from_rgba8(color[0], color[1], color[2], color[3]);
                self.draw_text(pixmap, content, x, y, *size, text_color)?;
            }
        }

        Ok(())
    }

    fn draw_circle(
        &self,
        pixmap: &mut Pixmap,
        x: f32,
        y: f32,
        radius: f32,
        color: [u8; 4],
    ) -> Result<()> {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        paint.anti_alias = true;

        let mut path = PathBuilder::new();
        path.push_circle(x, y, radius);
        let circle_path = path.finish().unwrap();

        pixmap.fill_path(
            &circle_path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        Ok(())
    }

    fn draw_rectangle(
        &self,
        pixmap: &mut Pixmap,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [u8; 4],
    ) -> Result<()> {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        paint.anti_alias = true;

        let rect = Rect::from_xywh(x - width / 2.0, y - height / 2.0, width, height).unwrap();
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);

        Ok(())
    }

    fn draw_arrow(
        &self,
        pixmap: &mut Pixmap,
        x: f32,
        y: f32,
        direction: ArrowDirection,
        size: f32,
        color: [u8; 4],
    ) -> Result<()> {
        let mut paint = Paint::default();
        paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
        paint.anti_alias = true;

        let mut path = PathBuilder::new();

        match direction {
            ArrowDirection::Up => {
                path.move_to(x, y - size / 2.0);
                path.line_to(x - size / 3.0, y + size / 2.0);
                path.line_to(x - size / 6.0, y + size / 6.0);
                path.line_to(x + size / 6.0, y + size / 6.0);
                path.line_to(x + size / 3.0, y + size / 2.0);
            }
            ArrowDirection::Down => {
                path.move_to(x, y + size / 2.0);
                path.line_to(x - size / 3.0, y - size / 2.0);
                path.line_to(x - size / 6.0, y - size / 6.0);
                path.line_to(x + size / 6.0, y - size / 6.0);
                path.line_to(x + size / 3.0, y - size / 2.0);
            }
            ArrowDirection::Left => {
                path.move_to(x - size / 2.0, y);
                path.line_to(x + size / 2.0, y - size / 3.0);
                path.line_to(x + size / 6.0, y - size / 6.0);
                path.line_to(x + size / 6.0, y + size / 6.0);
                path.line_to(x + size / 2.0, y + size / 3.0);
            }
            ArrowDirection::Right => {
                path.move_to(x + size / 2.0, y);
                path.line_to(x - size / 2.0, y - size / 3.0);
                path.line_to(x - size / 6.0, y - size / 6.0);
                path.line_to(x - size / 6.0, y + size / 6.0);
                path.line_to(x - size / 2.0, y + size / 3.0);
            }
        }

        path.close();
        let arrow_path = path.finish().unwrap();

        pixmap.fill_path(
            &arrow_path,
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );

        Ok(())
    }

    fn draw_text(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        x: f32,
        baseline_y: f32,
        size: f32,
        color: Color,
    ) -> anyhow::Result<()> {
        let w = pixmap.width();
        let h = pixmap.height();
        let cu8 = color.to_color_u8();
        let (cr, cg, cb, ca) = (cu8.red(), cu8.green(), cu8.blue(), cu8.alpha());

        let scale = PxScale::from(size);

        // Stage 1: layout and find cache misses in a limited scope
        let (glyphs_to_draw, misses) = {
            let scaled_font = self.font.as_scaled(scale); // immutable borrow of self via &self.font
            let mut pen_x = x;
            let mut prev = None;
            let mut glyphs = Vec::with_capacity(text.len());
            let mut misses: Vec<(ab_glyph::GlyphId, PxScale, GlyphCacheKey)> = Vec::new();

            for ch in text.chars() {
                let gid = self.font.glyph_id(ch);
                // kerning
                if let Some(prev_gid) = prev {
                    pen_x += scaled_font.kern(prev_gid, gid);
                }
                let glyph = Glyph {
                    id: gid,
                    scale,
                    position: point(pen_x, baseline_y),
                };

                let key = GlyphCacheKey {
                    glyph_id: gid.0,
                    scale_bits: size.to_bits(),
                };
                if !self.glyph_cache.contains_key(&key) {
                    // record miss details needed to build cache later
                    misses.push((gid, scale, key));
                }
                glyphs.push((glyph, key));
                pen_x += scaled_font.h_advance(gid);

                prev = Some(gid);
            }

            (glyphs, misses)
        }; // scaled_font borrow ends here

        // Stage 2: fill cache for misses (now we can mutably borrow self)
        if !misses.is_empty() {
            // Recreate scaled_font inside this new scope if needed (immutably again)
            let scaled_font = self.font.as_scaled(scale);
            for (gid, sc, key) in misses {
                let g = Glyph {
                    id: gid,
                    scale: sc,
                    position: point(0.0, 0.0),
                };
                Self::cache_glyph_impl(&mut self.glyph_cache, scaled_font, g, key);
            }
        }

        // Stage 3: blit cached glyphs
        let pixels = pixmap.pixels_mut();
        for (glyph, key) in glyphs_to_draw {
            if let Some(cached) = self.glyph_cache.get(&key) {
                self.blit_cached_glyph(pixels, w, h, &glyph, cached, cr, cg, cb, ca);
            }
        }

        Ok(())
    }

    // Free function so taking &mut self is not needed; pass only what is required.
    // Also avoids borrowing all of self when only cache is mutated.
    fn cache_glyph_impl(
        cache: &mut HashMap<GlyphCacheKey, CachedGlyph>,
        scaled_font: ab_glyph::PxScaleFont<&FontRef<'static>>,
        glyph: Glyph,
        key: GlyphCacheKey,
    ) {
        if let Some(outlined) = scaled_font.outline_glyph(glyph) {
            let bounds = outlined.px_bounds();
            let w = bounds.width().ceil() as u32;
            let h = bounds.height().ceil() as u32;
            if w == 0 || h == 0 {
                return;
            }
            let mut bitmap = vec![0u8; (w * h) as usize];
            outlined.draw(|x, y, cov| {
                bitmap[(y * w + x) as usize] = (cov * 255.0) as u8;
            });
            cache.insert(
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

    #[inline]
    fn blit_cached_glyph(
        &self,
        pixels: &mut [PremultipliedColorU8],
        w: u32,
        h: u32,
        glyph: &Glyph,
        cached: &CachedGlyph,
        cr: u8,
        cg: u8,
        cb: u8,
        ca: u8,
    ) {
        let glyph_x = glyph.position.x as i32 + cached.bearing_x;
        let glyph_y = glyph.position.y as i32 + cached.bearing_y;

        let wi = w as i32;
        let hi = h as i32;

        // Precompute color multipliers for performance
        let cr_f = cr as f32 / 255.0;
        let cg_f = cg as f32 / 255.0;
        let cb_f = cb as f32 / 255.0;
        let ca_f = ca as f32 / 255.0;

        for gy in 0..cached.height as i32 {
            let py = glyph_y + gy;
            if py < 0 || py >= hi {
                continue;
            }

            let src_row_start = (gy as u32 * cached.width) as usize;
            let dst_row_start = (py as u32 * w) as usize;

            for gx in 0..cached.width as i32 {
                let px = glyph_x + gx;
                if px < 0 || px >= wi {
                    continue;
                }

                let coverage = cached.bitmap[src_row_start + gx as usize];
                if coverage == 0 {
                    continue;
                }

                let coverage_f = (coverage as f32) / 255.0;
                let alpha = ca_f * coverage_f;

                if alpha >= 0.999 {
                    // Opaque fast path - direct assignment
                    pixels[dst_row_start + px as usize] =
                        PremultipliedColorU8::from_rgba(cr, cg, cb, 255).unwrap();
                } else {
                    // Alpha blending path with premultiplied math
                    let dst_idx = dst_row_start + px as usize;
                    let dst = &pixels[dst_idx];

                    let src_r = (cr_f * alpha * 255.0) as u8;
                    let src_g = (cg_f * alpha * 255.0) as u8;
                    let src_b = (cb_f * alpha * 255.0) as u8;
                    let src_a = (alpha * 255.0) as u8;

                    let inv = 1.0 - alpha;
                    let out_r = ((src_r as f32) + (dst.red() as f32) * inv) as u8;
                    let out_g = ((src_g as f32) + (dst.green() as f32) * inv) as u8;
                    let out_b = ((src_b as f32) + (dst.blue() as f32) * inv) as u8;
                    let out_a = src_a.max(dst.alpha());

                    pixels[dst_idx] = PremultipliedColorU8::from_rgba(
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

    fn render_response_prompt(&mut self, pixmap: &mut Pixmap) -> Result<()> {
        self.draw_text(
            pixmap,
            "Press SPACE to respond",
            self.center_x,
            self.height as f32 - 50.0,
            16.0,
            Color::from_rgba8(255, 255, 0, 255),
        )?;

        Ok(())
    }

    fn render_feedback(&mut self, pixmap: &mut Pixmap, response_given: bool) -> Result<()> {
        let (text, color) = if response_given {
            ("CORRECT", Color::from_rgba8(0, 255, 0, 255))
        } else {
            ("NO RESPONSE", Color::from_rgba8(255, 0, 0, 255))
        };

        self.draw_text(pixmap, text, self.center_x, self.center_y, 24.0, color)?;

        Ok(())
    }

    fn render_trial_info(&mut self, pixmap: &mut Pixmap, state: &ExperimentState) -> Result<()> {
        let phase_text = match state.phase {
            ExperimentPhase::Practice => {
                format!("Practice: {}/{}", state.trial_num + 1, state.practice_max)
            }
            ExperimentPhase::Experiment => {
                format!("Trial: {}/{}", state.trial_num + 1, state.experiment_max)
            }
            _ => String::new(),
        };

        if !phase_text.is_empty() {
            self.draw_text(
                pixmap,
                &phase_text,
                50.0,
                30.0,
                14.0,
                Color::from_rgba8(150, 150, 150, 255),
            )?;
        }

        Ok(())
    }

    fn render_practice_indicator(&mut self, pixmap: &mut Pixmap) -> Result<()> {
        self.draw_text(
            pixmap,
            "PRACTICE MODE",
            self.width as f32 - 100.0,
            30.0,
            14.0,
            Color::from_rgba8(255, 255, 0, 255),
        )?;

        Ok(())
    }

    fn render_debrief_screen(
        &mut self,
        pixmap: &mut Pixmap,
        state: &ExperimentState,
    ) -> Result<()> {
        self.draw_text(
            pixmap,
            "EXPERIMENT COMPLETE",
            self.center_x,
            self.center_y - 80.0,
            28.0,
            Color::WHITE,
        )?;

        // Show basic stats
        let valid_responses = state
            .results
            .iter()
            .filter(|r| r.reaction_ns.is_some())
            .count();

        let response_rate = if !state.results.is_empty() {
            valid_responses as f32 / state.results.len() as f32 * 100.0
        } else {
            0.0
        };

        let stats_text = format!("Response Rate: {:.1}%", response_rate);
        self.draw_text(
            pixmap,
            &stats_text,
            self.center_x,
            self.center_y - 20.0,
            18.0,
            Color::from_rgba8(200, 200, 200, 255),
        )?;

        if valid_responses > 0 {
            let reaction_times: Vec<f64> = state
                .results
                .iter()
                .filter_map(|r| r.reaction_ns)
                .map(|rt| rt as f64 / 1_000_000.0)
                .collect();

            if !reaction_times.is_empty() {
                let mean_rt = reaction_times.iter().sum::<f64>() / reaction_times.len() as f64;
                let rt_text = format!("Mean RT: {:.1} ms", mean_rt);
                self.draw_text(
                    pixmap,
                    &rt_text,
                    self.center_x,
                    self.center_y + 10.0,
                    18.0,
                    Color::from_rgba8(200, 200, 200, 255),
                )?;
            }
        }

        self.draw_text(
            pixmap,
            "Results saved. Thank you!",
            self.center_x,
            self.center_y + 50.0,
            16.0,
            Color::from_rgba8(150, 150, 150, 255),
        )?;

        Ok(())
    }
}
