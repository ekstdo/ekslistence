use std::sync::Arc;

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
            Ok(i) => {dbg!("message by [{}] got {} receivers", tag, i);}
            Err(_) => {dbg!("No receivers for [{}]", tag);}
        };
    }


}

// // @ts-expect-error missing types
// import GnomeBluetooth from 'gi://GnomeBluetooth?version=3.0';
// import Service from '../service.js';
// import Gio from 'gi://Gio';
// import { bulkConnect, bulkDisconnect } from '../utils.js';
// export class BluetoothDevice extends Service {
//     private _device: GnomeBluetooth.Device;
//     private _ids: number[];
//     private _connecting = false;

//     get device() { return this._device; }

//     constructor(device: GnomeBluetooth.Device) {
//         super();

//         this._device = device;
//         this._ids = [
//             'address',
//             'alias',
//             'battery-level',
//             'battery-percentage',
//             'connected',
//             'name',
//             'paired',
//             'trusted',
//         ].map(prop => device.connect(`notify::${prop}`, () => {
//             this.changed(prop);
//         }));

//         this._ids.push(device.connect('notify::icon', () => {
//             this.changed('icon-name');
//         }));
//     }

//     close() {
//         bulkDisconnect(this._device, this._ids);
//     }

//     get address() { return this._device.address; }
//     get alias() { return this._device.alias; }
//     get battery_level() { return this._device.battery_level; }
//     get battery_percentage() { return this._device.battery_percentage; }
//     get connected() { return this._device.connected; }
//     get icon_name() { return this._device.icon; }
//     get name() { return this._device.name; }
//     get paired() { return this._device.paired; }
//     get trusted() { return this._device.trusted; }
//     get type() { return GnomeBluetooth.type_to_string(this._device.type); }
//     get connecting() { return this._connecting || false; }

//     readonly setConnection = (connect: boolean) => {
//         this._connecting = true;
//         bluetooth.connectDevice(this, connect, () => {
//             this._connecting = false;
//             this.changed('connecting');
//         });
//         this.changed('connecting');
//     };
// }

// export class Bluetooth extends Service {
//     static {
//         Service.register(this, {
//             'device-added': ['string'],
//             'device-removed': ['string'],
//         }, {
//             'devices': ['jsobject'],
//             'connected-devices': ['jsobject'],
//             'enabled': ['boolean', 'rw'],
//             'state': ['string'],
//         });
//     }

//     private _client: GnomeBluetooth.Client;
//     private _devices: Map<string, BluetoothDevice>;

//     constructor() {
//         super();

//         this._devices = new Map();
//         this._client = new GnomeBluetooth.Client();
//         bulkConnect(this._client, [
//             ['device-added', this._deviceAdded.bind(this)],
//             ['device-removed', this._deviceRemoved.bind(this)],
//             ['notify::default-adapter-state', () => this.changed('state')],
//             ['notify::default-adapter-powered', () => this.changed('enabled')],
//         ]);

//         this._getDevices().forEach(device => this._deviceAdded(this, device));
//     }

//     readonly toggle = () => {
//         this._client.default_adapter_powered = !this._client.default_adapter_powered;
//     };

//     private _getDevices() {
//         const devices = [];
//         const deviceStore = this._client.get_devices();

//         for (let i = 0; i < deviceStore.get_n_items(); ++i) {
//             const device = deviceStore.get_item(i);

//             if (device.paired || device.trusted)
//                 devices.push(device);
//         }

//         return devices;
//     }

//     private _deviceAdded(_: GnomeBluetooth.Client, device: GnomeBluetooth.Device) {
//         if (this._devices.has(device.address))
//             return;

//         const d = new BluetoothDevice(device);
//         d.connect('changed', () => this.emit('changed'));
//         d.connect('notify::connected', () => this.notify('connected-devices'));
//         this._devices.set(device.address, d);
//         this.changed('devices');
//         this.emit('device-added', device.address);
//     }

//     private _deviceRemoved(_: GnomeBluetooth.Client, path: string) {
//         const device = this.devices.find(d => d.device.get_object_path() === path);
//         if (!device || !this._devices.has(device.address))
//             return;

//         this._devices.get(device.address)?.close();
//         this._devices.delete(device.address);
//         this.notify('devices');
//         this.notify('connected-devices');
//         this.emit('changed');
//         this.emit('device-removed', device.address);
//     }

//     readonly connectDevice = (
//         device: BluetoothDevice,
//         connect: boolean,
//         callback: (s: boolean) => void,
//     ) => {
//         this._client.connect_service(
//             device.device.get_object_path(),
//             connect,
//             null,
//             (client: GnomeBluetooth.Client, res: Gio.AsyncResult) => {
//                 try {
//                     const s = client.connect_service_finish(res);
//                     callback(s);

//                     this.changed('connected-devices');
//                 } catch (error) {
//                     logError(error);
//                     callback(false);
//                 }
//             },
//         );
//     };

//     readonly getDevice = (address: string) => this._devices.get(address);

//     set enabled(v) { this._client.default_adapter_powered = v; }
//     get enabled() { return this.state === 'on' || this.state === 'turning-on'; }

//     get state() { return _ADAPTER_STATE[this._client.default_adapter_state]; }

//     get devices() { return Array.from(this._devices.values()); }
//     get connected_devices() {
//         const list = [];
//         for (const [, device] of this._devices) {
//             if (device.connected)
//                 list.push(device);
//         }
//         return list;
//     }
// }

// export const bluetooth = new Bluetooth;
// export default bluetooth;
