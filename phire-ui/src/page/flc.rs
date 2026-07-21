use super::{Illustration, NextPage, Page, SharedState};
use crate::{
    data::BriefChartInfo,
    icons::Icons,
    images::Images,
    scene::SongScene,
};
use anyhow::Result;
use macroquad::prelude::*;
use phire::{
    config::Mods,
    ext::{semi_black, RectExt, BLACK_TEXTURE},
    fs,
    scene::NextScene,
    task::Task,
    ui::{DRectButton, Scroll, Ui, button_hit_large},
};
use std::{borrow::Cow, path::Path, sync::Arc};
use tokio::sync::Notify;

struct FlcChartItem {
    info: BriefChartInfo,
    dir_name: String,
    track_num: u32,
    illu: Illustration,
    btn: DRectButton,
}

pub struct FlcPage {
    icons: Arc<Icons>,
    charts: Vec<FlcChartItem>,
    scroll: Scroll,
    next_scene: Option<NextScene>,
    next_page: Option<NextPage>,
}

impl FlcPage {
    pub fn new(icons: Arc<Icons>) -> Self {
        let flc_dir = Path::new("assets/flc");
        let mut charts = Vec::new();
        if let Ok(entries) = std::fs::read_dir(flc_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_dir() {
                    continue;
                }
                let Some(dir_name) = path.file_name().and_then(|s| s.to_str()).map(|s| s.to_owned()) else {
                    continue;
                };
                if dir_name.parse::<u32>().is_err() {
                    continue;
                }
                let info_path = path.join("info.txt");
                let Ok(contents) = std::fs::read_to_string(&info_path) else {
                    continue;
                };
                let Ok(info) = fs::info_from_txt(&contents) else {
                    continue;
                };
                let yml_path = path.join("info.yml");
                if !yml_path.exists() {
                    let _ = std::fs::write(&yml_path, serde_yaml::to_string(&info).unwrap_or_default());
                }
                let img_path = path.join(&info.illustration);
                let illu = {
                    let tex = BLACK_TEXTURE.clone();
                    let notify = Arc::new(Notify::new());
                    Illustration {
                        texture: (tex.clone(), tex),
                        notify: Arc::clone(&notify),
                        task: Some(Task::new({
                            let img_path = img_path.clone();
                            async move {
                                notify.notified().await;
                                let data = std::fs::read(&img_path)?;
                                let image = image::load_from_memory(&data)?;
                                let thumbnail = Images::thumbnail(&image);
                                Ok((thumbnail, Some(image)))
                            }
                        })),
                        loaded: Arc::default(),
                        load_time: f32::NAN,
                    }
                };
                let brief_info = BriefChartInfo::from(info);
                charts.push(FlcChartItem {
                    info: brief_info,
                    dir_name,
                    track_num: 0,
                    illu,
                    btn: DRectButton::new(),
                });
            }
        }
        charts.sort_by_key(|c| c.dir_name.parse::<u32>().unwrap_or(0));
        for (i, chart) in charts.iter_mut().enumerate() {
            chart.track_num = i as u32 + 1;
        }
        Self {
            icons,
            charts,
            scroll: Scroll::new(),
            next_scene: None,
            next_page: None,
        }
    }
}

