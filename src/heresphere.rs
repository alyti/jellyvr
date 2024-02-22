use serde::{Serialize, Deserialize};
use serde_repr::{Serialize_repr, Deserialize_repr};

pub static MAGIC_HEADER: &'static str = "HereSphere-JSON-Version";

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Index {
    pub access: i32,
    pub banner: Option<Banner>,
    pub library: Vec<Library>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Banner {
    pub image: String,
    pub link: String
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Library {
    pub name: String,
    pub list: Vec<String>
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct Scan {
    pub scan_data: Vec<ScanData>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct ScanData {
    pub link: String,
    #[serde(flatten)]
    pub video: VideoData
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct VideoData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<i32>,
    pub title: String,
    pub duration: f64,
    pub media: Vec<Media>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<Tag>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub date_released: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub date_added: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub projection: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stereo: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_favorite: Option<bool>,
    pub thumbnail_image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_video: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub favorites: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_eye_swapped: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fov: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lens: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub camera_ipd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hsp: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event_server: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scripts: Option<Vec<Script>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitles: Option<Vec<Subtitle>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_favorite: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_rating: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_tags: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_hsp: Option<bool>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Script {
    pub name: String,
    pub url: String,
    pub rating: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Subtitle {
    pub name: String,
    pub language: String,
    pub url: String
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Tag {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rating: Option<f64>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct Media {
    pub name: String,
    pub sources: Vec<MediaSource>
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
/// Represents a media source with its resolution, height, width, size, and URL.
pub struct MediaSource {
    // pub resolution: Option<i32>,
    // pub height: Option<i32>,
    // pub width: Option<i32>,
    // pub size: Option<i32>,
    pub url: String
}

#[derive(Serialize_repr, Deserialize_repr, PartialEq, Debug, Clone)]
#[repr(u8)]
pub enum EventType {
    /// Event when the playback is opened.
    Open,
    /// Event when the playback is played.
    /// Seek and playback speed changes will be sent with the play event
    Play,
    /// Event when the playback is paused.
    Pause,
    /// Event when the playback is closed.
    Close,
}

/// Represents an event from heresphere.

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct Event {
    /// Login username.
    pub username: String,
    /// String with the video url used for the HereSphere API
    pub id: String,
    /// The video title.
    pub title: String,
    /// The type of event.
    pub event: EventType,
    /// The playback time in milliseconds.
    pub time: f64,
    /// The playback speed.
    pub speed: f64,
    /// UTC time in milliseconds.
    pub utc: f64,
    /// The connection key of the synchronized peripheral.
    pub connection_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct Request {
    pub username: String,
    pub password: String,

    pub is_favorite: Option<bool>,
    pub rating: Option<f64>,
    pub tags: Option<Vec<Tag>>,
    pub hsp: Option<String>,
    pub delete_file: Option<bool>,

    pub needs_media_source: Option<bool>
}
