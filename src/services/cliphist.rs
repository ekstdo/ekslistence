use image;
use tokio::sync::{broadcast::{channel, Sender}, RwLock};
use std::{collections::BTreeMap, env::VarError, io::{Cursor, Read, Write}, path::Path, process::Command, sync::Arc, thread};
use image::io::Reader as ImageReader;

use super::utils::{async_file_watcher, exec_for_ints};

#[derive(Debug, Clone)]
pub enum CliphistEntry {
    Text(String),
    PixelImage(image::DynamicImage),
    VectorImage(String),
    Blob(Vec<u8>)
}

impl CliphistEntry {
    pub fn as_bytes(&self) -> &[u8] {
        match self {
            CliphistEntry::Text(s) | CliphistEntry::VectorImage(s) => s.as_bytes(),
            CliphistEntry::PixelImage(s) => s.as_bytes(),
            CliphistEntry::Blob(b) => &b,
        }
    }

    pub fn clipboard(&self) -> std::io::Result<std::process::ExitStatus> {
        let mut wlcopy_child = Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        let mut stdin = wlcopy_child.stdin.take().expect("Failed to open stdin");
        let byte_repr = self.as_bytes();
        thread::scope(move |s| {
            s.spawn(move || {
                stdin.write_all(byte_repr).expect("Failed to write to stdin");
            });
        });
        wlcopy_child.wait()
    }
}

impl From<Vec<u8>> for CliphistEntry {
    fn from(value: Vec<u8>) -> Self {
        if let Ok(format) = image::guess_format(&value) { // pixel image
            match ImageReader::with_format(Cursor::new(&value), format).decode() {
                Ok(x) => CliphistEntry::PixelImage(x),
                Err(_) => CliphistEntry::Blob(value)
            }
        } else if let Ok(utf8string) = String::from_utf8(value.clone()) {
            match svg::read(&utf8string) {
                Ok(_) => CliphistEntry::VectorImage(utf8string),
                _ => CliphistEntry::Text(utf8string)
            }
        } else {
            CliphistEntry::Blob(value)
        }
    }
}

pub struct CliphistData {
    pub entries: BTreeMap<usize, CliphistEntry>,
}

pub struct CliphistSender {
    pub entries: Sender<Arc<RwLock<CliphistData>>>,
    pub changed: Sender<Arc<RwLock<CliphistData>>>
}

impl CliphistSender {
    pub fn new() -> CliphistSender {
        Self {
            entries: channel(30).0,
            changed: channel(30).0,
        }
    }
}

pub struct CliphistService {
    pub data: Arc<RwLock<CliphistData>>,
    pub sender: CliphistSender,
    pub watcher: notify::RecommendedWatcher,
    pub num_display: usize
}

pub enum CliphistError {
    DatabaseNotFound(shellexpand::LookupError<VarError>),
    WatcherError(notify::Error)
}

impl CliphistService {
    pub async fn new(num_display: usize) -> Result<Arc<RwLock<Self>>, CliphistError> {
        let path = shellexpand::env("$XDG_CACHE_HOME/cliphist/db")
            .or(shellexpand::env("$HOME/.cache/cliphist/db"))
            .map_err(CliphistError::DatabaseNotFound)?;

        let (watcher, mut rx) = async_file_watcher(Path::new(&path.into_owned()))
            .await.map_err(CliphistError::WatcherError)?;

        let service = Arc::new(RwLock::new(Self{
            data: Arc::new(RwLock::new(CliphistData {
                entries: BTreeMap::new(),
            })),
            sender: CliphistSender::new(),
            watcher,
            num_display
        }));

        {
            let mut writer = service.write().await;
            writer.handle_event().await;
        }

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

    pub fn get_i(i: usize) -> std::io::Result<std::process::ChildStdout> {
        Ok(Command::new("cliphist")
            .arg("decode")
            .arg(i.to_string())
            .stdout(std::process::Stdio::piped())
            .spawn()?
            .stdout
            .take()
            .expect("Failed to open stdout"))
    }
    pub fn copy_i(i: usize) -> std::io::Result<std::process::ExitStatus> {
        let cliphist = Self::get_i(i)?;
        let mut wlcopy_child = Command::new("wl-copy")
            .stdin(std::process::Stdio::from(cliphist))
            .spawn()?;
        wlcopy_child.wait()
    }

    pub async fn handle_event(&mut self) -> std::io::Result<()> {
        let is = exec_for_ints(Command::new("cliphist").arg("list"))?;
        if is.len() == 0 {
            let mut w = self.data.write().await;
            w.entries = BTreeMap::new();
        }

        let mut w = self.data.write().await;
        let mut changed = false;
        for i in is {
            let i = i as usize;
            if w.entries.contains_key(&i) {
                continue
            }
            changed = true;
            let bytes_vec = Self::get_i(i)?.bytes().collect::<std::io::Result<Vec<u8>>>()?;
            w.entries.insert(i, bytes_vec.into());
        }

        if changed {
            let _ = self.sender.changed.send(self.data.clone());
            let _ = self.sender.entries.send(self.data.clone());
        }

        Ok(())
    }
}
