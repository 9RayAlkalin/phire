phire::tl_file!("library");

use super::{FlcPage, NextPage, Page, SharedState};
use crate::{
    charts_view::{ChartDisplayItem, ChartsView, NEED_UPDATE},
    client::{Chart, Client},
    get_data,
    icons::Icons,
    popup::Popup,
    rate::RateDialog,
    scene::{ChartOrder, ORDERS},
    tags::TagsDialog,
};
use anyhow::{anyhow, Result};
use macroquad::prelude::*;
use phire::{
    ext::{semi_black, JoinToString, RectExt, SafeTexture, ScaleType},
    scene::{request_file, request_input, return_input, show_error, show_message, take_input, NextScene},
    task::Task,
    ui::{button_hit, DRectButton, RectButton, Ui},
};
use std::{
    any::Any,
    borrow::Cow,
    ops::Deref,
    sync::{atomic::Ordering, Arc},
};
use tap::Tap;

const PAGE_NUM: u64 = 28;

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChartListType {
    Local,
    Ranked,
    Special,
    Unstable,
    Popular,
}

type OnlineTaskResult = (Vec<ChartDisplayItem>, Vec<Chart>, u64);
type OnlineTask = Task<Result<OnlineTaskResult>>;

pub struct LibraryPage {
    charts_view: ChartsView,

    current_page: u64,
    online_total_page: u64,
    prev_page_btn: DRectButton,
    next_page_btn: DRectButton,

    online_task: Option<OnlineTask>,

    icons: Arc<Icons>,

    btn_flc: DRectButton,
    import_btn: DRectButton,

    next_page: Option<NextPage>,

    search_btn: DRectButton,
    search_str: String,
    search_clr_btn: RectButton,

    order_btn: DRectButton,
    order_menu: Popup,
    need_show_order_menu: bool,
    current_order: usize,

    filter_btn: DRectButton,
    tags: TagsDialog,
    tags_last_show: bool,
    rating: RateDialog,
    rating_last_show: bool,
    filter_show_tag: bool,
}

impl LibraryPage {
    pub fn new(icons: Arc<Icons>, rank_icons: [SafeTexture; 8]) -> Result<Self> {
        NEED_UPDATE.store(true, Ordering::Relaxed);
        let icon_star = icons.star.clone();
        Ok(Self {
            charts_view: ChartsView::new(Arc::clone(&icons), rank_icons),

            current_page: 0,
            online_total_page: 0,
            prev_page_btn: DRectButton::new(),
            next_page_btn: DRectButton::new(),

            online_task: None,

            icons,

            btn_flc: DRectButton::new(),
            import_btn: DRectButton::new(),

            next_page: None,

            search_btn: DRectButton::new(),
            search_str: String::new(),
            search_clr_btn: RectButton::new(),

            order_btn: DRectButton::new(),
            order_menu: Popup::new().with_options(ChartOrder::names()),
            need_show_order_menu: false,
            current_order: 0,

            filter_btn: DRectButton::new(),
            tags: TagsDialog::new(true).tap_mut(|it| it.perms = get_data().me.as_ref().map(|it| it.perms()).unwrap_or_default()),
            tags_last_show: false,
            rating: RateDialog::new(icon_star, true).tap_mut(|it| {
                it.rate.score = 3;
                it.rate_upper.as_mut().unwrap().score = 10;
            }),
            rating_last_show: false,
            filter_show_tag: true,
        })
    }
}

impl LibraryPage {
    fn total_page(&self, s: &SharedState) -> u64 {
        if s.charts_local.is_empty() {
            0
        } else {
            (s.charts_local.len() - 1) as u64 / PAGE_NUM + 1
        }
    }

    pub fn render_charts(&mut self, ui: &mut Ui, c: Color, t: f32, r: Rect) {
        self.charts_view.render(ui, r, c.a, t);
    }

    pub fn load_online(&mut self) {
        if get_data().config.offline_mode {
            show_message(tl!("offline-mode")).error();
            return;
        }
        if get_data().me.is_none() {
            show_error(anyhow!(tl!("must-login")));
            return;
        }
        self.charts_view.reset_scroll();
        self.charts_view.clear();
        let page = self.current_page;
        let search = self.search_str.clone();
        let order = {
            let (order, mut rev) = ORDERS[self.current_order];
            let order = match order {
                ChartOrder::Default => {
                    rev ^= true;
                    "updated"
                }
                ChartOrder::Name => "name",
                ChartOrder::Rating => "rating",
            };
            if rev {
                format!("-{order}")
            } else {
                order.to_owned()
            }
        };
        let tags = self
            .tags
            .tags
            .tags()
            .iter()
            .cloned()
            .chain(self.tags.unwanted.as_ref().unwrap().tags().iter().map(|it| format!("-{it}")))
            .join(",");
        let division = self.tags.division;
        let rating_range = format!("{},{}", self.rating.rate.score as f32 / 10., self.rating.rate_upper.as_ref().unwrap().score as f32 / 10.);
        let popular = false;
        let typ = match ChartListType::Local {
            ChartListType::Ranked => 0,
            ChartListType::Special => 1,
            ChartListType::Unstable => 2,
            _ => -1,
        };
        let by_me = if self.tags.show_me {
            get_data().me.as_ref().map(|it| it.id)
        } else {
            None
        };
        let show_unreviewed = self.tags.show_unreviewed;
        let show_stabilize = self.tags.show_stabilize;
        self.online_task = Some(Task::new(async move {
            let mut q = Client::query::<Chart>();
            if popular {
                q = q.suffix("/popular");
            } else {
                q = q.search(search).order(order).tags(tags).query("rating", rating_range);
            }
            if let Some(me) = by_me {
                q = q.query("uploader", me.to_string());
            }
            if show_stabilize {
                q = q.query("stableRequest", "true");
            } else if show_unreviewed {
                q = q.query("reviewed", "false").query("stableRequest", "false");
            }
            let (remote_charts, count) = q
                .query("type", typ.to_string())
                .query("division", division)
                .page(page)
                .page_num(PAGE_NUM)
                .send()
                .await?;
            let total_page = if count == 0 { 0 } else { (count - 1) / PAGE_NUM + 1 };
            let charts: Vec<_> = remote_charts.iter().map(ChartDisplayItem::from_remote).collect();
            Ok((charts, remote_charts, total_page))
        }));
    }

