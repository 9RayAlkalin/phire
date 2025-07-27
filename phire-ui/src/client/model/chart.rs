use super::{File, Object, Ptr, User};
use crate::data::BriefChartInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Chart {
    pub author_name: String,
    pub date_created: DateTime<Utc>,
    pub date_file_updated: DateTime<Utc>,
    pub date_updated: DateTime<Utc>,
    pub description: String,
    pub difficulty: f32,
    pub file: String,
    pub file_checksum: String,
    pub format: i64,
    pub id: String,
    pub is_hidden: bool,
    pub is_locked: bool,
    pub is_ranked: bool,
    pub level: String,
    pub level_type: i64,
    pub like_count: i64,
    pub note_count: i64,
    pub owner_id: i64,
    pub play_count: i64,
    pub rating: f64,
    pub score: f64,
    pub song: SongDto,
    pub song_id: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChartAssetDto {
    pub chart_id: Option<String>,
    pub date_created: Option<String>,
    pub date_updated: Option<String>,
    pub file: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub owner_id: Option<i64>,
    #[serde(rename = "type")]
    pub chart_asset_dto_type: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SongDto {
    pub author_name: String,
    pub chart_levels: Vec<ChartLevelDto>,
    pub date_created: DateTime<Utc>,
    pub date_file_updated: DateTime<Utc>,
    pub date_updated: DateTime<Utc>,
    pub description: String,
    pub duration: String,
    pub file: String,
    pub file_checksum: String,
    pub id: String,
    pub illustration: String,
    pub illustrator: String,
    pub is_hidden: bool,
    pub is_locked: bool,
    pub like_count: i64,
    pub bpm: f64,
    pub offset: i64,
    pub owner_id: i64,
    pub play_count: i64,
    pub preview_end: String,
    pub preview_start: String,
    pub title: String,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TagDto {
    pub date_created: Option<String>,
    pub description: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub normalized_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ChartLevelDto {
    pub count: Option<i64>,
    pub level_type: Option<i64>,
}

impl Object for Chart {
    const QUERY_PATH: &'static str = "charts";

    fn id(&self) -> i32 {
        self.id.clone().parse().unwrap_or_default()
    }
}

impl Chart {
    pub fn to_info(&self) -> BriefChartInfo {
        BriefChartInfo {
            id: None,
            uploader: None,
            name: self.song.title.clone(),
            level: self.level.clone(),
            difficulty: self.difficulty,
            intro: self.description.clone(),
            charter: self.author_name.clone(),
            composer: self.song.clone().author_name.clone(),
            illustrator: self.song.illustrator.clone(),
            score_total: 1_000_000,
            created: None,
            updated: None,
            chart_updated: None,
        }
    }
}
