use ab_glyph::{point, Font, FontRef, Glyph, PxScale, ScaleFont};
use anyhow::Result;
use bytemuck::{cast_slice, cast_slice_mut};
use cogex_cache::{get_text, intern_text, text_count, Atom};
use cogex_core::{ArrowDirection, Phase, StimulusType, TrialState};
use cogex_timing::{HighPrecisionTimer, Timer};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tiny_skia::{
    Color, FillRule, FilterQuality, Paint, PathBuilder, Pixmap, PixmapPaint, PremultipliedColorU8,
    Rect, Transform,
};

#[repr(usize)]
#[derive(Debug, Clone, Copy)]
enum CacheIndex {
    // Static text labels (0-4)
    Welcome = 0,
    Calibrating = 1,
    Respond = 2,
    Feedback = 3,
    PracticeMode = 4,

    // Stimulus shapes (5-7)
    CircleStim = 5,
    RectStim = 6,
    ArrowStim = 7,

    // Fixation cross parts (8-9)
    FixationCross = 8,
}

impl CacheIndex {
    const STATIC_COUNT: usize = 19;
}

struct TextCache {
    font: FontRef<'static>,
    size_px: f32,
    map: HashMap<Atom, Arc<Pixmap>>,
}

impl TextCache {
    fn new(font: FontRef<'static>, size_px: f32) -> Self {
        Self {
            font,
            size_px,
            map: HashMap::new(),
        }
    }

    fn get_or_render(&mut self, atom: Atom) -> Arc<Pixmap> {
        if let Some(p) = self.map.get(&atom) {
            return Arc::clone(p);
        }
        let pm = Arc::new(render_text_pixmap(
            atom.as_ref(),
            self.size_px,
            self.font.clone(),
            Color::from_rgba8(255, 255, 255, 255),
        ));
        self.map.insert(atom, Arc::clone(&pm));
        pm
    }
}

pub fn render_text_pixmap(
    text: &str,
    font_size: f32,
    font: FontRef<'static>,
    color: Color,
) -> Pixmap {
    let scale = PxScale::from(font_size);
    let sf = font.as_scaled(scale);

    // 1) Layout with baseline at ascent
    let mut pen_x = 0.0f32;
    let mut glyphs = Vec::<Glyph>::new();
    for ch in text.chars() {
        let id = font.glyph_id(ch);
        if let Some(prev) = glyphs.last() {
            pen_x += sf.kern(prev.id, id);
        }
        glyphs.push(Glyph {
            id,
            scale,
            position: point(pen_x, sf.ascent()),
        });
        pen_x += sf.h_advance(id);
    }

    // 2) Union pixel bounds from outlined glyphs
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;

    for g in &glyphs {
        if let Some(out) = font.outline_glyph(g.clone()) {
            let b = out.px_bounds();
            min_x = min_x.min(b.min.x);
            min_y = min_y.min(b.min.y);
            max_x = max_x.max(b.max.x);
            max_y = max_y.max(b.max.y);
        }
    }

    if min_x == f32::INFINITY {
        return Pixmap::new(1, 1).expect("pixmap");
    }

    let w = (max_x.ceil() - min_x.floor()).max(1.0) as u32;
    let h = (max_y.ceil() - min_y.floor()).max(1.0) as u32;

    // 3) Create transparent, premultiplied pixmap
    let mut pm = Pixmap::new(w, h).expect("pixmap");
    let mut clear = Paint::default();
    clear.set_color(Color::from_rgba8(0, 0, 0, 0));
    pm.fill_rect(
        Rect::from_xywh(0.0, 0.0, w as f32, h as f32).unwrap(),
        &clear,
        Transform::identity(),
        None,
    );

    // 4) Rasterize with premultiplied alpha blending
    let stride = pm.width() as usize;
    let dst = pm.pixels_mut();

    // Convert desired color to straight u8
    let cu = [
        (color.red() * 255.0) as u8,
        (color.green() * 255.0) as u8,
        (color.blue() * 255.0) as u8,
        (color.alpha() * 255.0) as u8,
    ];

    for g in &glyphs {
        if let Some(out) = font.outline_glyph(g.clone()) {
            let b = out.px_bounds();
            out.draw(|x, y, cov| {
                if cov <= f32::EPSILON {
                    return;
                }
                // Map local outline coords to pixmap coords
                let fx = x as f32 + b.min.x - min_x;
                let fy = y as f32 + b.min.y - min_y;

                let ix = fx.floor() as i32;
                let iy = fy.floor() as i32;
                if ix < 0 || iy < 0 || ix >= w as i32 || iy >= h as i32 {
                    return;
                }

                let i = iy as usize * stride + ix as usize;
                if i >= dst.len() {
                    return;
                }

                // Premultiply source by (coverage * alpha)
                let a_lin = (cov * cu[3] as f32 / 255.0).clamp(0.0, 1.0);
                let sr = (cu[0] as f32 * a_lin) as u8;
                let sg = (cu[1] as f32 * a_lin) as u8;
                let sb = (cu[2] as f32 * a_lin) as u8;
                let sa = (a_lin * 255.0) as u8;

                let src = PremultipliedColorU8::from_rgba(sr, sg, sb, sa).unwrap();
                let bg = dst[i];

                // Porter-Duff over in premultiplied space: out = src + bg * (1 - src.a)
                let inv = 1.0 - (sa as f32 / 255.0);
                let r = src.red().saturating_add((bg.red() as f32 * inv) as u8);
                let g = src.green().saturating_add((bg.green() as f32 * inv) as u8);
                let b = src.blue().saturating_add((bg.blue() as f32 * inv) as u8);
                let a = src.alpha().saturating_add((bg.alpha() as f32 * inv) as u8);

                dst[i] = PremultipliedColorU8::from_rgba(r, g, b, a).unwrap();
            });
        }
    }

    pm
}

