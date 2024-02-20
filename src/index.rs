use super::AppError;
use super::AppState;
use crate::heresphere;
use crate::jellyfin::{
    self,
    types::{BaseItemKind, LocationType},
};
use crate::AppConfig;
use color_eyre::Section;
use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use surrealdb;
use surrealdb::engine::local::Db;
use surrealdb::Surreal;
use tracing;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub(crate) struct HeresphereIndex {
    pub(crate) id: Option<surrealdb::sql::Thing>,
    pub(crate) libraries: Vec<heresphere::Library>,
    pub(crate) scan: Option<heresphere::Scan>,
    pub(crate) last_updated: chrono::DateTime<chrono::Utc>,
}

#[derive(Serialize, Debug, Clone)]
struct Binding<T: Serialize> {
    user: String,
    data: T,
}

impl HeresphereIndex {
    pub(crate) async fn prime_data(
        app: &AppState,
        host: &str,
        user_id: &str,
        token: &str,
    ) -> Result<HeresphereIndex, AppError> {
        let user = app.jellyfin.client.resume_user(&user_id, &token);
        let items = user
            .items()
            .await?
            .items
            .ok_or(AppError(eyre::eyre!("No items in BaseItemDtoQueryResult")))?;
        let library = baseitems_to_library("Library", &host, &items);
        let scan = baseitems_to_scan(&app.config.jellyfin_base_url, &token, &host, &app.config, &items);
        let videos = baseitems_to_video_cache(
            &user_id,
            &app.config.jellyfin_base_url,
            &token,
            &app.config,
            &items,
        );
        tracing::debug!(
            library_len = library.list.len(),
            scan_len = scan.scan_data.len(),
            videos_len = videos.len(),
            "Priming cache"
        );
        let index = HeresphereIndex {
            id: Some(surrealdb::sql::Thing::from(("index", user_id))),
            libraries: vec![library],
            scan: Some(scan),
            last_updated: chrono::Utc::now(),
        };
        app.db
            .query("DELETE type::thing('index', $user); INSERT INTO index $data")
            .bind(Binding {
                user: user_id.to_string(),
                data: index.clone(),
            })
            .await?
            .check()
            .with_note(|| "Inserting cache")?;
        app.db
            .query("DELETE videos:[<string> $user, '']..; INSERT INTO videos $data")
            .bind(Binding {
                user: user_id.to_string(),
                data: videos,
            })
            .await?
            .check()
            .with_note(|| "Inserting videos")?;
        Ok(index)
    }

    pub(crate) async fn prime_data_maybe(
        app: &AppState,
        host: &str,
        user_id: &str,
        token: &str,
    ) -> Result<HeresphereIndex, AppError> {
        let session: Option<HeresphereIndex> = app.db.select(("index", user_id)).await?;
        match session {
            Some(state) => {
                // Check if cache is too old
                if state.last_updated < chrono::Utc::now() - app.config.cache_lifetime {
                    tracing::info!("Cache is too old, updating");
                    HeresphereIndex::prime_data(app, host, user_id, token).await
                } else {
                    tracing::debug!("Cache is fresh");
                    Ok(state)
                }
            }
            None => {
                tracing::debug!("No cache found, creating initial cache.");
                HeresphereIndex::prime_data(app, host, user_id, token).await
            }
        }
    }