impl Page for FlcPage {
    fn label(&self) -> Cow<'static, str> {
        "FLC".into()
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        for chart in &mut self.charts {
            chart.illu.settle(s.t);
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if self.scroll.touch(touch, t) {
            return Ok(true);
        }
        if !self.scroll.contains(touch) {
            return Ok(false);
        }
        for item in &mut self.charts {
            if item.btn.touch(touch, t) {
                button_hit_large();
                let tex = BLACK_TEXTURE.clone();
                let chart_item = crate::page::ChartItem {
                    info: item.info.clone(),
                    local_path: Some(format!(":flc/{}", item.dir_name)),
                    illu: Illustration {
                        texture: (tex.clone(), tex),
                        notify: Arc::default(),
                        task: None,
                        loaded: Arc::default(),
                        load_time: f32::NAN,
                    },
                };
                let scene = SongScene::new(
                    chart_item,
                    None,
                    Some(format!(":flc/{}", item.dir_name)),
                    Arc::clone(&self.icons),
                    s.icons.clone(),
                    Mods::default(),
                );
                self.next_scene = Some(NextScene::Overlay(Box::new(scene)));
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let sr = ui.screen_rect();
        s.render_fader(ui, |ui, c| {
            let list_y = 0.06;
            let list_rect = Rect::new(sr.x + 0.03, sr.y + list_y, sr.w - 0.06, sr.h - list_y - 0.06);
            ui.scope(|ui| {
                ui.dx(list_rect.x);
                ui.dy(list_rect.y);
                self.scroll.size((list_rect.w, list_rect.h));
                self.scroll.render(ui, |ui| {
                    let row_h = 0.2;
                    let pad = 0.01;
                    for (_id, item) in self.charts.iter_mut().enumerate() {
                        item.illu.notify();
                        let y = _id as f32 * row_h;
                        let r = Rect::new(pad, y + pad, list_rect.w - pad * 2., row_h - pad * 2.);
                        let (r, path) = item.btn.render_shadow(ui, r, t, c.a, |_| semi_black(c.a));

                        ui.fill_path(&path, item.illu.shading(r, t, c.a * 0.55));
                        ui.fill_path(&path, (semi_black(0.45 * c.a), (0., 0.), semi_black(0.75 * c.a), (0., r.h)));

                        let img_w = r.h * 16. / 9.;
                        let img_r = Rect::new(r.x, r.y, img_w, r.h);
                        ui.fill_path(&img_r.rounded(0.01), semi_black(0.4 * c.a));
                        ui.fill_path(&img_r.rounded(0.01), item.illu.shading(img_r, t, c.a));
                        ui.fill_path(&img_r.rounded(0.01), (semi_black(0.15 * c.a), (0., 0.), semi_black(0.5 * c.a), (0., img_r.h)));

                        let text_x = img_r.right() + 0.02;
                        let text_w = r.right() - text_x - 0.02;

                        let level_str = item.info.level.clone();
                        let level_measure = ui.text(&level_str).size(0.45).measure();
                        let lx = r.right() - 0.02 - level_measure.w;

                        ui.text(format!("TRACK {}", item.track_num))
                            .pos(text_x, r.y + 0.02)
                            .anchor(0., 0.)
                            .size(0.32)
                            .color(Color::new(c.r, c.g, c.b, c.a * 0.5))
                            .draw();
                        ui.text(&item.info.name)
                            .pos(text_x, r.y + 0.055)
                            .anchor(0., 0.)
                            .max_width(text_w)
                            .size(0.55)
                            .color(c)
                            .draw();
                        ui.text(&level_str)
                            .pos(lx, r.y + 0.04)
                            .anchor(0., 0.)
                            .size(0.45)
                            .color(Color::new(c.r, c.g, c.b, c.a * 0.75))
                            .draw();
                        let composer_w = ui.text(&item.info.composer).size(0.35).measure().w;
                        ui.text(&item.info.composer)
                            .pos(text_x, r.bottom() - 0.025)
                            .anchor(0., 1.)
                            .size(0.35)
                            .color(Color::new(c.r, c.g, c.b, c.a * 0.45))
                            .draw();
                        if !item.info.illustrator.is_empty() {
                            ui.text(&item.info.illustrator)
                                .pos(text_x + composer_w + 0.08, r.bottom() - 0.025)
                                .anchor(0., 1.)
                                .size(0.35)
                                .color(Color::new(c.r, c.g, c.b, c.a * 0.35))
                                .draw();
                        }
                    }
                    (list_rect.w, self.charts.len() as f32 * row_h)
                });
            });
        });
        Ok(())
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }

    fn next_page(&mut self) -> NextPage {
        self.next_page.take().unwrap_or_default()
    }
}