pub struct FrameStats {
    pub clear: Duration,
    pub phase: Duration,
    pub copy: Duration,
    pub total: Duration,
    pub dirty_count: usize,
}

pub trait Renderer {
    fn clear_dirty(&mut self, dirty: &[Rect]);
    fn blit_cached(&mut self, index: usize, pos: (f32, f32));
    fn blit_text_by_intern_id(&mut self, intern_id: usize, pos: (f32, f32));
}

pub trait PhaseRenderer<P: Phase>: Renderer {
    fn render_phase(
        &mut self,
        phase: &P,
        stimulus: Option<(&StimulusType, (f32, f32))>,
        trial_state: Option<&TrialState>,
        progress: Option<(usize, usize)>,
    ) -> Result<()>;
}

pub struct SkiaRenderer {
    width: u32,
    height: u32,
    center: (f32, f32),

    font: FontRef<'static>,

    static_cache: Vec<Pixmap>,
    static_sizes: Vec<(u32, u32)>,
    text_cache: TextCache,

    progress_text_interns: Vec<Vec<usize>>, // [trial_count][current_trial]
    progress_text_pixmaps: Vec<Vec<Arc<Pixmap>>>,

    // Rendering state
    canvas: Pixmap,
    dirty_regions: Vec<Rect>,
    first_frame: bool,

    // Performance tracking
    component_timers: HashMap<&'static str, RefCell<HighPrecisionTimer>>,
    clear_buffer: Vec<u8>,
}

