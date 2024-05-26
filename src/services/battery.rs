use std::sync::Arc;

use log::{info, warn};
use tokio::sync::{broadcast::{channel, Sender}, RwLock};
use zbus::{proxy, Connection, fdo::PropertiesProxy};
use tokio_stream::StreamExt;

#[derive(Eq, PartialEq, Clone, Copy, Debug)]
#[repr(u8)]
pub enum BatteryState {
    Unknown = 0,
    Charging = 1,
    Discharigng = 2,
    Empty = 3,
    FullyCharged = 4,
    PendingCharge = 5,
    PendingDischarge = 6
}

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower",
    default_path = "/org/freedesktop/UPower/devices/DisplayDevice"
)]
trait Battery {
    /// GetHistory method
    fn get_history(
        &self,
        type_: &str,
        timespan: u32,
        resolution: u32,
    ) -> zbus::Result<Vec<(u32, f64, u32)>>;

    /// GetStatistics method
    fn get_statistics(&self, type_: &str) -> zbus::Result<Vec<(f64, f64)>>;

    /// Refresh method
    fn refresh(&self) -> zbus::Result<()>;

    /// BatteryLevel property
    #[zbus(property)]
    fn battery_level(&self) -> zbus::Result<u32>;

    /// Capacity property
    #[zbus(property)]
    fn capacity(&self) -> zbus::Result<f64>;

    /// ChargeCycles property
    #[zbus(property)]
    fn charge_cycles(&self) -> zbus::Result<i32>;

    /// Energy property
    #[zbus(property)]
    fn energy(&self) -> zbus::Result<f64>;

    /// EnergyEmpty property
    #[zbus(property)]
    fn energy_empty(&self) -> zbus::Result<f64>;

    /// EnergyFull property
    #[zbus(property)]
    fn energy_full(&self) -> zbus::Result<f64>;

    /// EnergyFullDesign property
    #[zbus(property)]
    fn energy_full_design(&self) -> zbus::Result<f64>;

    /// EnergyRate property
    #[zbus(property)]
    fn energy_rate(&self) -> zbus::Result<f64>;

    /// HasHistory property
    #[zbus(property)]
    fn has_history(&self) -> zbus::Result<bool>;

    /// HasStatistics property
    #[zbus(property)]
    fn has_statistics(&self) -> zbus::Result<bool>;

    /// IconName property
    #[zbus(property)]
    fn icon_name(&self) -> zbus::Result<String>;

    /// IsPresent property
    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;

    /// IsRechargeable property
    #[zbus(property)]
    fn is_rechargeable(&self) -> zbus::Result<bool>;

    /// Luminosity property
    #[zbus(property)]
    fn luminosity(&self) -> zbus::Result<f64>;

    /// Model property
    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;

    /// NativePath property
    #[zbus(property)]
    fn native_path(&self) -> zbus::Result<String>;

    /// Online property
    #[zbus(property)]
    fn online(&self) -> zbus::Result<bool>;

    /// Percentage property
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;

    /// PowerSupply property
    #[zbus(property)]
    fn power_supply(&self) -> zbus::Result<bool>;

    /// Serial property
    #[zbus(property)]
    fn serial(&self) -> zbus::Result<String>;

    /// State property
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    /// Technology property
    #[zbus(property)]
    fn technology(&self) -> zbus::Result<u32>;

    /// Temperature property
    #[zbus(property)]
    fn temperature(&self) -> zbus::Result<f64>;

    /// TimeToEmpty property
    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;

    /// TimeToFull property
    #[zbus(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;

    /// Type property
    #[zbus(property)]
    fn type_(&self) -> zbus::Result<u32>;

    /// UpdateTime property
    #[zbus(property)]
    fn update_time(&self) -> zbus::Result<u64>;

    /// Vendor property
    #[zbus(property)]
    fn vendor(&self) -> zbus::Result<String>;

    /// Voltage property
    #[zbus(property)]
    fn voltage(&self) -> zbus::Result<f64>;

    /// WarningLevel property
    #[zbus(property)]
    fn warning_level(&self) -> zbus::Result<u32>;
}

#[derive(PartialEq, Clone, Debug)]
pub struct BatteryData {
    pub available: bool,
    pub percent: i64,
    pub charging: bool,
    pub charged: bool,
    pub icon_name: String,
    pub time_remaining: i64,
    pub energy: f64,
    pub energy_full: f64,
    pub energy_rate: f64
}

impl Default for BatteryData {
    fn default() -> Self {
        BatteryData {
            available: false,
            percent: -1,
            charging: false,
            charged: false,
            icon_name: "battery-missing-symbolic".into(),
            time_remaining: 0,
            energy: 0.,
            energy_full: 0.,
            energy_rate: 0.
        }
    }
}

