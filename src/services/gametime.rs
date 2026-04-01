use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Cached in-game time read from the Lua mod's gametime.json.
#[derive(Debug, Clone, Serialize)]
pub struct GameTime {
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub world_age_hours: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct GameTimeFile {
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    world_age_hours: Option<f64>,
}

pub type GameTimeCache = Arc<RwLock<Option<GameTime>>>;

pub fn spawn_gametime_poller(
    data_dir: String,
    cache: GameTimeCache,
) -> tokio::task::JoinHandle<()> {
    let gametime_file = format!("{data_dir}/gametime.json");

    tracing::info!(gametime_file = %gametime_file, "gametime_poller_started");

    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(30);
        loop {
            tokio::time::sleep(interval).await;

            let path = Path::new(&gametime_file);
            if !path.exists() {
                continue;
            }

            match tokio::fs::read_to_string(path).await {
                Ok(content) => match serde_json::from_str::<GameTimeFile>(&content) {
                    Ok(gt) => {
                        *cache.write().await = Some(GameTime {
                            month: gt.month,
                            day: gt.day,
                            hour: gt.hour,
                            minute: gt.minute,
                            world_age_hours: gt.world_age_hours,
                        });
                    }
                    Err(e) => {
                        tracing::debug!(error = %e, "gametime_parse_skip");
                    }
                },
                Err(e) => {
                    tracing::debug!(error = %e, "gametime_read_error");
                }
            }
        }
    })
}
