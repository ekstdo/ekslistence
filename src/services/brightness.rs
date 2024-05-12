use std::{fs, path::{Path, PathBuf}, process::Command, sync::Arc};

use tokio::sync::{broadcast, RwLock};

use super::utils::{self, async_file_watcher};


#[derive(Debug, Clone)]
pub struct BrightnessData {
    pub screen_value: f64,
    pub max: f64,
}

#[derive(Debug, Clone)]
pub struct BrightnessSender {
    pub changed: broadcast::Sender<Arc<RwLock<BrightnessData>>>,
    pub screen_value: broadcast::Sender<Arc<RwLock<BrightnessData>>>
}

impl BrightnessSender {
    fn new() -> Self {
        Self {
            changed: broadcast::channel(30).0,
            screen_value: broadcast::channel(30).0,
        }
    }
}

#[derive(Debug)]
pub struct BrightnessService {
    pub data: Arc<RwLock<BrightnessData>>,
    pub watcher: notify::RecommendedWatcher,
    pub sender: BrightnessSender
}

#[derive(Debug)]
pub enum BacklightError{
    BacklightNotFoundError(std::io::Error),
    BrightnessCtlNotInstalled(std::io::Error),
    FileWatchError(notify::Error),
    NotANumberError(std::io::Error),
}

impl BrightnessService {
    pub async fn new() -> Result<Arc<RwLock<BrightnessService>>, BacklightError> {
        let max = BrightnessService::run_to_int(Command::new("brightnessctl").arg("max"))?;
        let (watcher, mut rx) = async_file_watcher(BrightnessService::get_path()?.as_ref())
            .await
            .map_err(BacklightError::FileWatchError)?;

        let service = Arc::new(RwLock::new(BrightnessService {
            data: Arc::new(RwLock::new(BrightnessData {
                screen_value: 0.,
                max: max as f64
            })),
            watcher,
            sender: BrightnessSender::new()
        }));

        {
            let service = service.clone();
            tokio::spawn(async move {
                while let Some(_res) = rx.recv().await {
                    let mut writer = service.write().await;
                    writer.handle_event().await;
                }
            });
        }

        Ok(service)
    }

    fn get_path() -> Result<PathBuf, BacklightError> {
        let dir = fs::read_dir("/sys/class/backlight/")
            .map_err(BacklightError::BacklightNotFoundError)?;
        let f = dir.into_iter()
            .next()
            .ok_or(BacklightError::BacklightNotFoundError(
                std::io::Error::new(std::io::ErrorKind::NotFound, String::from("no backlight file"))
            ))?
            .map_err(BacklightError::BacklightNotFoundError)?;
        let mut path_buf = f.path();
        path_buf.push("brightness");
        Ok(path_buf)
    }

    pub fn set_screen_value(&mut self, mut new_value: f64) {
        new_value = if new_value < 0. { 0. } else if new_value > 1. { 1. } else { new_value };
        Command::new("brightnessctl")
            .arg("set")
            .arg(format!("{}%", new_value * 100.))
            .arg("-q")
            .spawn()
            .expect("Failed to call brightnessctl");
        // filewatcher does the rest
    }

    fn run_to_int(c: &mut Command) -> Result<i64, BacklightError> {

        utils::exec_for_int(c).map_err(|e| match e.kind() {
            std::io::ErrorKind::InvalidData => BacklightError::NotANumberError,
            _ => BacklightError::BrightnessCtlNotInstalled
        }(e))
    }

    async fn handle_event(&mut self) -> Result<(), BacklightError> {
        let value = BrightnessService::run_to_int(Command::new("brightnessctl").arg("get"))? as f64 / self.data.read().await.max;
        self.update_screen_value(value).await;
        Ok(())
    }

    update!(update_screen_value, screen_value, f64);
}
