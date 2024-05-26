use std::{borrow::Cow, env::VarError, path::Path, process::Command};

use log::{info, warn, debug};
use tokio::sync::{mpsc};
use notify::{Watcher, RecursiveMode};
use tokio::runtime::Handle;


macro_rules! update {
    ($method_name:ident, $i:ident, $t:ty) => {
        async fn $method_name (&mut self, $i: $t) {
            let val = {
                let rl = self.data.read().await;
                rl.$i.clone()
            };
            if $i == val {
                return
            }
            self.data.write().await.$i = $i;
            match self.sender.$i.send(self.data.clone()) {
                Ok(_) => return,
                Err(_) => {debug!(target: stringify!($method_name), "No receiver");}
            }
        }
    }
}

pub async fn async_file_watcher<P: AsRef<Path>>(path: P) -> Result<(notify::RecommendedWatcher, mpsc::Receiver<notify::event::Event>), notify::Error> {
    let handle = Handle::current();
    // channel to transport from notify to tokio
    let (tx, rx) = mpsc::channel(1);

    // Automatically select the best implementation for your platform.
    let mut watcher = notify::RecommendedWatcher::new(move |res| {
        match res {
           Ok(event) => {
               handle.block_on(async {
                   let _ = tx.send(event).await;
               });
           },
           Err(e) => println!("watch error: {:?}", e),
        }
    }, notify::Config::default())?;
    watcher.watch(path.as_ref(), RecursiveMode::Recursive)?;

    Ok((watcher, rx)) // keep the watcher alive, freeing it will end the watch loop
}

pub fn exec_for_int(c: &mut Command) -> Result<i64, std::io::Error> {
    let output = c.output()?;
    let binding = String::from_utf8_lossy(&output.stdout);
    let output = binding
        .split_whitespace()
        .next()
        .ok_or(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a number"))?;

    output
        .parse::<i64>()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}

pub fn exec_for_ints(c: &mut Command) -> Result<Vec<i64>, std::io::Error> {
    let output = c.output()?;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|x| x.split(' ')
             .next()
             .ok_or(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a number"))
             .and_then(|x| x
                       .parse::<i64>()
                       .map_err(|e|std::io::Error::new(std::io::ErrorKind::InvalidData, e)))
        ).collect::<std::io::Result<Vec<i64>>>()
}


macro_rules! assign_path {
    ($method_name:ident, $path:expr, $alt:expr) => {
        pub fn $method_name() -> Result<std::path::PathBuf, shellexpand::LookupError<VarError>> {
            shellexpand::env($path)
                      .or(shellexpand::env($alt))
                      .map(|x| x.to_string().into())
        }
    };
}

macro_rules! assign_paths {
    ($method_name:ident, $path:expr, $alt:expr) => {
        pub fn $method_name() -> Result<Vec<std::path::PathBuf>, shellexpand::LookupError<VarError>> {
            shellexpand::env($path)
                      .or(shellexpand::env($alt))
                      .map(|x| std::env::split_paths(&x.to_string()).collect())
        }
    };
}

pub mod PathGetter {
    use std::{ffi::OsStr, path::PathBuf};

    use walkdir::WalkDir;

    use super::*;
    assign_path!(home, "$HOME/", "$HOME/");
    assign_path!(cache, "$XDG_CACHE_HOME/", "$HOME/.cache/");
    assign_path!(config, "$XDG_CONFIG_HOME/", "$HOME/.config/");
    assign_path!(data, "$XDG_DATA_HOME/", "$HOME/.local/share/");
    assign_path!(state, "$XDG_STATE_HOME/", "$HOME/.local/state/");
    assign_path!(runtime, "$XDG_RUNTIME_DIR/", "$HOME/.cache/0700/");
    assign_paths!(data_dirs, "$XDG_DATA_DIRS/", "/usr/local/share/:/usr/share/");
    assign_paths!(config_dirs, "$XDG_CONFIG_DIRS/", "/etc/xdg");

    pub fn desktop_file_dirs() -> Result<Vec<std::path::PathBuf>, shellexpand::LookupError<VarError>> {
        let mut result = Vec::new();
        result.push(config()?);
        result.extend(config_dirs()?);
        result.push({
            let mut d = data()?;
            d.push("applications");
            d
        });
        result.extend(data_dirs()?.into_iter().map(|mut x| {
            x.push("applications");
            x
        }));
        result.push("/usr/share/xsessions".to_owned().into());
        result.push("/etc/xdg/autostart".to_owned().into());
        result.push("/var/lib/snapd/desktop/applications".to_owned().into());
        result.push("/var/lib/flatpak/exports/share/".to_owned().into());
        result.push({
            let mut d = home()?;
            d.push(".local");
            d.push("share");
            d.push("flatpak");
            d.push("exports");
            d.push("share");
            d
        });
        result.dedup();
        Ok(result.into_iter().filter(|x| x.exists()).collect())
    }

    pub fn desktop_files() -> Result<impl Iterator<Item=PathBuf>, shellexpand::LookupError<VarError>> {
        Ok(desktop_file_dirs()?
            .into_iter()
            .map(|x| WalkDir::new(x)
                 .into_iter()
                 .filter(|y| y.is_ok())
                 .map(|y| y.unwrap().into_path())
                 .filter(|y| y.extension() == Some(&OsStr::new("desktop")))
            )
            .flatten())
    }

}


// pub fn exec_for_str(cmd: &mut Command) -> Result<String, std::io::Error> {
//     cmd.output()?.stdout
// }