    pub(crate) async fn get_video(
        db: &Surreal<Db>,
        user_id: &str,
        video_id: &str,
    ) -> Result<VideoCache, AppError> {
        let binds = HashMap::from([("user", user_id), ("video", video_id)]);
        let resp = db
            .query("SELECT * FROM type::thing('videos', [<string> $user, $video])")
            .bind(binds)
            .await?
            .check()?
            .take(0)?;
        match resp {
            Some(video) => {
                tracing::debug!(video = ?video, "Found video");
                Ok(video)
            }
            None => Err(AppError(eyre::eyre!("No video found"))),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct VideoCache {
    id: surrealdb::sql::Thing,
    pub data: heresphere::VideoData,
    last_updated: chrono::DateTime<chrono::Utc>,
}

pub(crate) fn baseitems_to_library(
    name: &str,
    host: &str,
    items: &[jellyfin::types::BaseItemDto],
) -> heresphere::Library {
    let links = items
        .iter()
        .filter_map(|item| {
            if let Some(LocationType::Virtual) = item.location_type {
                return None;
            }
            Some(format!(
                "{}/heresphere/{}",
                host,
                item.id.expect("No id in BaseItemDto").simple().to_string()
            ))
        })
        .collect();

    heresphere::Library {
        name: name.to_string(),
        list: links,
    }
}

pub(crate) fn baseitem_to_tags(item: &jellyfin::types::BaseItemDto) -> Vec<heresphere::Tag> {
    let mut tags = vec![];
    if let Some(chapters) = &item.chapters {
        let mut previous_tag: Option<usize> = None;
        for chapter in chapters {
            tags.push(heresphere::Tag {
                name: format!(
                    "Chapter:{}",
                    chapter.name.as_ref().unwrap_or(&"Unknown".to_string())
                ),
                start: Some(chapter.start_position_ticks.unwrap_or_default() as f64 / 10000.0),
                end: Some(item.run_time_ticks.unwrap_or_default() as f64 / 10000.0),
                track: Some(0),
                ..Default::default()
            });
            if let Some(previous_tag) = previous_tag {
                tags[previous_tag].end =
                    Some(chapter.start_position_ticks.unwrap_or_default() as f64 / 10000.0);
            }
            previous_tag = Some(tags.len() - 1);
        }
    }

    if let Some(genres) = &item.genres {
        for genre in genres {
            tags.push(heresphere::Tag {
                name: format!("Genre:{}", genre),
                ..Default::default()
            });
        }
    }
    if let Some(tags_) = &item.tags {
        for tag in tags_ {
            tags.push(heresphere::Tag {
                name: format!("Tag:{}", tag),
                ..Default::default()
            });
        }
    }
    if let Some(type_) = &item.type_ {
        tags.push(heresphere::Tag {
            name: format!("Type:{}", type_.to_string()),
            ..Default::default()
        });
    }
    match item.type_.unwrap() {
        BaseItemKind::Movie => {
            if let Some(name) = &item.name {
                tags.push(heresphere::Tag {
                    name: format!("Movie:{}", name),
                    ..Default::default()
                });
            }
            if let Some(studios) = &item.studios {
                for studio in studios {
                    tags.push(heresphere::Tag {
                        name: format!(
                            "Studio:{}",
                            studio.name.as_ref().unwrap_or(&"Unknown".to_string())
                        ),
                        ..Default::default()
                    });
                }
            }
        }
        BaseItemKind::Episode => {
            if let Some(name) = &item.series_name {
                tags.push(heresphere::Tag {
                    name: format!("Series:{}", name),
                    ..Default::default()
                });
            }
            if let Some(name) = &item.series_studio {
                tags.push(heresphere::Tag {
                    name: format!("Studio:{}", name),
                    ..Default::default()
                });
            }
        }
        _ => {}
    }

    if let Some(season) = &item.season_name {
        tags.push(heresphere::Tag {
            name: format!("Season:{}", season),
            ..Default::default()
        });
    }

    if let Some(people) = &item.people {
        for person in people {
            if let Some(name) = &person.name {
                match person.type_.as_deref() {
                    Some(type_) => {
                        if let Some(role) = &person.role {
                            tags.push(heresphere::Tag {
                                name: format!("{}:{} ({})", type_, name, role),
                                ..Default::default()
                            });
                            tags.push(heresphere::Tag {
                                name: format!("{}:{}", type_, name),
                                ..Default::default()
                            });
                        } else {
                            tags.push(heresphere::Tag {
                                name: format!("{}:{}", type_, name),
                                ..Default::default()
                            });
                        }
                    }
                    None => {}
                }
            }
        }
    }
    tags
}

pub(crate) fn baseitems_to_scan(
    jf_host: &str,
    jf_token: &str,
    host: &str,
    config: &AppConfig,
    items: &[jellyfin::types::BaseItemDto],
) -> heresphere::Scan {
    let data = items
        .iter()
        .filter_map(|item| {
            if let Some(LocationType::Virtual) = item.location_type {
                return None;
            }
            let id = item.id.expect("No id in BaseItemDto").simple().to_string();
            let url = format!("{}/heresphere/{}", host, id);
            let thumb = match item.type_.unwrap() {
                BaseItemKind::Movie => format!(
                    "{}/Items/{}/Images/Backdrop?maxHeight=300&maxWidth=300&quality=90&api_key={}",
                    jf_host, id, jf_token
                ),
                _ => format!(
                    "{}/Items/{}/Images/Primary?maxHeight=300&maxWidth=300&quality=90&api_key={}",
                    jf_host, id, jf_token
                ),
            };

            Some(heresphere::ScanData {
                link: url,
                title: baseitem_to_title(item),
                date_released: baseitem_date_to_string(item.premiere_date),
                date_added: baseitem_date_to_string(item.date_created),
                duration: (item.run_time_ticks.unwrap_or_default() as f64 / 10000.0),
                rating: item.community_rating.unwrap_or_default() as f64 / 2.0, // 0-10 to 0-5
                favorites: 0,
                comments: 0,
                is_favorite: false,
                tags: baseitem_to_tags(item),

                thumbnail_image: thumb,
                projection: "perspective".to_string(),
                stereo: "mono".to_string(),
                media: baseitem_to_media(jf_host, jf_token, item),
                subtitles: Some(baseitem_to_subtitles(item, jf_host, jf_token, None)),
            })
        })
        .collect();

    heresphere::Scan { scan_data: data }
}

pub(crate) fn baseitem_date_to_string(date: Option<chrono::DateTime<chrono::Utc>>) -> String {
    date.unwrap_or_default().format("%Y-%m-%d").to_string()
}

pub(crate) fn baseitem_to_media(
    jf_host: &str,
    jf_token: &str,
    item: &jellyfin::types::BaseItemDto,
) -> Vec<heresphere::Media> {
    let mut media = vec![];
    if let Some(files) = &item.media_sources {
        for file in files {
            let url = format!(
                "{}/Items/{}/Download?api_key={}",
                jf_host,
                &file.id.as_ref().expect("No id in MediaSourceInfo"),
                jf_token
            );
            media.push(heresphere::Media {
                name: file.container.clone().unwrap_or("some mp4".to_string()),
                sources: vec![heresphere::MediaSource {
                    url: url,
                    ..Default::default()
                }],
            });
        }
    }
    media
}

pub(crate) fn baseitem_to_title(item: &jellyfin::types::BaseItemDto) -> String {
    match item.type_.unwrap() {
        BaseItemKind::Episode => {
            let season = item.parent_index_number.clone().unwrap_or_default();
            let episode = item.index_number.clone().unwrap_or_default();
            let title = item.name.clone().unwrap_or_default();
            format!("S{:02}E{:02} - {}", season, episode, title)
        }
        _ => item.name.clone().unwrap_or_default(),
    }
}

fn baseitem_to_subtitles(item: &jellyfin::types::BaseItemDto, jf_host: &str, jf_token: &str, prefered_subtitles_language: Option<&str>) -> Vec<heresphere::Subtitle> {
    let mut subtitles = vec![];
    if let Some(media_sources) = &item.media_sources {
        for media_source in media_sources {
            if let Some(media_stream) = &media_source.media_streams {
                for stream in media_stream {
                    match stream.type_ {
                        Some(jellyfin::types::MediaStreamType::Subtitle) => {
                            if let Some(is_text) = stream.is_text_subtitle_stream {
                                if !is_text {
                                    continue;
                                }
                            }

                            let language = stream.language.clone().unwrap_or_default();
                            if let Some(prefered_subtitles_language) = prefered_subtitles_language {
                                if language != prefered_subtitles_language {
                                    continue;
                                }
                            }
                            // {host}/Videos/{routeItemId}/{routeMediaSourceId}/Subtitles/{routeIndex}/Stream.{routeFormat}?api_key={routeApiKey}
                            let url = format!(
                                "{}/Videos/{}/{}/Subtitles/{}/Stream.{}?api_key={}",
                                jf_host,
                                item.id.expect("No id in BaseItemDto").simple().to_string(),
                                media_source.id.as_ref().expect("No id in MediaSourceInfo"),
                                stream.index.unwrap_or_default(),
                                stream.codec.clone().unwrap_or_default(),
                                jf_token
                            );
                            subtitles.push(heresphere::Subtitle {
                                language: language.clone(),
                                name: stream.display_title.clone().unwrap_or(language),
                                url,
                            });
                        },
                        _ => {}
                    }
                }
            }
        }
    }
    subtitles
}

fn baseitems_to_video_cache(
    user_id: &str,
    jf_host: &str,
    jf_token: &str,
    config: &AppConfig,
    items: &[jellyfin::types::BaseItemDto],
) -> Vec<VideoCache> {
    items
        .iter()
        .filter_map(|item| {
            if let Some(LocationType::Virtual) = item.location_type {
                return None;
            }
            let id = item.id.expect("No id in BaseItemDto").simple().to_string();
            let thumb = match item.type_.unwrap() {
                BaseItemKind::Movie => format!(
                    "{}/Items/{}/Images/Backdrop?maxHeight=300&maxWidth=300&quality=90&api_key={}",
                    jf_host, id, jf_token
                ),
                _ => format!(
                    "{}/Items/{}/Images/Primary?maxHeight=300&maxWidth=300&quality=90&api_key={}",
                    jf_host, id, jf_token
                ),
            };

            let data = heresphere::VideoData {
                access: 1,
                title: baseitem_to_title(item),
                description: item.overview.clone().unwrap_or_default(),
                thumbnail_image: thumb,
                date_released: baseitem_date_to_string(item.premiere_date),
                date_added: baseitem_date_to_string(item.date_created),
                duration: (item.run_time_ticks.unwrap_or_default() as f64 / 10000.0),
                rating: item.community_rating.unwrap_or_default() as f64 / 2.0, // 0-10 to 0-5
                is_favorite: false,
                projection: "perspective".to_string(),
                stereo: "mono".to_string(),
                tags: baseitem_to_tags(item),
                media: baseitem_to_media(jf_host, jf_token, item),
                write_hsp: true,
                event_server: None,
                subtitles: baseitem_to_subtitles(item, jf_host, jf_token, config.prefered_subtitles_language.as_deref()),
            };
            Some(VideoCache {
                id: surrealdb::sql::Thing::from((
                    "videos",
                    surrealdb::sql::Id::from(vec![user_id, &id]),
                )),
                data,
                last_updated: chrono::Utc::now(),
            })
        })
        .collect()
}