impl SkiaRenderer {
    pub fn new(width: u32, height: u32, max_trials: usize) -> Self {
        // Pre-intern all predictable text patterns
        Self::pre_intern_text_patterns(max_trials);

        let font = FontRef::try_from_slice(include_bytes!("../../assets/DejaVuSans.ttf"))
            .expect("Font load");

        let mut canvas = Pixmap::new(width, height).unwrap();
        // Make canvas opaque once so the whole pipeline stays premultiplied + memcpy.
        {
            let mut p = Paint::default();
            p.set_color(Color::from_rgba8(0, 0, 0, 255));
            let r = Rect::from_xywh(0.0, 0.0, width as f32, height as f32).unwrap();
            canvas.fill_rect(r, &p, Transform::identity(), None);
        }

        let mut renderer = SkiaRenderer {
            width,
            height,
            center: (width as f32 / 2.0, height as f32 / 2.0),
            font: font.clone(),
            static_cache: vec![Pixmap::new(1, 1).unwrap(); CacheIndex::STATIC_COUNT],
            static_sizes: vec![(1, 1); CacheIndex::STATIC_COUNT],
            text_cache: TextCache::new(font, 24.0),
            progress_text_interns: Vec::new(),
            progress_text_pixmaps: Vec::new(),
            canvas: canvas,
            dirty_regions: Vec::with_capacity(16),
            first_frame: true,
            component_timers: ["phase", "clear", "copy", "total"]
                .iter()
                .map(|&k| (k, RefCell::new(HighPrecisionTimer::new())))
                .collect(),
            clear_buffer: vec![0u8, 0, 0, 255]
                .into_iter()
                .cycle()
                .take((width * height * 4) as usize)
                .collect(),
        };

        renderer.init_cache(max_trials);
        renderer
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        // Update dimensions and center
        self.width = new_width;
        self.height = new_height;
        self.center = (new_width as f32 / 2.0, new_height as f32 / 2.0);

        // Recreate the canvas pixmap
        self.canvas = Pixmap::new(new_width, new_height).expect("Failed to resize canvas pixmap");
        self.canvas.fill(Color::from_rgba8(0, 0, 0, 255));

        // Reallocate the clear buffer to match the new size
        self.clear_buffer = vec![0u8, 0, 0, 255]
            .into_iter()
            .cycle()
            .take((new_width * new_height * 4) as usize)
            .collect();

        self.first_frame = true;
    }

    /// Pre-intern all predictable text patterns at startup
    fn pre_intern_text_patterns(max_trials: usize) {
        // Common progress patterns - pre-compute all combinations
        for trial in 0..=max_trials {
            for current in 0..=trial {
                intern_text(&format!("Trial: {}/{}", current, max_trials));
            }
        }

        // Pre-intern percentage patterns for feedback
        for pct in (0..=100).step_by(5) {
            intern_text(&format!("Accuracy: {}%", pct));
        }

        // Pre-intern common response times
        for rt in (100..=2000).step_by(50) {
            intern_text(&format!("Response time: {}ms", rt));
        }
    }

    fn init_cache(&mut self, max_trials: usize) {
        self.cache_static_text();
        self.cache_stimuli();
        self.cache_fixation();
        // Build progress lookup as intern IDs (idempotent, no new strings)
        self.precompute_progress_pixmaps(max_trials);
    }

    fn cache_static_text(&mut self) {
        let labels = [
            (CacheIndex::Welcome as usize, "WELCOME"),
            (CacheIndex::Calibrating as usize, "CALIBRATING..."),
            (CacheIndex::Respond as usize, "respond"),
            (CacheIndex::Feedback as usize, "FEEDBACK"),
            (CacheIndex::PracticeMode as usize, "PRACTICE MODE"),
        ];

        for (index, text) in labels {
            let pixmap = render_text_pixmap(
                text,
                32.0,
                self.font.clone(),
                Color::from_rgba8(255, 255, 255, 255),
            );
            self.static_sizes[index] = (pixmap.width(), pixmap.height());
            self.static_cache[index] = pixmap;
        }
    }

    fn cache_stimuli(&mut self) {
        // Circle
        let circle_pixmap = self.render_stimulus_to_pixmap(&StimulusType::Circle {
            radius: 50.0,
            color: [255, 0, 0, 255],
        });
        self.static_sizes[CacheIndex::CircleStim as usize] =
            (circle_pixmap.width(), circle_pixmap.height());
        self.static_cache[CacheIndex::CircleStim as usize] = circle_pixmap;

        // Rectangle
        let rect_pixmap = self.render_stimulus_to_pixmap(&StimulusType::Rectangle {
            width: 80.0,
            height: 60.0,
            color: [0, 255, 0, 255],
        });
        self.static_sizes[CacheIndex::RectStim as usize] =
            (rect_pixmap.width(), rect_pixmap.height());
        self.static_cache[CacheIndex::RectStim as usize] = rect_pixmap;

        // Arrow
        let arrow_pixmap = self.render_stimulus_to_pixmap(&StimulusType::Arrow {
            direction: ArrowDirection::Right,
            size: 60.0,
            color: [0, 0, 255, 255],
        });
        self.static_sizes[CacheIndex::ArrowStim as usize] =
            (arrow_pixmap.width(), arrow_pixmap.height());
        self.static_cache[CacheIndex::ArrowStim as usize] = arrow_pixmap;
    }

