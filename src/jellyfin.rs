use std::vec;

use progenitor::generate_api;
use uuid::Uuid;

use self::types::{ResponseProfile, SubtitleProfile, TranscodingProfile};

generate_api!("jellyfin-openapi-stable-models-only.json");

#[derive(Clone)]
pub struct JellyfinConfig {
    pub base_url: String,
}

impl JellyfinConfig {
    pub fn new(base_url: String) -> Self {
        Self { base_url }
    }
}

fn emby_authorization(token: Option<&str>) -> String {
    format!(
        r#"MediaBrowser Client="jellyvr", Device="Unknown VR HMD", DeviceId="placeholder", Version="0.0.1"{}"#,
        token.map_or("".to_string(), |t| format!(r#", Token="{}""#, t))
    )
}

#[derive(Clone)]
pub struct JellyfinClient {
    pub config: JellyfinConfig,
    client: reqwest::Client,
}

impl JellyfinClient {
    pub fn new(config: JellyfinConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    pub async fn new_quick_connect(&self) -> Result<QuickConnectSession, reqwest::Error> {
        let url = format!("{}/QuickConnect/Initiate", self.config.base_url);
        let response: types::QuickConnectResult = self
            .client
            .get(&url)
            .header("X-Emby-Authorization", emby_authorization(None))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(QuickConnectSession {
            client: self.clone(),
            secret: response.secret.expect("No secret in QuickConnectResult"),
            code: response.code.expect("No code in QuickConnectResult"),
        })
    }

    pub fn resume_quick_connect(&self, secret: &str, code: &str) -> QuickConnectSession {
        QuickConnectSession {
            client: self.clone(),
            secret: secret.to_string(),
            code: code.to_string(),
        }
    }

    pub fn resume_user(&self, id: &str, token: &str) -> JellyfinUser {
        JellyfinUser {
            client: self.clone(),
            id: id.to_string(),
            token: token.to_string(),
            username: "".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct QuickConnectSession {
    client: JellyfinClient,
    pub secret: String,
    pub code: String,
}

impl QuickConnectSession {
    pub async fn poll(&self) -> Result<bool, reqwest::Error> {
        let url = format!(
            "{}/QuickConnect/Connect?Secret={}",
            self.client.config.base_url, self.secret
        );
        let response: types::QuickConnectResult = self
            .client
            .client
            .get(&url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response.authenticated.unwrap_or_default())
    }

    pub async fn auth(&self) -> Result<JellyfinUser, reqwest::Error> {
        let url = format!(
            "{}/Users/AuthenticateWithQuickConnect",
            self.client.config.base_url
        );
        let response: types::AuthenticationResult = self
            .client
            .client
            .post(&url)
            .json(&types::QuickConnectDto {
                secret: self.secret.clone(),
            })
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let user = JellyfinUser {
            client: self.client.clone(),
            id: response
                .user
                .as_ref()
                .expect("No user_id in AuthenticationResult")
                .id
                .expect("No id in User")
                .to_string(),
            token: response
                .access_token
                .expect("No access_token in AuthenticationResult"),
            username: response
                .user
                .expect("No user in AuthenticationResult")
                .name
                .expect("No name in User")
                .to_string(),
        };
        let caps_url = format!("{}/Sessions/Capabilities/Full", self.client.config.base_url);
        self.client.client.post(&caps_url).json(&types::ClientCapabilitiesDto{
            // These don't actually seem to do anything at all...
            app_store_url: Some("https://github.com/alyti/jellyvr/".to_string()),
            icon_url: Some("https://raw.githubusercontent.com/alyti/jellyvr/main/assets/images/jellyfin-jellyvr-logo.png".to_string()),
            device_profile: None, //Some(DeviceProfile{}),
            message_callback_url: None,
            playable_media_types: vec!["Video".to_string()],
            supported_commands: vec![],
            supports_content_uploading: Some(false),
            supports_media_control: Some(false),
            supports_persistent_identifier: Some(false),
            supports_sync: Some(false),
        }).header("X-Emby-Authorization", emby_authorization(Some(&user.token))).send().await?.error_for_status()?;
        Ok(user)
    }
}

#[derive(Clone)]
pub struct JellyfinUser {
    client: JellyfinClient,
    pub id: String,
    pub token: String,
    pub username: String,
}

impl JellyfinUser {
    pub async fn items(&self) -> Result<types::BaseItemDtoQueryResult, reqwest::Error> {
        let url = format!("{}/Users/{}/Items", self.client.config.base_url, self.id);
        let query: &[(&str, &str)] = &[
            ("SortBy", "SortName,ProductionYear".into()),
            ("SortOrder", "Ascending".into()),
            ("IncludeItemTypes", "Movie,Episode".into()),
            ("Recursive", "true".into()),
            ("Fields", "DateCreated,MediaSources,BasicSyncInfo,Genres,Tags,Studios,SeriesStudio,People,Chapters".into()),
            ("ImageTypeLimit", "1".into()),
            ("EnableImageTypes", "Primary,Backdrop".into()),
            ("StartIndex", "0".into()),
            ("IsMissing", "false".into())
        ];
        let response: types::BaseItemDtoQueryResult = self
            .client
            .client
            .get(&url)
            .query(query)
            .header(
                "X-Emby-Authorization",
                emby_authorization(Some(&self.token)),
            )
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response)
    }

    pub async fn playback_info(
        &self,
        item: &str,
    ) -> Result<types::PlaybackInfoResponse, reqwest::Error> {
        let url = format!(
            "{}/Items/{}/PlaybackInfo",
            self.client.config.base_url, item
        );
        let response: types::PlaybackInfoResponse = self
            .client
            .client
            .get(&url)
            .query(&[("UserId", &self.id)])
            .json(&types::PlaybackInfoDto {
                user_id: Some(Uuid::parse_str(&self.id).expect("Invalid UUID")),
                allow_audio_stream_copy: None,
                allow_video_stream_copy: None,
                audio_stream_index: None,
                auto_open_live_stream: None,
                device_profile: Some(types::DeviceProfile {
                    direct_play_profiles: vec![
                        types::DirectPlayProfile{
                            container: Some("webm".to_string()),
                            type_: Some(types::DlnaProfileType::Video),
                            video_codec: Some("hevc,h264,vp8,vp9,av1".to_string()),
                            audio_codec: Some("aac,mp3,opus,flac,vorbis".to_string()),
                        },
                        types::DirectPlayProfile{
                            container: Some("mp4,m4v".to_string()),
                            type_: Some(types::DlnaProfileType::Video),
                            video_codec: Some("hevc,h264,vp8,vp9,av1".to_string()),
                            audio_codec: Some("aac,mp3,opus,flac,vorbis".to_string()),
                        },
                        types::DirectPlayProfile{
                            container: Some("mkv".to_string()),
                            type_: Some(types::DlnaProfileType::Video),
                            video_codec: Some("hevc,h264,vp8,vp9,av1".to_string()),
                            audio_codec: Some("aac,mp3,opus,flac,vorbis".to_string()),
                        },
                    ],
                    codec_profiles: vec![],
                    transcoding_profiles: vec![
                        TranscodingProfile{
                            container: Some("ts".to_string()),
                            type_: Some(types::DlnaProfileType::Video),
                            audio_codec: Some("aac,mp3,vorbis".to_string()),
                            video_codec: Some("hvec,h264".to_string()),
                            context: types::EncodingContext::Streaming,
                            protocol: Some("hls".to_string()),
                            max_audio_channels: Some("2".to_string()),
                            min_segments: 1,
                            break_on_non_key_frames: true,
                            conditions: vec![],
                            copy_timestamps: false,
                            enable_subtitles_in_manifest: false,
                            enable_mpegts_m2_ts_mode: false,
                            estimate_content_length: true,
                            segment_length: 0,
                            transcode_seek_info: types::TranscodeSeekInfo::Auto,
                    
                        }
                    ],
                    container_profiles: vec![],
                    response_profiles: vec![ResponseProfile{
                        type_: Some(types::DlnaProfileType::Video),
                        container: Some("m4v, mp4, mkv, webm".to_string()),
                        mime_type: Some("video/mp4,video/x-matroska,video/webm".to_string()),
                        audio_codec: None,
                        conditions: None,
                        org_pn: None,
                        video_codec: None,
                    }],
                    subtitle_profiles: vec![
                        SubtitleProfile{
                            format: Some("srt".into()),
                            method: Some(types::SubtitleDeliveryMethod::External),
                            didl_mode: None,
                            container: None,
                            language: None,
                        },
                        SubtitleProfile{
                            format: Some("ass".into()),
                            method: Some(types::SubtitleDeliveryMethod::External),
                            didl_mode: None,
                            container: None,
                            language: None,
                        },
                        SubtitleProfile{
                            format: Some("ssa".into()),
                            method: Some(types::SubtitleDeliveryMethod::External),
                            didl_mode: None,
                            container: None,
                            language: None,
                        },
                    ],
                    album_art_pn: None,
                    enable_album_art_in_didl: false,
                    enable_ms_media_receiver_registrar: false,
                    enable_single_album_art_limit: false,
                    enable_single_subtitle_limit: false,
                    friendly_name: Some("HereSphere (JellyVR)".to_string()),
                    id: None,
                    identification: None,
                    ignore_transcode_byte_range_requests: false,
                    manufacturer: None,
                    manufacturer_url: None,
                    max_album_art_height: None,
                    max_album_art_width: None,
                    max_icon_height: None,
                    max_icon_width: None,
                    max_static_bitrate: Some(400000000),
                    max_static_music_bitrate: None,
                    max_streaming_bitrate: Some(400000000),
                    model_description: None,
                    model_name: None,
                    model_number: None,
                    model_url: None,
                    music_streaming_transcoding_bitrate: Some(384000),
                    name: None,
                    protocol_info: None,
                    requires_plain_folders: false,
                    requires_plain_video_items: false,
                    serial_number: None,
                    sony_aggregation_flags: None,
                    supported_media_types: None,
                    timeline_offset_seconds: 0,
                    user_id: None,
                    xml_root_attributes: vec![],
                }),
                enable_direct_play: None,
                enable_direct_stream: None,
                enable_transcoding: None,
                live_stream_id: None,
                max_audio_channels: None,
                max_streaming_bitrate: None,
                media_source_id: None,
                start_time_ticks: None,
                subtitle_stream_index: None,
            })
            .header(
                "X-Emby-Authorization",
                emby_authorization(Some(&self.token)),
            )
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(response)
    }
}
