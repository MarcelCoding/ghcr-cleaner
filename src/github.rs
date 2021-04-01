use std::time::Duration;

use futures::future::try_join_all;
use reqwest::Client;
use serde::Deserialize;
use tokio::time::sleep;

const USER_AGENT: &str = "ghcr-cleaner <https://github.com/MarcelCoding/ghrc-cleaner>";

pub struct GitHub {
    client: Client,
    user: User,
}

struct User {
    name: String,
    token: String,
}

#[derive(Deserialize, Debug)]
pub struct Version {
    id: i32,
    pub name: String,
    pub html_url: String,
    pub metadata: VersionContainerMetadata,
}

#[derive(Deserialize, Debug)]
pub struct VersionContainerMetadata {
    pub container: VersionContainer,
}

#[derive(Deserialize, Debug)]
pub struct VersionContainer {
    pub tags: Vec<String>,
}

pub struct Image {
    account: Option<String>,
    name: String,
}

impl Image {
    pub fn new(name: String) -> Option<Image> {
        let image_input: Vec<&str> = name.splitn(2, '/').collect();

        match &image_input.len() {
            1 => Some(Image {
                account: None,
                name: image_input[0].to_string(),
            }),
            2 => Some(Image {
                account: Some(image_input[0].to_string()),
                name: image_input[1].to_string(),
            }),
            _ => None,
        }
    }
}

impl GitHub {
    pub fn new(username: String, token: String) -> reqwest::Result<Self> {
        let timeout = Duration::from_secs(10);

        let client = Client::builder()
            .connect_timeout(timeout)
            .https_only(true)
            .timeout(timeout)
            .user_agent(USER_AGENT)
            .pool_max_idle_per_host(3)
            .build()?;

        Ok(GitHub {
            client,
            user: User {
                name: username,
                token,
            },
        })
    }

    pub async fn fetch_versions(&self, image: &Image) -> reqwest::Result<Vec<Version>> {
        let mut versions: Vec<Version> = vec![];
        let mut page = 1;

        loop {
            let mut fetched_versions = self.fetch_version_page(image, page, 75).await?;
            let len = fetched_versions.len();

            versions.append(&mut fetched_versions);

            if len == 0 {
                return Ok(versions);
            } else {
                page += 1;
            }
        }
    }

    async fn fetch_version_page(
        &self,
        image: &Image,
        page: i32,
        page_size: i32,
    ) -> reqwest::Result<Vec<Version>> {
        let url = match &image.account {
            None => format!("https://api.github.com/user/packages/container/{0}/versions?page={1}&per_page={2}", image.name, page, page_size),
            Some(org) => format!("https://api.github.com/orgs/{0}/packages/container/{1}/versions?page={2}&per_page={3}", org, image.name, page, page_size),
        };

        let request = self
            .client
            .get(&url)
            .basic_auth(&self.user.name, Some(&self.user.token))
            .build()?;

        self.client
            .execute(request)
            .await?
            .error_for_status()?
            .json()
            .await
    }

    pub async fn delete_versions(
        &self,
        image: &Image,
        versions: &[&Version],
    ) -> reqwest::Result<()> {
        try_join_all(
            versions
                .iter()
                .map(|version| self.delete_version(image, &version.id)),
        )
        .await?;
        Ok(())
    }

    async fn delete_version(&self, image: &Image, version_id: &i32) -> reqwest::Result<()> {
        let url = match &image.account {
            None => format!(
                "https://api.github.com/user/packages/container/{0}/versions/{1}",
                image.name, version_id
            ),
            Some(org) => format!(
                "https://api.github.com/orgs/{0}/packages/container/{1}/versions/{2}",
                org, image.name, version_id
            ),
        };

        let mut response;

        for _ in 0..5 {
            let request = self
                .client
                .delete(&url)
                .basic_auth(&self.user.name, Some(&self.user.token))
                .build()?;

            response = self.client.execute(request).await?;

            if response.status().is_server_error() {
                sleep(Duration::from_secs(3)).await;
            } else {
                response.error_for_status()?;
                return Ok(());
            }
        }

        Ok(())
    }
}