    fn cache_fixation(&mut self) {
        let size = 40u32; // full extent of cross
        let mut pm = Pixmap::new(size, size).unwrap();

        let mut paint = Paint::default();
        paint.anti_alias = false;
        paint.set_color(Color::from_rgba8(255, 255, 255, 255));

        // horizontal bar (40x2) centered
        let h = Rect::from_xywh(0.0, (size as f32 - 2.0) * 0.5, size as f32, 2.0).unwrap();
        pm.fill_rect(h, &paint, Transform::identity(), None);

        // vertical bar (2x40) centered
        let v = Rect::from_xywh((size as f32 - 2.0) * 0.5, 0.0, 2.0, size as f32).unwrap();
        pm.fill_rect(v, &paint, Transform::identity(), None);

        self.static_sizes[CacheIndex::FixationCross as usize] = (pm.width(), pm.height());
        self.static_cache[CacheIndex::FixationCross as usize] = pm;
    }

    fn precompute_progress_pixmaps(&mut self, max_trials: usize) {
        self.progress_text_interns = Vec::with_capacity(max_trials + 1);
        for total in 0..=max_trials {
            let mut row: Vec<usize> = Vec::with_capacity(total + 1);
            for current in 0..=total {
                let s = format!("Trial: {}/{}", current, total);
                let intern_id = intern_text(&s);
                row.push(intern_id);
            }
            self.progress_text_interns.push(row);
        }
    }

