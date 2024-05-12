use std::{fs, path::{Path, PathBuf}, process::Command, sync::Arc};

use tokio::sync::{broadcast, mpsc, RwLock};
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
                Err(_) => {dbg!(format!("No receiver for {}", stringify!($method_name)));}
            }
        }
    }
}

pub async fn async_file_watcher(path: &Path) -> Result<(notify::RecommendedWatcher, mpsc::Receiver<notify::event::Event>), notify::Error> {
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
    watcher.watch(path, RecursiveMode::Recursive)?;

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


// pub fn exec_for_str(cmd: &mut Command) -> Result<String, std::io::Error> {
//     cmd.output()?.stdout
// }
