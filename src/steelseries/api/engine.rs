use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::steelseries) struct SubAppMetadata {
    encrypted_web_server_address: String,
    pub(in crate::steelseries) web_server_address: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubAppSecretMetadata {
    encrypted_web_server_address_cert_text: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubApp {
    pub name: String,
    pub is_enabled: bool,
    pub is_ready: bool,
    pub is_running: bool,
    pub metadata: SubAppMetadata,
    pub secret_metadata: SubAppSecretMetadata,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubAppResponseData {
    sub_apps: HashMap<String, SubApp>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CoreProps {
    encrypted_address: String,
    gg_encrypted_address: String,
    address: String,
}

impl SteelSeriesEngineClient {
    pub(crate) fn new(gg_encrypted_address: String) -> SteelSeriesEngineClient {
        Self {
            gg_encrypted_address,
        }
    }

    pub(crate) fn new_autodetect() -> SteelSeriesEngineClient {
        let default_path = PathBuf::from("C:\\ProgramData\\SteelSeries\\GG\\coreProps.json");
        let file = File::open(default_path);

        match file {
            Ok(file) => match serde_json::de::from_reader::<_, CoreProps>(BufReader::new(file)) {
                Ok(coreprops) => Self::new(coreprops.gg_encrypted_address),
                Err(err) => panic!("{}", err.to_string()),
            },
            _ => panic!("Failed to read coreProps.json"),
        }
    }
    async fn get_sub_apps_async(&self) -> SubAppResponseData {
        let response = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()
            .expect("Failed to build HTTP client for SteelSeriesEngineClient")
            .get(format!("https://{}/subApps", self.gg_encrypted_address))
            .send()
            .await;

        match response {
            Ok(res) => res
                .json::<SubAppResponseData>()
                .await
                .expect("Failed to decode subapp response data"),
            Err(e) => panic!("Failed to get subapp response data: {}", e),
        }
    }
    pub async fn get_subapp_url(&self, app_name: &str) -> Option<String> {
        let response = self.get_sub_apps_async().await;
        let app = response.sub_apps.get(app_name);
        match app {
            Some(app) => Some(app.metadata.web_server_address.clone()),
            None => None,
        }
    }
}

pub struct SteelSeriesEngineClient {
    gg_encrypted_address: String,
}
