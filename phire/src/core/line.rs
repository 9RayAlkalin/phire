use super::{chart::ChartSettings, Anim, AnimFloat, BpmList, Matrix, Note, Object, Point, RenderConfig, Resource, Vector};
use crate::{
    config::Mods,
    core::NoteKind,
    ext::{parse_alpha, NotNanExt, SafeTexture},
    judge::{JudgeStatus, LIMIT_BAD},
    ui::Ui,
};
use macroquad::prelude::*;
use miniquad::{RenderPass};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum UIElement {
    Pause = 1,
    ComboNumber = 2,
    Combo = 3,
    Score = 4,
    Bar = 5,
    Name = 6,
    Level = 7,
}

impl UIElement {
    pub fn from_u8(val: u8) -> Option<Self> {
        Some(match val {
            1 => Self::Pause,
            2 => Self::ComboNumber,
            3 => Self::Combo,
            4 => Self::Score,
            5 => Self::Bar,
            6 => Self::Name,
            7 => Self::Level,
            _ => return None,
        })
    }
}

pub struct GifFrames {
    /// time of each frame in milliseconds
    frames: Vec<(u128, SafeTexture)>,
    /// milliseconds
    total_time: u128,
}

impl GifFrames {
    pub fn new(frames: Vec<(u128, SafeTexture)>) -> Self {
        let total_time = frames.iter().map(|(time, _)| *time).sum();
        Self { frames, total_time }
    }

    pub fn get_time_frame(&self, time: u128) -> &SafeTexture {
        let mut time = time % self.total_time;
        for (t, frame) in &self.frames {
            if time < *t {
                return frame;
            }
            time -= t;
        }
        &self.frames.last().unwrap().1
    }

    pub fn get_prog_frame(&self, prog: f32) -> &SafeTexture {
        let time = (prog * self.total_time as f32) as u128;
        self.get_time_frame(time)
    }

    pub fn total_time(&self) -> u128 {
        self.total_time
    }
}

#[derive(Default)]
pub enum JudgeLineKind {
    #[default]
    Normal,
    Texture(SafeTexture, String),
    TextureGif(Anim<f32>, GifFrames, String),
    Text(Anim<String>),
    Paint(Anim<f32>, RefCell<(Option<RenderPass>, bool)>),
}

#[derive(Clone)]
pub struct JudgeLineCache {
    update_order: Vec<u32>,
    above_indices: Vec<usize>,
    below_indices: Vec<usize>,
}

impl JudgeLineCache {
    pub fn new(notes: &mut Vec<Note>) -> Self {
        notes.sort_by_key(|it| {
            (
                !it.above,
                it.speed.not_nan(),
                (it.height + it.object.translation.1.now() * it.speed).not_nan(),
            )
        });
        
        let mut res = Self {
            update_order: Vec::new(),
            above_indices: Vec::new(),
            below_indices: Vec::new(),
        };
        res.reset(notes);
        res
    }

    pub(crate) fn reset(&mut self, notes: &mut Vec<Note>) {
        self.update_order = (0..notes.len() as u32).collect();
        self.above_indices.clear();
        self.below_indices.clear();
        let mut index = 0;
        while notes.get(index).map_or(false, |it| it.above) {
            self.above_indices.push(index);
            let speed = notes[index].speed;
            loop {
                index += 1;
                if !notes.get(index).map_or(false, |it| it.above && it.speed == speed) {
                    break;
                }
            }
        }
        while index != notes.len() {
            self.below_indices.push(index);
            let speed = notes[index].speed;
            loop {
                index += 1;
                if !notes.get(index).map_or(false, |it| it.speed == speed) {
                    break;
                }
            }
        }
    }
}

pub struct JudgeLine {
    pub object: Object,
    pub height: AnimFloat,
    pub notes: Vec<Note>,
    pub show_below: bool,

    pub cache: JudgeLineCache,
}

unsafe impl Sync for JudgeLine {}
unsafe impl Send for JudgeLine {}

