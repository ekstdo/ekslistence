use std::sync::Arc;

use log::debug;
use bluer::{self, Adapter, AdapterEvent};
use tokio::sync::{broadcast::{channel, error::SendError, Sender}, RwLock};
use tokio_stream::StreamExt;

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
#[repr(u8)]
pub enum BlueToothState {
    Absent = 0, On = 1, TurningOn = 2, TurningOff = 3, Off = 4
}

impl BlueToothState {
    pub fn enabled(&self) -> bool {
        self == &BlueToothState::On || self == &BlueToothState::TurningOn
    }
}
#[derive(Debug)]
pub struct BlueToothData {
    pub devices: Vec<bluer::Device>,
    pub state: BlueToothState
}

#[derive(Debug)]
pub struct BlueToothSender {
    pub devices: Sender<Arc<RwLock<BlueToothData>>>,
    pub state: Sender<Arc<RwLock<BlueToothData>>>,
    pub changed: Sender<Arc<RwLock<BlueToothData>>>,
}

impl BlueToothSender {
    pub fn new() -> Self {
        Self {
            devices: channel(30).0,
            state: channel(30).0,
            changed: channel(30).0
        }
    }
}

#[derive(Debug)]
pub struct BlueToothService {
    pub data: Arc<RwLock<BlueToothData>>,
    pub adapter: Adapter,
    pub sender: BlueToothSender
}

impl BlueToothService {
    pub async fn new() -> Result<Arc<RwLock<Self>>, bluer::Error> {
        let session = bluer::Session::new().await?;
        let adapter = session.default_adapter().await?;
        let bts = Arc::new(RwLock::new(BlueToothService {
            data: Arc::new(RwLock::new(BlueToothData { 
                devices: Vec::new(),
                state: BlueToothState::Absent,
            })),
            adapter,
            sender: BlueToothSender::new()
        }));
        {
            let adapter = session.default_adapter().await?;
            let bts = bts.clone();
            tokio::spawn(async move  {
                let mut stream = adapter.events().await.unwrap();
                while let Some(adapter_event) = stream.next().await {
                    let data = bts.read().await.data.clone();
                    bts.write().await.handle_event(&adapter, adapter_event, data).await;
                }
            });
        }
        Ok(bts)
    }

    pub async fn handle_event(&mut self, ad: &Adapter, ev: AdapterEvent, data: Arc<RwLock<BlueToothData>>) -> Result<(), bluer::Error> {
        match ev {
            bluer::AdapterEvent::PropertyChanged(bluer::AdapterProperty::Powered(p)) => {
                self.data.write().await.state = if p { BlueToothState::On } else { BlueToothState::Off };
                if p {
                    self.data.write().await.devices = BlueToothService::get_devices(ad).await?;
                    BlueToothService::handle_send(self.sender.devices.send(data.clone()), "bluetooth devices");
                }
                BlueToothService::handle_send(self.sender.state.send(data.clone()), "bluetooth state");
            },
            _ => {
                self.data.write().await.devices = BlueToothService::get_devices(ad).await?;
                BlueToothService::handle_send(self.sender.devices.send(data.clone()), "bluetooth devices");
            }
        }
        BlueToothService::handle_send(self.sender.changed.send(data), "bluetooth");
        Ok(())
    }

    pub async fn get_devices(ad: &Adapter) -> Result<Vec<bluer::Device>, bluer::Error> {
        if !ad.is_powered().await? {
            Ok(Vec::new())
        } else {
            ad.device_addresses()
                .await?
                .into_iter()
                .map(|x| ad.device(x))
                .collect::<Result<Vec<bluer::Device>, bluer::Error>>()
        }
    }

    fn handle_send<T>(result: Result<usize, SendError<T>>, tag: &str) {
        match result {
            Ok(i) => {debug!("message by [{}] got {} receivers", tag, i);}
            Err(_) => {debug!("No receivers for [{}]", tag);}
        };
    }


}

