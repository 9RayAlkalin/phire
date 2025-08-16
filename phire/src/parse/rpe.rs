crate::tl_file!("parser" ptl);

use super::{process_lines};
use crate::{
    core::{
        Anim, AnimFloat, AnimVector, BpmList, Chart, ChartSettings, ClampedTween, GifFrames, HitSoundMap,
        JudgeLine, JudgeLineCache, Keyframe, Note, NoteKind, Object, StaticTween, Triple, Tweenable, EPS,
        HEIGHT_RATIO,
    },
    ext::NotNanExt,
    fs::FileSystem,
    judge::{HitSound, JudgeStatus}
};
use anyhow::{Context, Result};
use macroquad::prelude::Color;
use ordered_float::NotNan;
use sasa::AudioClip;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, rc::Rc, str::FromStr};

pub const RPE_WIDTH: f32 = 1350.;
pub const RPE_HEIGHT: f32 = 900.;
const SPEED_RATIO: f32 = 10. / 45. / HEIGHT_RATIO;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPEBpmItem {
    bpm: f32,
    start_time: Triple,
}

// serde is weird...
fn f32_zero() -> f32 {
    0.
}

fn f32_one() -> f32 {
    1.
}

fn i32_one() -> i32 {
    1
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPEEvent<T = f32> {
    start: T,
    end: T,
    start_time: Triple,
    end_time: Triple,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPESpeedEvent {
    start_time: Triple,
    end_time: Triple,
    start: f32,
    end: f32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPEEventLayer {
    alpha_events: Option<Vec<RPEEvent>>,
    move_x_events: Option<Vec<RPEEvent>>,
    move_y_events: Option<Vec<RPEEvent>>,
    rotate_events: Option<Vec<RPEEvent>>,
    speed_events: Option<Vec<RPESpeedEvent>>,
}

#[derive(Clone, Deserialize, Serialize)]
pub struct RGBColor(u8, u8, u8);

impl Default for RGBColor {
    fn default() -> Self {
        Self(255, 255, 255)
    }
}

impl From<RGBColor> for Color {
    fn from(RGBColor(r, g, b): RGBColor) -> Self {
        Self::from_rgba(r, g, b, 255)
    }
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPENote {
    // TODO above == 0? what does that even mean?
    #[serde(rename = "type")]
    kind: u8,
    above: u8,
    start_time: Triple,
    end_time: Triple,
    position_x: f32,
    y_offset: f32,
    alpha: u16,               // some alpha has 256...
    hitsound: Option<String>, // TODO implement this feature
    size: f32,
    speed: f32,
    is_fake: u8,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPEJudgeLine {
    // TODO group
    #[serde(rename = "Name")]
    name: String,
    #[serde(default="f32_one", rename = "bpmfactor")]
    bpm_factor: f32,
    event_layers: Vec<Option<RPEEventLayer>>,
    notes: Option<Vec<RPENote>>,
    is_cover: u8,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPEMetadata {
    #[serde(rename = "RPEVersion")]
    #[allow(unused)] rpe_version: i32,
    offset: i32,
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RPEChart {
    #[serde(rename = "META")]
    meta: RPEMetadata,
    #[serde(rename = "BPMList")]
    bpm_list: Vec<RPEBpmItem>,
    judge_line_list: Vec<RPEJudgeLine>,
}

fn parse_events<T: Tweenable, V: Clone + Into<T>>(
    r: &mut BpmList,
    rpe: &[RPEEvent<V>],
    default: Option<T>,
) -> Result<Anim<T>> {
    let mut kfs = Vec::new();
    if let Some(default) = default {
        if !rpe.is_empty() && rpe[0].start_time.beats() != 0.0 {
            kfs.push(Keyframe::new(0.0, default, 0));
        }
    }
    for e in rpe {
        kfs.push(Keyframe {
            time: r.time(&e.start_time),
            value: e.start.clone().into(),
            tween: StaticTween::get_rc(2),
        });
        kfs.push(Keyframe::new(r.time(&e.end_time), e.end.clone().into(), 0));
    }
    Ok(Anim::new(kfs))
}

fn parse_speed_events(r: &mut BpmList, rpe: &[RPEEventLayer], max_time: f32) -> Result<AnimFloat> {
    let rpe: Vec<&Vec<RPESpeedEvent>> = rpe.iter().filter_map(|it| it.speed_events.as_ref()).collect();
    if rpe.is_empty() {
        // TODO or is it?
        return Ok(AnimFloat::default());
    };
    let anis: Vec<AnimFloat> = rpe
        .into_iter()
        .map(|it| {
            let mut kfs = Vec::new();
            for e in it {
                let start_beats = e.start_time.beats();
                let end_beats = e.end_time.beats();
                kfs.push(Keyframe::new(r.time_beats(start_beats), e.start, 2));
                kfs.push(Keyframe::new(r.time_beats(end_beats), e.end, 0));
            }
            AnimFloat::new(kfs)
        })
        .collect();
    let mut pts: Vec<NotNan<f32>> = anis.iter().flat_map(|it| it.keyframes.iter().map(|it| it.time.not_nan())).collect();
    pts.push(max_time.not_nan());
    pts.sort();
    pts.dedup();
    let mut sani = AnimFloat::chain(anis);
    sani.map_value(|v| v * SPEED_RATIO);
    for i in 0..(pts.len() - 1) {
        let now_time = *pts[i];
        let next_time = *pts[i + 1];
        sani.set_time(now_time);
        let speed = sani.now();
        sani.set_time(next_time);
        sani.set_previous();
        let end_speed = sani.now();
        if speed.signum() * end_speed.signum() < 0. {
            pts.push(f32::tween(&now_time, &next_time, speed / (speed - end_speed)).not_nan());
        }
    }
    pts.sort();
    pts.dedup();
    let mut kfs = Vec::new();
    let mut height = 0.0;
    for i in 0..(pts.len() - 1) {
        let now_time = *pts[i];
        let next_time = *pts[i + 1];
        sani.set_time(now_time);
        let speed = sani.now();
        // this can affect a lot! do not use end_time...
        // using end_time causes Hold tween (x |-> 0) to be recognized as Linear tween (x |-> x)
        sani.set_time(next_time);
        sani.set_previous();
        let end_speed = sani.now();
        kfs.push(if (speed - end_speed).abs() < EPS {
            Keyframe::new(now_time, height, 2)
        } else if speed.abs() > end_speed.abs() {
            Keyframe {
                time: now_time,
                value: height,
                tween: Rc::new(ClampedTween::new(7 /*quadOut*/, 0.0..(1. - end_speed / speed))),
            }
        } else {
            Keyframe {
                time: now_time,
                value: height,
                tween: Rc::new(ClampedTween::new(6 /*quadIn*/, (speed / end_speed)..1.)),
            }
        });
        height += (speed + end_speed) * (next_time - now_time) / 2.;
    }
    kfs.push(Keyframe::new(max_time, height, 0));
    Ok(AnimFloat::new(kfs))
}

fn parse_gif_events<V: Clone + Into<f32>>(r: &mut BpmList, rpe: &[RPEEvent<V>], gif: &GifFrames) -> Result<AnimFloat> {
    let mut kfs = Vec::new();
    kfs.push(Keyframe::new(0.0, 0.0, 2));
    let mut next_rep_time: u128 = 0;
    for e in rpe {
        while r.time(&e.start_time) > next_rep_time as f32 / 1000. {
            kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 1.0, 0));
            kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 0.0, 2));
            next_rep_time += gif.total_time();
        }
        let stop_prog = 1. - (next_rep_time as f32 - r.time(&e.start_time) * 1000.) / gif.total_time() as f32;
        kfs.push(Keyframe::new(r.time(&e.start_time), stop_prog, 0));
        kfs.push(Keyframe {
            time: r.time(&e.start_time),
            value: e.start.clone().into(),
            tween: StaticTween::get_rc(2),
        });
        kfs.push(Keyframe::new(r.time(&e.end_time), e.end.clone().into(), 2));
        next_rep_time = (r.time(&e.end_time) * 1000. + gif.total_time() as f32 * (1. - e.end.clone().into())).round() as u128;
    }

    // TODO maybe a better approach?
    const GIF_MAX_TIME: f32 = 2000.;
    while GIF_MAX_TIME > next_rep_time as f32 / 1000. {
        kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 1.0, 0));
        kfs.push(Keyframe::new(next_rep_time as f32 / 1000., 0.0, 2));
        next_rep_time += gif.total_time();
    }
    Ok(Anim::new(kfs))
}

async fn parse_notes(
    r: &mut BpmList,
    rpe: Vec<RPENote>,
    fs: &mut dyn FileSystem,
    height: &mut AnimFloat,
    hitsounds: &mut HitSoundMap,
) -> Result<Vec<Note>> {
    let mut notes = Vec::new();
    for note in rpe {
        let time: f32 = r.time(&note.start_time);
        height.set_time(time);
        let note_height = height.now();
        let y_offset = note.y_offset * 2. / RPE_HEIGHT * note.speed;
        let kind = match note.kind {
            1 => NoteKind::Click,
            2 => {
                let end_time = r.time(&note.end_time);
                height.set_time(end_time);
                NoteKind::Hold {
                    end_time,
                    end_height: height.now(),
                    end_speed: None,
                }
            }
            3 => NoteKind::Flick,
            4 => NoteKind::Drag,
            _ => ptl!(bail "unknown-note-type", "type" => note.kind),
        };
        let hitsound = match note.hitsound {
            Some(s) => {
                // TODO: RPE doc needed...
                if s == "flick.mp3" {
                    HitSound::Flick
                } else if s == "tap.mp3" {
                    HitSound::Click
                } else if s == "drag.mp3" {
                    HitSound::Drag
                } else {
                    if hitsounds.get(&s).is_none() {
                        let data = fs.load_file(&s).await;
                        if let Ok(data) = data {
                            hitsounds.insert(s.clone(), AudioClip::new(data)?);
                        } else {
                            ptl!(bail "hitsound-missing", "name" => s);
                        }
                    }
                    HitSound::Custom(String::from_str(&s)?)
                }
            }
            None => HitSound::default_from_kind(&kind),
        };
        notes.push(Note {
            object: Object {
                alpha: if note.alpha >= 255 {
                        AnimFloat::default()
                    } else {
                        AnimFloat::fixed(note.alpha as f32 / 255.)
                    },
                translation: AnimVector(AnimFloat::fixed(note.position_x / (RPE_WIDTH / 2.)), AnimFloat::fixed(y_offset)),
                scale: if note.size == 1.0 {
                    AnimVector::default()
                } else {
                    AnimVector(AnimFloat::fixed(note.size), AnimFloat::fixed(note.size))
                },
                rotation: AnimFloat::default(),
            },
            kind,
            hitsound,
            time,
            height: note_height,
            speed: note.speed,

            above: note.above == 1,
            multiple_hint: false,
            fake: note.is_fake != 0,
            judge: JudgeStatus::NotJudged,
            protected: false,
        })
    }
    Ok(notes)
}

async fn parse_judge_line(
    bpm_list: Vec<RPEBpmItem>,
    rpe: RPEJudgeLine,
    max_time: f32,
    fs: &mut dyn FileSystem,
    hitsounds: &mut HitSoundMap,
) -> Result<JudgeLine> {
    let event_layers: Vec<_> = rpe.event_layers.into_iter().flatten().collect();
    let r = &mut BpmList::new(bpm_list.into_iter().map(|it| (it.start_time.beats(), it.bpm / rpe.bpm_factor)).collect());

    fn events_with_factor(
        r: &mut BpmList,
        event_layers: &[RPEEventLayer],
        get: impl Fn(&RPEEventLayer) -> &Option<Vec<RPEEvent>>,
        factor: f32,
        desc: &str,
    ) -> Result<AnimFloat> {
        let anis: Vec<_> = event_layers
            .iter()
            .filter_map(|it| get(it).as_ref().map(|es| parse_events(r, es, None)))
            .collect::<Result<_>>()
            .with_context(|| ptl!("type-events-parse-failed", "type" => desc))?;
        let mut res = AnimFloat::chain(anis);
        res.map_value(|v| v * factor);
        Ok(res)
    }
    let mut height = parse_speed_events(r, &event_layers, max_time)?;
    let mut notes = parse_notes(r, rpe.notes.unwrap_or_default(), fs, &mut height, hitsounds).await?;
    let cache = JudgeLineCache::new(&mut notes);
    Ok(JudgeLine {
        object: Object {
            alpha: events_with_factor(r, &event_layers, |it| &it.alpha_events, 1. / 255., "alpha")?,
            rotation: events_with_factor(r, &event_layers, |it| &it.rotate_events, -1., "rotate")?,
            translation: AnimVector(
                events_with_factor(r, &event_layers, |it| &it.move_x_events, 2. / RPE_WIDTH, "move X")?,
                events_with_factor(r, &event_layers, |it| &it.move_y_events, 2. / RPE_HEIGHT, "move Y")?,
            ),
            scale: AnimVector::default(),
        },
        height,
        notes,
        show_below: rpe.is_cover != 1,

        cache,
    })
}

pub async fn parse_rpe(source: &str, fs: &mut dyn FileSystem) -> Result<Chart> {
    let rpe: RPEChart = serde_json::from_str(source).with_context(|| ptl!("json-parse-failed"))?;
    let bpm_list = rpe.bpm_list;
    let mut r = BpmList::new(bpm_list.clone().into_iter().map(|it| (it.start_time.beats(), it.bpm)).collect());
    fn vec<T>(v: &Option<Vec<T>>) -> impl Iterator<Item = &T> {
        v.iter().flat_map(|it| it.iter())
    }
    let mut hitsounds = HashMap::new();
    #[rustfmt::skip]
    let max_time = *rpe
        .judge_line_list
        .iter()
        .map(|line| {
            line.notes.as_ref().map(|notes| {
                notes
                    .iter()
                    .map(|note| r.time(&note.end_time).not_nan())
                    .max()
                    .unwrap_or_default()
            }).unwrap_or_default().max(
                line.event_layers.iter().filter_map(|it| it.as_ref().map(|layer| {
                    vec(&layer.alpha_events)
                        .chain(vec(&layer.move_x_events))
                        .chain(vec(&layer.move_y_events))
                        .chain(vec(&layer.rotate_events))
                        .map(|it| r.time(&it.end_time).not_nan())
                        .max().unwrap_or_default()
                })).max().unwrap_or_default()
            )
        })
        .max().unwrap_or_default() + 1.;
    // don't want to add a whole crate for a mere join_all...
    let mut lines = Vec::new();
    for (id, line) in rpe.judge_line_list.into_iter().enumerate() {
        let name = line.name.clone();
        lines.push(
            parse_judge_line(bpm_list.clone(), line, max_time, fs, &mut hitsounds)
                .await
                .with_context(move || ptl!("judge-line-location-name", "jlid" => id, "name" => name))?,
        );
    }
    process_lines(&mut lines);
    Ok(Chart::new(rpe.meta.offset as f32 / 1000.0, lines, r, ChartSettings::default()))
}
