use std::env;
use std::path::Path;
use std::io;
use std::sync::Arc;

use tokio::net::{unix::SocketAddr, UnixListener, UnixStream};
use tokio::io::Interest;
use tokio::sync::RwLock;

pub struct HyprlandService {
    instance_signature: String,
    xdg_runtime_dir: String,
}


impl HyprlandService {
    pub async fn new() -> std::io::Result<Arc<RwLock<Self>>> {

        let res = Arc::new(RwLock::new(HyprlandService {
            instance_signature: env::var("HYPRLAND_INSTANCE_SIGNATURE")
                .expect("Hyprland is not running (HYPRLAND_INSTANCE_SIGNATURE not set)"),
            xdg_runtime_dir: env::var("XDG_RUNTIME_DIR").unwrap_or(String::from("/"))
        }));

        {
            let res = res.clone();
            let r = res.read().await;

            let stream = r.connection("socket2").await?;

            tokio::spawn(async move {
                loop {
                    let ready = stream.ready(Interest::READABLE).await;
                    if ready.is_err() {
                        return
                    }
                    let ready = ready.unwrap();
                    if ready.is_readable() {
                        let mut data = vec![0; 1024];
                        // Try to read data, this may still fail with `WouldBlock`
                        // if the readiness event is a false positive.
                        match stream.try_read(&mut data) {
                            Ok(n) if n > 0 => {
                                println!("read {} bytes", n);
                                let x = String::from_utf8_lossy(&data[..n]);
                                println!("x: {:?}", x);
                            }
                            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                                continue;
                            }
                            Err(e) => {
                                break;
                                // return Err(e.into());
                            }
                            _ => continue
                        }

                    }
                }
            });
        }


        Ok(res)
    }

    pub async fn connection(&self, socket: &str) -> std::io::Result<UnixStream> {
        let sock_fp = |folder: &String| format!("{folder}/hypr/{}/.{socket}.sock", self.instance_signature);
        let mut path_name = sock_fp(&self.xdg_runtime_dir);
        let mut path = Path::new(&path_name);
        if !path.exists() {
            path_name = sock_fp(&"/tmp".into());
            path = Path::new(&path_name);
        }
        let stream = UnixStream::connect(path).await?;
        Ok(stream)

                        // if ready.is_writable() {
                        //     // Try to write data, this may still fail with `WouldBlock`
                        //     // if the readiness event is a false positive.
                        //     match stream.try_write(b"hello world") {
                        //         Ok(n) => {
                        //             println!("write {} bytes", n);
                        //         }
                        //         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                        //             continue;
                        //         }
                        //         Err(e) => {
                        //             return Err(e.into());
                        //         }
                        //     }
                        // }
                    // }
                // }
                // Err(e) => { /* connection failed */ }
            // }

    }

    pub async fn message_async(&self, _cmd: &String) -> std::io::Result<String> {
        let stream = self.connection("socket").await?;

        stream.writable().await?;
        Ok(String::from("hi"))
    }
}
