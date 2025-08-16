crate::tl_file!("parser");

#[cfg(feature = "video")]
use super::{BpmList, JudgeLine, Matrix, Resource, UIElement, Vector};
use crate::{judge::JudgeStatus, ui::Ui};
use macroquad::prelude::*;
use sasa::AudioClip;
use std::{cell::RefCell, collections::HashMap};

#[derive(Default)]
pub struct ChartSettings {
    pub pe_alpha_extension: bool,
}

pub type HitSoundMap = HashMap<String, AudioClip>;
const PROGRESS_BAR_COLOR: Color = Color::new(0.565, 0.565, 0.565, 1.0);

pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
    pub bpm_list: RefCell<BpmList>,
    pub settings: ChartSettings,
}

impl Chart {
    pub fn new(offset: f32, lines: Vec<JudgeLine>, bpm_list: BpmList, settings: ChartSettings) -> Self {
        Self {
            offset,
            lines,
            bpm_list: RefCell::new(bpm_list),
            settings,
        }
    }

    #[inline]
    pub fn with_element<R>(&self, ui: &mut Ui, element: UIElement, f: impl FnOnce(&mut Ui, Color) -> R) -> R {
        let default_color = if matches!(element, UIElement::Bar) { PROGRESS_BAR_COLOR } else { WHITE };
        f(ui, default_color)
    }

    pub fn reset(&mut self) {
        self.lines
            .iter_mut()
            .flat_map(|it| it.notes.iter_mut())
            .for_each(|note| {
                note.judge = JudgeStatus::NotJudged;
                note.protected = false;
            });
        for line in &mut self.lines {
            line.cache.reset(&mut line.notes);
        }
    }

    pub fn update(&mut self, res: &mut Resource) {
        for line in &mut self.lines {
            line.object.set_time(res.time);
        }
        // TODO optimize
        let trs = self.lines.iter().map(|it| it.now_transform(res)).collect::<Vec<_>>();
        let mut guard = self.bpm_list.borrow_mut();
        for (index, (line, tr)) in self.lines.iter_mut().zip(trs).enumerate() {
            line.update(res, tr, &mut guard, index);
        }
    }

    pub fn render(&self, ui: &mut Ui, res: &mut Resource) {
        res.apply_model_of(&Matrix::identity().append_nonuniform_scaling(&Vector::new(if res.config.flip_x() { -1. } else { 1. }, -1.)), |res| {
            let mut guard = self.bpm_list.borrow_mut();
            for (id, line) in self.lines.iter().enumerate() {
                line.render(ui, res, &mut guard, &self.settings, id);
            }
            drop(guard);
            res.note_buffer.borrow_mut().draw_all();
            if res.config.sample_count > 1 {
                unsafe { get_internal_gl() }.flush();
                if let Some(target) = &res.chart_target {
                    target.blit();
                }
            }
        });
    }
}