impl JudgeLine {
    pub fn update(&mut self, res: &mut Resource, tr: Matrix, bpm_list: &mut BpmList, index: usize) {
        // self.object.set_time(res.time); // this is done by chart, chart has to calculate transform for us
        let rot = self.object.rotation.now();
        self.height.set_time(res.time);
        self.cache.update_order.retain(|id| {
            let note = &mut self.notes[*id as usize];
            note.update(res, rot, &tr, bpm_list, index);
            !note.dead()
        });

        let not_judge = |index: usize| {
            match self.notes[index].kind {
                NoteKind::Hold { end_time, .. } => {
                    matches!(self.notes[index].judge, JudgeStatus::Judged) && res.time > end_time
                },
                _ => {
                    matches!(self.notes[index].judge, JudgeStatus::Judged)
                },
            }
        };
        self.cache.above_indices.retain_mut(|index| {
            while not_judge(*index) {
                if self
                    .notes
                    .get(*index + 1)
                    .map_or(false, |it| it.above && it.speed == self.notes[*index].speed)
                {
                    *index += 1;
                } else {
                    return false;
                }
            }
            true
        });
        self.cache.below_indices.retain_mut(|index| {
            while not_judge(*index) {
                if self
                    .notes
                    .get(*index + 1)
                    .map_or(false, |it| it.speed == self.notes[*index].speed)
                {
                    *index += 1;
                } else {
                    return false;
                }
            }
            true
        });
    }

    pub fn fetch_pos(&self, res: &Resource) -> Vector {
        self.object.now_translation(res)
    }

    pub fn fetch_rotate(&self) -> Matrix {
        self.object.now_rotation()
    }

    pub fn now_transform(&self, res: &Resource) -> Matrix {
        self.fetch_rotate().append_translation(&self.fetch_pos(res))
    }

