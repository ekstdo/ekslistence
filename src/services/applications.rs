use std::{collections::HashMap, ffi::OsStr, io::Write, path::PathBuf, process::Command, sync::Arc};

use tokio::sync::{broadcast::{Sender, self}, mpsc::UnboundedSender, RwLock};

use notify::{Watcher, RecursiveMode};
use super::utils::{PathGetter, async_file_watcher};

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum ApplicationType {
    Application,
    Link,
    Service,
    // Directory isn't an application
}

impl Default for ApplicationType {
    fn default() -> Self {
        ApplicationType::Application
    }
}


#[derive(Debug, Clone, Default, Eq)]
pub struct Application {
    pub name: String,
    pub description: Option<String>,
    pub executable: String,
    pub desktop: PathBuf,
    pub icon_name: Option<String>, 
    pub startup_wm_class: Option<String>,
    pub frequency: usize,
    pub type_: ApplicationType,
    pub terminal: bool,
    pub categories: Option<String>
}

#[derive(Debug)]
pub enum ApplicationError {
    DesktopfileNotFound,
    NoName,
    NoExec,
    IniParse(std::io::Error),
    IniMissing(String),
    WrongType,
    CacheFileError(std::io::Error),
    HomeFolderNotFound,
    Hidden, // not really an error, but more a return type
    FileWatchError(notify::Error),
}

// Convert from Desktop file to application
impl TryFrom<PathBuf> for Application {
    type Error = ApplicationError;
    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        if !value.exists() || value.extension() != Some(&OsStr::new("desktop")) {
            return Err(ApplicationError::DesktopfileNotFound)
        }
        let entry = freedesktop_entry_parser::parse_entry(&value)
            .map_err(ApplicationError::IniParse)?;
        if !entry.has_section("Desktop Entry") {
            return Err(ApplicationError::IniMissing("Desktop Entry".to_string()));
        }
        let desktop = entry.section("Desktop Entry");

        let description = desktop.attr("GenericName").map(String::from);

        let (type_, executable) = match desktop.attr("Type") {
            Some("Application") => (ApplicationType::Application, desktop.attr("Exec").map(String::from).ok_or(ApplicationError::NoExec)?),
            Some("Link") => (ApplicationType::Link, desktop.attr("URL").map(String::from).ok_or(ApplicationError::NoExec)?),
            Some("Service") => (ApplicationType::Service, desktop.attr("Exec").map(String::from).ok_or(ApplicationError::NoExec)?),
            _ => return Err(ApplicationError::WrongType),
        };

        if desktop.attr("Hidden") == Some("true") || desktop.attr("NoDisplay") == Some("true") {
            return Err(ApplicationError::Hidden);
        }

        Ok(Application {
            description,
            desktop: value,
            name: desktop.attr("Name").map(String::from).ok_or(ApplicationError::NoName)?,
            executable,
            icon_name: desktop.attr("Icon").map(String::from),
            startup_wm_class: desktop.attr("StartupWMClass").map(String::from),
            type_,
            terminal: desktop.attr("Terminal") == Some("true"),
            categories: desktop.attr("Categories").map(String::from),
            ..Default::default()
        })
    }
}

impl Ord for Application {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match self.frequency.cmp(&other.frequency) {
            std::cmp::Ordering::Equal => self.name.cmp(&other.name),
            a => a
        }
    }
}

impl PartialOrd for Application {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Application {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == std::cmp::Ordering::Equal
    }
}

impl Application {
    pub fn match_(&self, pat: &str) -> bool {
        self.name.matches(pat).next().is_some() ||
        self.description.as_ref().map_or(false, |x| x.matches(pat).next().is_some()) ||
        self.executable.matches(pat).next().is_some() ||
        self.desktop.to_str().unwrap().matches(pat).next().is_some() 
    }

    pub fn launch(&mut self) -> std::io::Result<()> {
        // requires dex to launch the file
        Command::new("dex").arg(&self.desktop).spawn()?;
        self.frequency += 1;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct ApplicationsData {
    pub apps: Vec<Application>,
}

impl ApplicationsData {
    pub fn new() -> Result<ApplicationsData, ApplicationError> {
        let mut a: ApplicationsData = PathGetter::desktop_files()
            .map_err(|_| ApplicationError::HomeFolderNotFound)?
            .collect::<Vec<PathBuf>>()
            .into();
        if ApplicationsData::get_path().map_err(ApplicationError::CacheFileError)?.exists() {
            a.load_frequencies().map_err(ApplicationError::CacheFileError)?;
        } else {
            a.save_frequencies().map_err(ApplicationError::CacheFileError)?;
        }
        Ok(a)
    }

    pub fn query<'a>(&'a self, term: &str) -> Vec<&'a Application> {
        let mut apps = self.apps.iter().filter(|x| x.match_(term)).collect::<Vec<_>>();
        apps.sort();
        apps
    }

