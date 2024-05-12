use std::env;
use std::path::Path;

use tokio::net::{unix::SocketAddr, UnixListener, UnixStream};

struct HyprlandService {
    instance_signature: String,
    xdg_runtime_dir: String,
}


impl HyprlandService {
    fn new() -> Self {
        HyprlandService {
            instance_signature: env::var("HYPRLAND_INSTANCE_SIGNATURE")
                .expect("Hyprland is not running (HYPRLAND_INSTANCE_SIGNATURE not set)"),
            xdg_runtime_dir: env::var("XDG_RUNTIME_DIR").unwrap_or(String::from("/"))
        }
    }

    async fn connection(&self, socket: &str) -> std::io::Result<(UnixStream, SocketAddr)> {
        let sock_fp = |folder: &String| format!("{folder}/hypr/{}/.{socket}.sock", self.instance_signature);
        let mut path_name = sock_fp(&self.xdg_runtime_dir);
        let mut path = Path::new(&path_name);
        if !path.exists() {
            path_name = sock_fp(&"/tmp".into());
            path = Path::new(&path_name);
        }
        let listener = UnixListener::bind(path)?;
        listener.accept().await
    }

    async fn message(&self, cmd: &String) -> std::io::Result<String> {
        let (stream, address) = self.connection("socket").await?;
        Ok(String::from("hi"))
    }
}