    pub fn render(&self, ui: &mut Ui, res: &mut Resource, bpm_list: &mut BpmList, settings: &ChartSettings, id: usize) {
        let alpha = self.object.now_alpha();
        res.with_model(self.now_transform(res), |res| {
            res.with_model(self.object.now_scale(), |res| {
                res.apply_model(|res| 
                    if res.config.render_line {
                        let mut color = res.judge_line_color;
                        color.a = parse_alpha(color.a * alpha.max(0.0), res.alpha, 0.15, res.config.chart_debug_line > 0.);
                        if color.a == 0.0 {
                            return;
                        }
                        let len = res.info.line_length;
                        draw_line(-len, 0., len, 0., 0.0075, color);
                    }
                )
            });
            let mut config = RenderConfig {
                settings,
                line_height: self.height.now(),
                appear_before: f32::INFINITY,
                invisible_time: f32::INFINITY,
                draw_below: self.show_below,
            };
            if res.config.has_mod(Mods::FADE_OUT) {
                config.invisible_time = LIMIT_BAD;
            }
            let mut line_set_debug_alpha = false;
            if alpha < 0.0 {
                if !settings.pe_alpha_extension {
                    if res.config.chart_debug_note > 0. {
                        line_set_debug_alpha = true;
                    } else {
                        return;
                    }
                }
                let w = (-alpha).floor() as u32;
                match w {
                    1 => {
                        if res.config.chart_debug_note > 0. {
                            line_set_debug_alpha = true;
                        } else {
                            return;
                        }
                    }
                    2 => {
                        config.draw_below = false;
                    }
                    w if (100..1000).contains(&w) => {
                        config.appear_before = (w as f32 - 100.) / 10.;
                    }
                    w if (1000..2000).contains(&w) => {
                        // TODO unsupported
                    }
                    _ => {}
                }
            }
            let (vw, vh) = (1.2 / res.config.chart_ratio, 1.0 / res.config.chart_ratio);
            let p = [
                res.screen_to_world(Point::new(-vw, -vh)),
                res.screen_to_world(Point::new(-vw, vh)),
                res.screen_to_world(Point::new(vw, -vh)),
                res.screen_to_world(Point::new(vw, vh)),
            ];
            let height_above = p[0].y.max(p[1].y.max(p[2].y.max(p[3].y)));
            let height_below = p[0].y.min(p[1].y.min(p[2].y.min(p[3].y)));
            let agg = res.config.aggressive;
            let mut height = self.height.clone();
            if res.config.note_scale > 0. && res.config.render_note {
                for index in &self.cache.above_indices {
                    let speed = self.notes[*index].speed;
                    for note in self.notes[*index..].iter() {
                        if !note.above || speed != note.speed {
                            break;
                        }
                        if matches!(note.judge, JudgeStatus::Judged) && !matches!(note.kind, NoteKind::Hold { .. }) {
                            continue;
                        }
                        if agg {
                            let line_height = match note.kind {
                                NoteKind::Hold { end_time, .. } => {
                                    let time = if res.time < end_time {
                                        res.time.min(note.time)
                                    } else {
                                        res.time
                                    };
                                    height.set_time(time);
                                    height.now()
                                }
                                _ => {
                                    config.line_height
                                }
                            };
                            let note_height = (note.height - line_height + note.object.translation.1.now()) / res.aspect_ratio * speed;
                            if note_height < height_below {
                                continue;
                            }
                            if note_height > height_above {
                                break;
                            }
                        }
                        note.render(ui, res, &mut config, bpm_list, line_set_debug_alpha, id, height_above);
                    }
                }

                res.with_model(Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
                    for index in &self.cache.below_indices {
                        let speed = self.notes[*index].speed;
                        for note in self.notes[*index..].iter() {
                            if speed != note.speed {
                                break;
                            }
                            if matches!(note.judge, JudgeStatus::Judged) && !matches!(note.kind, NoteKind::Hold { .. }) {
                                continue;
                            }
                            if agg {
                                let line_height = match note.kind {
                                    NoteKind::Hold { end_time, .. } => {
                                        let time = if res.time < end_time {
                                            res.time.min(note.time)
                                        } else {
                                            res.time
                                        };
                                        height.set_time(time);
                                        height.now()
                                    }
                                    _ => {
                                        config.line_height
                                    }
                                };
                                let note_height = (note.height - line_height + note.object.translation.1.now()) / res.aspect_ratio * speed;
                                if note_height < -height_above {
                                    continue;
                                }
                                if note_height > -height_below {
                                    break;
                                }
                            }
                            note.render(ui, res, &mut config, bpm_list, line_set_debug_alpha, id, -height_below);
                        }
                    }
                });
            }
            if res.config.chart_debug_line > 0. {
                res.with_model(Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
                    res.apply_model(|res| {
                        if !res.config.render_line { return };
                        String::new();

                        let line_height_ulp = {
                            if !config.line_height.is_nan() & !config.line_height.is_infinite() {
                                f32::EPSILON * config.line_height.abs()
                            } else {
                                0.0
                            }
                        };
                        let line_height_ulp_string = {
                                if line_height_ulp > 0.0018518519 {
                                    format!("(Speed too high! ULP: {:.4})", line_height_ulp)
                                } else {
                                    String::new()
                                }
                        };
                        let color = if line_height_ulp > 0.018518519 { // 10px error in 1080P
                            Color::new(1., 0., 0., parse_alpha(alpha, res.alpha, 0.15, res.config.chart_debug_line > 0.))
                        } else if line_height_ulp > 0.0018518519 { // 1px error in 1080P
                            Color::new(1., 1., 0., parse_alpha(alpha, res.alpha, 0.15, res.config.chart_debug_line > 0.))
                        } else {
                            Color::new(1., 1., 1., parse_alpha(alpha, res.alpha, 0.15, res.config.chart_debug_line > 0.))
                        };
                        ui.text(format!("[{}] h:{:.2}{}", id, config.line_height, line_height_ulp_string))
                        .pos(0., -res.config.chart_debug_line * 0.1)
                        .anchor(0.5, 1.)
                        .size(res.config.chart_debug_line)
                        .color(color)
                        .draw();
                    });
                });
            }
        });
    }
}
