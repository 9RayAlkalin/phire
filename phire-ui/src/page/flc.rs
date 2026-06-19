use super::{Page, SharedState};
use anyhow::Result;
use macroquad::prelude::*;
use phire::scene::NextScene;
use std::borrow::Cow;

pub struct FlcPage {
    title: String,
}

impl FlcPage {
    pub fn new() -> Self {
        Self {
            title: "FLC".to_string(),
        }
    }
}

impl Page for FlcPage {
    fn label(&self) -> Cow<'static, str> {
        "FLC".into()
    }

    fn update(&mut self, _s: &mut SharedState) -> Result<()> {
        Ok(())
    }

    fn touch(&mut self, _touch: &Touch, _s: &mut SharedState) -> Result<bool> {
        Ok(false)
    }

    fn render(&mut self, ui: &mut phire::ui::Ui, s: &mut SharedState) -> Result<()> {
        s.render_fader(ui, |ui, c| {
            ui.text(&self.title)
                .pos(0., 0.)
                .anchor(0.5, 0.5)
                .size(1.5)
                .color(c)
                .draw();
        });
        Ok(())
    }
}