#[derive(Clone, Debug)]
pub struct BatterySender {
    pub changed: Sender<Arc<RwLock<BatteryData>>>,
    pub available: Sender<Arc<RwLock<BatteryData>>>,
    pub icon_name: Sender<Arc<RwLock<BatteryData>>>,
    pub percent: Sender<Arc<RwLock<BatteryData>>>,
    pub charging: Sender<Arc<RwLock<BatteryData>>>,
    pub charged: Sender<Arc<RwLock<BatteryData>>>,
    pub time_remaining: Sender<Arc<RwLock<BatteryData>>>,
    pub energy: Sender<Arc<RwLock<BatteryData>>>,
    pub energy_full: Sender<Arc<RwLock<BatteryData>>>,
    pub energy_rate: Sender<Arc<RwLock<BatteryData>>>,
}

impl BatterySender {
    fn new() -> Self {
        Self {
            changed: channel(30).0,
            available: channel(30).0,
            icon_name: channel(30).0,
            percent: channel(30).0,
            charging: channel(30).0,
            charged: channel(30).0,
            time_remaining: channel(30).0,
            energy: channel(30).0,
            energy_full: channel(30).0,
            energy_rate: channel(30).0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum BatteryIcons {
    AGSLike,
    Internal
}

#[derive(Clone, Debug)]
pub struct BatteryService {
    pub sender: BatterySender,
    pub data: Arc<RwLock<BatteryData>>,

    pub icons: BatteryIcons
}




impl BatteryService {
    pub async fn new(icons: BatteryIcons) -> zbus::Result<Arc<RwLock<Self>>> {
        let battery_data = Arc::new(RwLock::new(BatteryData::default()));
        let q = Arc::new(RwLock::new(Self {
            data: battery_data.clone(),
            sender: BatterySender::new(),
            icons
        }));
        {
            let p = q.clone();


            let connection = Connection::system().await?;
            let bip = BatteryProxy::new(&connection).await?;
            let ppp = PropertiesProxy::new(&connection, "org.freedesktop.UPower", "/org/freedesktop/UPower/devices/DisplayDevice").await?;

            {
                q.write().await.sync(&bip).await?;
            }

            tokio::spawn(async move {
                let mut changed_stream = ppp.receive_properties_changed().await.unwrap();
                while let Some(_) = changed_stream.next().await {
                    p.write().await.sync(&bip).await.unwrap();
                }
            });
        }

        Ok(q)
    }

    async fn sync<'a>(&mut self, bip: &BatteryProxy<'a>) -> zbus::Result<()> {
        let available = bip.is_present().await?;
        let state = bip.state().await? as u8;
        let charging = state == BatteryState::Charging as u8;
        let percent = bip.percentage().await? as i64;
        let level = (percent / 10) * 10;
        let charged = state == BatteryState::FullyCharged as u8 || state == BatteryState::Charging as u8&& percent == 100;


        let time_remaining = if charging { bip.time_to_full().await? } else { bip.time_to_empty().await? };
        let icon_name = match self.icons {
            BatteryIcons::AGSLike => {
                let state = if state == BatteryState::Charging as u8 { "-charging" } else if charged { "-charged" } else { "" };
                format!("battery-level-{}{}-symbolic", level, state)
            }
            BatteryIcons::Internal => bip.icon_name().await?,
        };

        let energy = bip.energy().await?;
        let energy_full = bip.energy_full().await?;
        let energy_rate = bip.energy_rate().await?;


        self.update_available(available).await;
        self.update_icon_name(icon_name).await;
        self.update_percent(percent).await;
        self.update_charging(charging).await;
        self.update_charged(charged).await;
        self.update_time_remaining(time_remaining).await;
        self.update_energy(energy).await;
        self.update_energy_full(energy_full).await;
        self.update_energy_rate(energy_rate).await;
        self.update().await;
        Ok(())
    }

    update!(update_available, available, bool);
    update!(update_icon_name, icon_name, String);
    update!(update_percent, percent, i64);
    update!(update_charging, charging, bool);
    update!(update_charged, charged, bool);
    update!(update_time_remaining, time_remaining, i64);
    update!(update_energy, energy, f64);
    update!(update_energy_full, energy_full, f64);
    update!(update_energy_rate, energy_rate, f64);

    async fn update(&mut self) {
        match self.sender.changed.send(self.data.clone()) {
            Ok(_) => {},
            Err(_) => {info!("No receiver");}
        }
    }
}




