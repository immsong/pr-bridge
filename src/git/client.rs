use anyhow::Result;
use git2::Remote;
pub struct Client {}

impl Client {
    pub async fn ls_remote(url: &str) -> Result<Vec<(String, String)>> {
        let url = url.to_string();

        let task_result = tokio::task::spawn_blocking(move || {
            let mut remote = match git2::Remote::create_detached(url.as_bytes()) {
                Ok(r) => r,
                Err(e) => return Err(anyhow::Error::new(e)),
            };

            if let Err(e) = remote.connect(git2::Direction::Fetch) {
                return Err(anyhow::Error::new(e));
            }

            let list = match remote.list() {
                Ok(l) => l,
                Err(e) => return Err(anyhow::Error::new(e)),
            };

            let refs: Vec<_> = list
                .iter()
                .map(|head| (head.oid().to_string(), head.name().to_string()))
                .collect();

            Ok(refs)
        })
        .await;

        match task_result {
            Ok(result) => result,
            Err(e) => Err(anyhow::Error::new(e)),
        }
    }
}