    pub fn get_frequencies<'a>(&'a self) -> HashMap<&'a String, usize> {
        let mut h = HashMap::new();
        for i in &self.apps {
            h.insert(&i.name, i.frequency);
        }
        h
    }

    pub fn save_frequencies(&self) -> std::io::Result<()> {
        let file = std::fs::File::create(ApplicationsData::get_path()?)?;
        let mut writer = std::io::BufWriter::new(file);
        serde_json::to_writer(&mut writer, &self.get_frequencies())?;
        writer.flush()?;
        Ok(())
    }

    pub fn load_frequencies(&mut self) -> std::io::Result<()> {
        let file = std::fs::File::open(ApplicationsData::get_path()?)?;
        let reader = std::io::BufReader::new(file);
        let u: HashMap<String, usize> = serde_json::from_reader(reader)?;
        for i in self.apps.iter_mut() {
            if u.contains_key(&i.name) {
                i.frequency = u[&i.name];
            }
        }
        Ok(())
    }

    pub fn get_path() -> std::io::Result<PathBuf> {
        let mut cache_path = PathGetter::cache().map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        cache_path.push("apps");
        std::fs::create_dir_all(&cache_path)?;
        cache_path.push("apps_frequency.json");
        if !cache_path.exists() {
        }

        Ok(cache_path)
    }

    // This operation might be slow and should probably be replaced by partial updates
    // but idk how reliable filewatcher is
    pub fn update_all(&mut self) -> Result<(), ApplicationError> {
        let freq = self.get_frequencies();
        let mut new_apps = ApplicationsData::apps_from_pathbufs(PathGetter::desktop_files().map_err(|_| ApplicationError::HomeFolderNotFound)?.collect());
        for i in new_apps.iter_mut() {
            if freq.contains_key(&i.name) {
                i.frequency = freq[&i.name];
            }
        }
        self.apps = new_apps;
        Ok(())
    }

    pub fn apps_from_pathbufs(value: Vec<PathBuf>) -> Vec<Application> {
        value
            .into_iter()
            .map(|x| x.try_into())
            .filter(|x| x.is_ok())
            .map(|x| x.unwrap())
            .collect::<Vec<_>>()
    }
}


impl From<Vec<PathBuf>> for ApplicationsData {
    fn from(value: Vec<PathBuf>) -> Self {
        Self{
            apps: ApplicationsData::apps_from_pathbufs(value)
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApplicationSender {
    pub changed: Sender<Arc<RwLock<ApplicationsData>>>
}

impl ApplicationSender {
    fn new() -> Self {
        Self {
            changed: broadcast::channel(30).0,
        }
    }
}

#[derive(Debug)]
pub struct ApplicationService {
    pub data: Arc<RwLock<ApplicationsData>>,
    pub sender: ApplicationSender,
    pub watcher: notify::RecommendedWatcher,
}


impl ApplicationService {
    pub async fn new() -> Result<Arc<RwLock<Self>>, ApplicationError> {
        let applications = ApplicationsData::new()?;
        let applications_arc = Arc::new(RwLock::new(applications));

        let paths = PathGetter::desktop_file_dirs().map_err(|_| ApplicationError::HomeFolderNotFound)?;

        let (mut watcher, mut rx) = async_file_watcher::<&std::path::Path>(paths[0].as_ref())
            .await
            .map_err(ApplicationError::FileWatchError)?;

        for i in &paths[1..] {
            watcher
                .watch(i.as_ref(), RecursiveMode::Recursive)
                .map_err(ApplicationError::FileWatchError)?;
        }

        let service = Arc::new(RwLock::new(Self {
            data: applications_arc,
            sender: ApplicationSender::new(),
            watcher
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

    async fn handle_event(&mut self) -> Result<(), ApplicationError> {
        self.data.write().await.update_all()?;
        self.data.write().await.save_frequencies().map_err(ApplicationError::CacheFileError)?;
        Ok(())
    }
}