    fn render_stimulus_to_pixmap(&self, stimulus: &StimulusType) -> Pixmap {
        let (width, height) = match stimulus {
            StimulusType::Circle { radius, .. } => {
                let size = (radius * 2.0).ceil() as u32;
                (size, size)
            }
            StimulusType::Rectangle { width, height, .. } => (*width as u32, *height as u32),
            StimulusType::Arrow { size, .. } => {
                let size = (size * 2.0).ceil() as u32;
                (size, size)
            }
            _ => (100, 100),
        };

        let mut pixmap = Pixmap::new(width, height).unwrap();
        let mut paint = Paint::default();
        paint.anti_alias = false;

        match stimulus {
            StimulusType::Circle { radius, color } => {
                paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
                let mut pb = PathBuilder::new();
                pb.push_circle(*radius, *radius, *radius);
                pixmap.fill_path(
                    &pb.finish().unwrap(),
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            StimulusType::Rectangle {
                width: w,
                height: h,
                color,
            } => {
                paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
                let rect = Rect::from_xywh(0.0, 0.0, *w, *h).unwrap();
                pixmap.fill_rect(rect, &paint, Transform::identity(), None);
            }
            StimulusType::Arrow {
                direction,
                size,
                color,
            } => {
                paint.set_color(Color::from_rgba8(color[0], color[1], color[2], color[3]));
                let mut pb = PathBuilder::new();
                let cx = *size;
                let cy = *size;
                match direction {
                    ArrowDirection::Right => {
                        pb.move_to(cx + size, cy);
                        pb.line_to(cx, cy - size);
                        pb.line_to(cx, cy + size);
                        pb.close();
                    }
                    ArrowDirection::Left => {
                        pb.move_to(cx - size, cy);
                        pb.line_to(cx, cy - size);
                        pb.line_to(cx, cy + size);
                        pb.close();
                    }
                    ArrowDirection::Up => {
                        pb.move_to(cx, cy - size);
                        pb.line_to(cx - size, cy);
                        pb.line_to(cx + size, cy);
                        pb.close();
                    }
                    ArrowDirection::Down => {
                        pb.move_to(cx, cy + size);
                        pb.line_to(cx - size, cy);
                        pb.line_to(cx + size, cy);
                        pb.close();
                    }
                }
                pixmap.fill_path(
                    &pb.finish().unwrap(),
                    &paint,
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
            _ => {}
        }

        pixmap
    }

    fn clear_dirty(&mut self, dirty: &[Rect]) {
        let stride = self.width as usize * 4;
        let canvas_data = self.canvas.data_mut();

        for rect in dirty {
            let x0 = rect.x().floor().max(0.0).min(self.width as f32) as usize;
            let y0 = rect.y().floor().max(0.0).min(self.height as f32) as usize;
            let x1 = (rect.x() + rect.width()).ceil().min(self.width as f32) as usize;
            let y1 = (rect.y() + rect.height()).ceil().min(self.height as f32) as usize;
            if x1 <= x0 || y1 <= y0 {
                continue;
            }
            let row_len = (x1 - x0) * 4;
            for y in y0..y1 {
                let off = y * stride + x0 * 4;
                canvas_data[off..off + row_len]
                    .copy_from_slice(&self.clear_buffer[off..off + row_len]);
            }
        }
    }

    fn copy_dirty_region(&self, dirty: Rect, frame_buffer: &mut [u8]) {
        let (x0, y0, x1, y1) = (
            dirty.x().floor().max(0.0).min(self.width as f32) as usize,
            dirty.y().floor().max(0.0).min(self.height as f32) as usize,
            (dirty.x() + dirty.width()).ceil().min(self.width as f32) as usize,
            (dirty.y() + dirty.height()).ceil().min(self.height as f32) as usize,
        );

        if x1 <= x0 || y1 <= y0 {
            return;
        }
        let w = x1 - x0;
        let bytes = w * 4;
        let row_bytes = (self.width as usize) * 4;
        let canvas_data = self.canvas.data();

        for row in y0..y1 {
            let off = row * row_bytes + x0 * 4;
            unsafe {
                std::ptr::copy_nonoverlapping(
                    canvas_data.as_ptr().add(off),
                    frame_buffer.as_mut_ptr().add(off),
                    bytes,
                );
            }
        }
    }

    fn coalesce_dirty(rects: &mut Vec<Rect>) {
        rects.sort_by(|a, b| {
            a.y()
                .partial_cmp(&b.y())
                .unwrap()
                .then(a.x().partial_cmp(&b.x()).unwrap())
        });
        let mut out: Vec<Rect> = Vec::with_capacity(rects.len());
        for r in rects.drain(..) {
            if let Some(last) = out.last_mut() {
                let same_row =
                    (r.y() - last.y()).abs() < 1.0 && (r.height() - last.height()).abs() < 1.0;
                let touching = r.x() <= last.x() + last.width() + 1.0;
                if same_row && touching {
                    let nx = last.x().min(r.x());
                    let nx2 = (last.x() + last.width()).max(r.x() + r.width());
                    *last = Rect::from_xywh(nx, last.y(), nx2 - nx, last.height()).unwrap();
                    continue;
                }
            }
            out.push(r);
        }
        *rects = out;
    }

    pub fn render_frame<P: Phase>(
        &mut self,
        phase: &P,
        stimulus: Option<(&StimulusType, (f32, f32))>,
        trial_state: Option<&TrialState>,
        progress: Option<(usize, usize)>,
        frame_buffer: &mut [u8],
        timer: &mut HighPrecisionTimer,
    ) -> Result<FrameStats>
    where
        P: Phase,
    {
        if self.first_frame {
            self.first_frame = false;
            self.canvas.fill(Color::from_rgba8(0, 0, 0, 255));
            frame_buffer.copy_from_slice(&self.clear_buffer);
            self.dirty_regions.clear();
        }

        // 1) Extract old dirty rects
        let mut old_dirty = Vec::new();
        std::mem::swap(&mut old_dirty, &mut self.dirty_regions);

        // 2) CLEAR old regions on offscreen canvas
        let clear_slice =
            unsafe { std::slice::from_raw_parts(old_dirty.as_ptr(), old_dirty.len()) };
        let t_clear_off = {
            let t = timer.now();
            SkiaRenderer::clear_dirty(self, clear_slice);
            timer.elapsed(t)
        };

        // 4) DRAW new content
        let t_phase = {
            let t = timer.now();
            self.render_phase(phase, stimulus, trial_state, progress)?;
            timer.elapsed(t)
        };
        // 5) COPY new dirty regions to visible frame_buffer
        let ptr = self.dirty_regions.as_ptr();
        let len = self.dirty_regions.len();
        let mut present_rects = old_dirty;
        unsafe {
            present_rects.extend_from_slice(std::slice::from_raw_parts(ptr, len));
        }
        SkiaRenderer::coalesce_dirty(&mut present_rects);

        let t_copy = {
            let t = timer.now();
            for rect in &present_rects {
                self.copy_dirty_region(*rect, frame_buffer);
            }
            timer.elapsed(t)
        };
        let t_clear_vis = Duration::ZERO;
        // Record timings
        let total = t_clear_vis + t_clear_off + t_phase + t_copy;
        self.component_timers["phase"]
            .borrow_mut()
            .record_frame(t_phase);
        self.component_timers["clear"]
            .borrow_mut()
            .record_frame(t_clear_vis + t_clear_off);
        self.component_timers["copy"]
            .borrow_mut()
            .record_frame(t_copy);
        timer.record_frame(total);

        Ok(FrameStats {
            clear: t_clear_vis + t_clear_off,
            phase: t_phase,
            copy: t_copy,
            total,
            dirty_count: self.dirty_regions.len(),
        })
    }

    fn blit_cached_fast(&mut self, index: usize, pos: (f32, f32)) {
        if index >= self.static_cache.len() {
            return;
        }

        let pixmap = &self.static_cache[index];
        let (w_u32, h_u32) = self.static_sizes[index];
        let w = w_u32 as usize;
        let h = h_u32 as usize;

        // Compute top-left corner
        let x0 = (pos.0 - w as f32 * 0.5).floor() as i32;
        let y0 = (pos.1 - h as f32 * 0.5).floor() as i32;

        // // Skip if entirely outside canvas
        // if x0 >= self.canvas.width() as i32 || y0 >= self.canvas.height() as i32 {
        //     return;
        // }
        // if x0 + w as i32 <= 0 || y0 + h as i32 <= 0 {
        //     return;
        // }

        // Determine source and destination ranges
        let dst_x_start = x0.max(0) as usize;
        let dst_y_start = y0.max(0) as usize;
        let dst_x_end = (x0 + w as i32).min(self.canvas.width() as i32) as usize;
        let dst_y_end = (y0 + h as i32).min(self.canvas.height() as i32) as usize;

        let src_x_start = if x0 < 0 { (-x0) as usize } else { 0 };
        let src_y_start = if y0 < 0 { (-y0) as usize } else { 0 };

        let max_w = dst_x_end - dst_x_start;
        let max_h = dst_y_end - dst_y_start;

        if max_w == 0 || max_h == 0 {
            return;
        }

        let src_data = pixmap.data();
        let canvas_stride = self.canvas.width() as usize;
        let dst_data = self.canvas.data_mut();

        let pixmap_stride = pixmap.width() as usize;

        // Check if the region is fully opaque
        let mut fully_opaque = true;
        'opaque_check: for y in 0..max_h {
            let row_start = (src_y_start + y) * pixmap_stride + src_x_start;
            for x in 0..max_w {
                let alpha = src_data[(row_start + x) * 4 + 3];
                if alpha != 255 {
                    fully_opaque = false;
                    break 'opaque_check;
                }
            }
        }

        if fully_opaque {
            // Fast memcpy per row for opaque regions
            for y in 0..max_h {
                let src_row_start = (src_y_start + y) * pixmap_stride * 4 + src_x_start * 4;
                let dst_row_start = (dst_y_start + y) * canvas_stride * 4 + dst_x_start * 4;
                dst_data[dst_row_start..dst_row_start + max_w * 4]
                    .copy_from_slice(&src_data[src_row_start..src_row_start + max_w * 4]);
            }
        } else {
            // Blend per pixel (premultiplied)
            for y in 0..max_h {
                for x in 0..max_w {
                    let src_idx = ((src_y_start + y) * pixmap_stride + (src_x_start + x)) * 4;
                    let dst_idx = ((dst_y_start + y) * canvas_stride + (dst_x_start + x)) * 4;

                    let sa = src_data[src_idx + 3] as u32;
                    let sr = src_data[src_idx + 0] as u32;
                    let sg = src_data[src_idx + 1] as u32;
                    let sb = src_data[src_idx + 2] as u32;

                    let da = dst_data[dst_idx + 3] as u32;
                    let dr = dst_data[dst_idx + 0] as u32;
                    let dg = dst_data[dst_idx + 1] as u32;
                    let db = dst_data[dst_idx + 2] as u32;

                    let inv_a = 255 - sa;

                    dst_data[dst_idx + 0] = (sr + (dr * inv_a + 127) / 255) as u8;
                    dst_data[dst_idx + 1] = (sg + (dg * inv_a + 127) / 255) as u8;
                    dst_data[dst_idx + 2] = (sb + (db * inv_a + 127) / 255) as u8;
                    dst_data[dst_idx + 3] = (sa + (da * inv_a + 127) / 255) as u8;
                }
            }
        }

        self.dirty_regions.push(
            Rect::from_xywh(
                dst_x_start as f32,
                dst_y_start as f32,
                max_w as f32,
                max_h as f32,
            )
            .unwrap(),
        );
    }
}

impl Renderer for SkiaRenderer {
    fn clear_dirty(&mut self, dirty: &[Rect]) {
        SkiaRenderer::clear_dirty(self, dirty);
    }

    fn blit_cached(&mut self, index: usize, pos: (f32, f32)) {
        self.blit_cached_fast(index, pos);
    }

    fn blit_text_by_intern_id(&mut self, intern_id: usize, pos: (f32, f32)) {
        if intern_id >= text_count() {
            return;
        }

        let atom = Atom::from(get_text(intern_id).as_str());
        let pm = self.text_cache.get_or_render(atom);
        let (w, h) = (pm.width(), pm.height());
        let (cw, ch) = (self.width as usize, self.height as usize);

        let x = (pos.0 - w as f32 * 0.5) as i32;
        let y = (pos.1 - h as f32 * 0.5) as i32;

        // Cull fully off-screen
        if x + w as i32 <= 0 || y + h as i32 <= 0 || x >= cw as i32 || y >= ch as i32 {
            return;
        }

        // Clipping
        let dst_x = x.max(0) as usize;
        let dst_y = y.max(0) as usize;
        let src_x_offset = (-x).max(0) as usize;
        let src_y_offset = (-y).max(0) as usize;
        let copy_w = (w as usize - src_x_offset).min(cw - dst_x);
        let copy_h = (h as usize - src_y_offset).min(ch - dst_y);

        let src_data = pm.data();
        let dst_data = self.canvas.data_mut();
        let src_row_bytes = pm.width() as usize * 4;

        // Detect full opacity once
        let fully_opaque = {
            let mut opaque = true;
            for row in 0..copy_h {
                let start = (src_y_offset + row) * src_row_bytes + src_x_offset * 4;
                let end = start + copy_w * 4;
                if src_data[start..end].iter().step_by(4).any(|&a| a != 255) {
                    opaque = false;
                    break;
                }
            }
            opaque
        };

        // Convert rows to u32 slices for fast copying
        let src_u32 = cast_slice(src_data);
        let dst_u32 = cast_slice_mut(dst_data);

        if fully_opaque {
            for row in 0..copy_h {
                let src_row_start = (src_y_offset + row) * pm.width() as usize + src_x_offset;
                let dst_row_start = (dst_y + row) * cw + dst_x;
                dst_u32[dst_row_start..dst_row_start + copy_w]
                    .copy_from_slice(&src_u32[src_row_start..src_row_start + copy_w]);
            }
        } else {
            // Branch-free alpha blending
            for row in 0..copy_h {
                let src_row_start = (src_y_offset + row) * pm.width() as usize + src_x_offset;
                let dst_row_start = (dst_y + row) * cw + dst_x;

                for i in 0..copy_w {
                    let s = src_u32[src_row_start + i];
                    let d = dst_u32[dst_row_start + i];

                    let sa = ((s >> 24) as u32 & 0xFF) as u32;
                    let inv = 255 - sa;

                    // Premultiplied blending
                    let sr = (s & 0xFF) as u32;
                    let sg = ((s >> 8) as u32 & 0xFF) as u32;
                    let sb = ((s >> 16) as u32 & 0xFF) as u32;

                    let dr = (d & 0xFF) as u32;
                    let dg = ((d >> 8) as u32 & 0xFF) as u32;
                    let db = ((d >> 16) as u32 & 0xFF) as u32;
                    let da = ((d >> 24) as u32 & 0xFF) as u32;

                    let r = sr + (dr * inv + 127) / 255;
                    let g = sg + (dg * inv + 127) / 255;
                    let b = sb + (db * inv + 127) / 255;
                    let a = sa + (da * inv + 127) / 255;

                    dst_u32[dst_row_start + i] = (a << 24) | (b << 16) | (g << 8) | r;
                }
            }
        }

        self.dirty_regions.push(
            Rect::from_xywh(dst_x as f32, dst_y as f32, copy_w as f32, copy_h as f32).unwrap(),
        );
    }
}

impl<P> PhaseRenderer<P> for SkiaRenderer
where
    P: Phase,
{
    fn render_phase(
        &mut self,
        phase: &P,
        stimulus: Option<(&StimulusType, (f32, f32))>,
        trial_state: Option<&TrialState>,
        progress: Option<(usize, usize)>,
    ) -> Result<()> {
        match phase {
            p if p.is_welcome() => {
                self.blit_cached(CacheIndex::Welcome as usize, self.center);
            }
            p if p.requires_calibration() => {
                self.blit_cached(CacheIndex::Calibrating as usize, self.center);
            }
            p if p.is_practice() || p.is_experiment() => {
                if let Some(state) = trial_state {
                    match state {
                        TrialState::Fixation => {
                            self.blit_cached(CacheIndex::FixationCross as usize, self.center);
                        }
                        TrialState::Stimulus | TrialState::Response => {
                            if let Some((s, pos)) = stimulus {
                                if let Some(cache_idx) = match s {
                                    StimulusType::Circle { .. } => {
                                        Some(CacheIndex::CircleStim as usize)
                                    }
                                    StimulusType::Rectangle { .. } => {
                                        Some(CacheIndex::RectStim as usize)
                                    }
                                    StimulusType::Arrow { .. } => {
                                        Some(CacheIndex::ArrowStim as usize)
                                    }
                                    other => panic!(
                                        "unexpected StimType passed to render phase: {:?}",
                                        other
                                    ),
                                } {
                                    self.blit_cached(cache_idx, pos);
                                }
                            }
                            if *state == TrialState::Response {
                                self.blit_cached(
                                    CacheIndex::Respond as usize,
                                    (self.center.0, self.center.1 + 100.0),
                                );
                            }
                        }
                        TrialState::Feedback => {
                            self.blit_cached(CacheIndex::Feedback as usize, self.center);
                        }
                        TrialState::Complete => {
                            // Blank inter-trial interval
                        }
                    }
                    if let Some((current, total)) = progress {
                        if let Some(intern_id) = self
                            .progress_text_interns
                            .get(total)
                            .and_then(|row| row.get(current))
                        {
                            let pos = (50.0, 30.0); // same fixed offset as before
                            self.blit_text_by_intern_id(*intern_id, pos);
                        }
                    }
                }

                if p.is_practice() {
                    self.blit_cached(
                        CacheIndex::PracticeMode as usize,
                        (self.center.0 - 100.0, 30.0),
                    );
                }
            }
            _ => {}
        }
        Ok(())
    }
}
