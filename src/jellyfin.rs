use progenitor::generate_api;

use self::types::DeviceProfile;

generate_api!("jellyfin-openapi-stable-models-only.json");

#[derive(Clone)]
pub struct JellyfinConfig {
    pub base_url: String,
}

impl JellyfinConfig {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url
        }
    }
}

fn emby_authorization(token: Option<&str>) -> String {
    format!(r#"MediaBrowser Client="jellyvr", Device="Unknown VR HMD", DeviceId="placeholder", Version="0.0.1"{}"#, token.map_or("".to_string(), |t| format!(r#", Token="{}""#, t)))
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
        let response: types::QuickConnectResult = self.client.get(&url).header("X-Emby-Authorization", emby_authorization(None)).send().await?.error_for_status()?.json().await?;
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
        let url = format!("{}/QuickConnect/Connect?Secret={}", self.client.config.base_url, self.secret);
        let response: types::QuickConnectResult = self.client.client.get(&url).send().await?.error_for_status()?.json().await?;
        Ok(response.authenticated.unwrap_or_default())
    }

    pub async fn auth(&self) -> Result<JellyfinUser, reqwest::Error> {
        let url = format!("{}/Users/AuthenticateWithQuickConnect", self.client.config.base_url);
        let response: types::AuthenticationResult = self.client.client.post(&url).json(&types::QuickConnectDto{
            secret: self.secret.clone()
        }).send().await?.error_for_status()?.json().await?;
        let user = JellyfinUser {
            client: self.client.clone(),
            id: response.user.as_ref().expect("No user_id in AuthenticationResult").id.expect("No id in User").to_string(),
            token: response.access_token.expect("No access_token in AuthenticationResult"),
            username: response.user.expect("No user in AuthenticationResult").name.expect("No name in User").to_string(),
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
    pub username: String
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
        let response: types::BaseItemDtoQueryResult = self.client.client.get(&url).query(query).header("X-Emby-Authorization", emby_authorization(Some(&self.token))).send().await?.error_for_status()?.json().await?;
        Ok(response)
    }
}