    #[inline]
    fn switch_to_type(&mut self, _s: &mut SharedState, _ty: ChartListType) {}

    fn sync_local(&mut self, s: &SharedState) {
        self.charts_view.can_refresh = false;
        self.charts_view
            .set(s.t, s.charts_local.iter().map(|it| ChartDisplayItem::new(it.clone(), None)).collect());
    }
}

impl Page for LibraryPage {
    fn label(&self) -> Cow<'static, str> {
        "LIBRARY".into()
    }

    fn on_result(&mut self, res: Box<dyn Any>, s: &mut SharedState) -> Result<()> {
        let _res = match res.downcast::<bool>() {
            Err(res) => res,
            Ok(delete) => {
                self.charts_view.on_result(s.t, *delete);
                return Ok(());
            }
        };
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool> {
        let t = s.t;
        if self.order_menu.showing() {
            self.order_menu.touch(touch, t);
            return Ok(true);
        }
        if self.tags.touch(touch, t) {
            return Ok(true);
        }
        if self.rating.touch(touch, t) {
            return Ok(true);
        }
        if self.charts_view.transiting() {
            return Ok(true);
        }
        if false && self.online_task.is_none() {
            if self.prev_page_btn.touch(touch, t) {
                if self.current_page != 0 {
                    self.current_page -= 1;
                    self.load_online();
                }
                return Ok(true);
            }
            if self.next_page_btn.touch(touch, t) {
                if self.current_page + 1 < self.total_page(s) {
                    self.current_page += 1;
                    self.load_online();
                }
                return Ok(true);
            }
        }
        if self.charts_view.touch(touch, t, s.rt)? {
            return Ok(true);
        }
        {
            if self.btn_flc.touch(touch, t) {
                self.next_page = Some(NextPage::Overlay(Box::new(FlcPage::new(Arc::clone(&self.icons)))));
                return Ok(true);
            }
            if self.import_btn.touch(touch, t) {
                request_file("_import");
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn update(&mut self, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        self.tags.update(t);
        self.rating.update(t);
        if self.tags.show_rating {
            self.tags.show_rating = false;
            self.filter_show_tag = false;
            self.rating.enter(t);
        } else if self.tags_last_show && !self.tags.showing() {
            self.current_page = 0;
            self.load_online();
        }
        if self.rating.show_tags {
            self.rating.show_tags = false;
            self.filter_show_tag = true;
            self.tags.enter(t);
        } else if self.rating_last_show && !self.rating.showing() {
            self.current_page = 0;
            self.load_online();
        }
        self.tags_last_show = self.tags.showing();
        self.rating_last_show = self.rating.showing();
        if let Some(task) = &mut self.online_task {
            if let Some(res) = task.take() {
                match res {
                    Err(err) => show_error(err.context(tl!("failed-to-load-online"))),
                    Ok(res) => {
                        self.online_total_page = res.2;
                        self.charts_view.set(t, res.0);
                    }
                }
                self.online_task = None;
            }
        }
        self.order_menu.update(t);
        for chart in &mut s.charts_local {
            chart.illu.settle(t);
        }
        if self.charts_view.update(t)? {
            self.load_online();
        }
        if self.charts_view.need_update() {
            s.reload_local_charts();
            self.sync_local(s);
        }
        if let Some((id, text)) = take_input() {
            if id == "search" {
                self.search_str = text;
                self.current_page = 0;
                self.load_online();
            } else {
                return_input(id, text);
            }
        }
        if self.order_menu.changed() {
            self.current_order = self.order_menu.selected();
            self.current_page = 0;
            self.load_online();
        }
        Ok(())
    }

    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()> {
        let t = s.t;
        let sr = ui.screen_rect();
        let mut r = Rect::new(sr.x + 0.03, sr.y + 0.15, sr.w - 0.06, sr.h - 0.18);
        s.render_fader(ui, |ui, c| {
            let w = 0.24;
            let gap = 0.02;
            self.btn_flc.render_text(ui, Rect::new(r.right() - w * 2. - gap, -ui.top + 0.04, w, r.y + ui.top - 0.06), t, c.a, "FLC 活动", 0.6, false);
            self.import_btn.render_text(ui, Rect::new(r.right() - w, -ui.top + 0.04, w, r.y + ui.top - 0.06), t, c.a, tl!("import"), 0.6, false);
        });
        s.fader.render(ui, t, |ui, c| {
            let path = r.rounded(0.00);
            ui.fill_path(&path, semi_black(0.4 * c.a));
            self.render_charts(ui, c, s.t, r.feather(-0.01));
        });

        self.charts_view.render_top(ui, t);
        self.order_menu.render(ui, t, 1.);
        self.tags.render(ui, t);
        self.rating.render(ui, t);
        Ok(())
    }

    fn next_page(&mut self) -> NextPage {
        self.next_page.take().unwrap_or_default()
    }

    fn next_scene(&mut self, _s: &mut SharedState) -> NextScene {
        self.charts_view.next_scene().unwrap_or_default()
    }
}
