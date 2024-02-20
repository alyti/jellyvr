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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct ScanData {
    pub link: String,
    pub title: String,
    pub date_released: String,
    pub date_added: String,
    pub duration: f64,
    pub rating: f64,
    pub favorites: i32,
    pub comments: i32,
    pub is_favorite: bool,
    pub tags: Vec<Tag>,
    pub thumbnail_image: String,
    pub media: Vec<Media>,
    pub projection: String,
    pub stereo: String,
    pub subtitles: Option<Vec<Subtitle>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all(serialize = "camelCase", deserialize = "camelCase"))]
pub struct VideoData {
    pub access: i32,
    pub title: String,
    pub description: String,
    pub thumbnail_image: String,
    // pub thumbnail_video: Option<String>,
    pub date_released: String,
    pub date_added: String,
    pub duration: f64,
    pub rating: f64,
    // pub favorites: i32,
    // pub comments: i32,
    pub is_favorite: bool,
    pub projection: String,
    pub stereo: String,
    // pub is_eye_swapped: bool,
    // pub fov: f64,
    // pub lens: String,
    // pub camera_ipd: f64,
    // pub hsp: Option<String>,
    pub event_server: Option<String>,
    // pub scripts: Vec<Script>,
    pub subtitles: Vec<Subtitle>,
    pub tags: Vec<Tag>,
    pub media: Vec<Media>,
    // pub write_favorite: bool,
    // pub write_rating: bool,
    // pub write_tags: bool,
    pub write_hsp: bool
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Script {
    pub name: String,
    pub url: String,
    pub rating: f64
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
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
