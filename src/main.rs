slint::include_modules!();








use chrono::Local;
use notify::{Watcher};

mod services;


#[derive(Debug)]
pub enum BacklightError{
    BacklightNotFoundError(std::io::Error),
    BrightnessCtlNotInstalled(std::io::Error)
}

#[tokio::main]
async fn main() -> Result<(), slint::PlatformError> {
    let ui = AppWindow::new()?;



    let _timer1 = {
        use slint::{Timer, TimerMode};
        let timer = Timer::default();
        timer.start(TimerMode::Repeated, std::time::Duration::from_millis(100), {
            let ui_handle = ui.as_weak();
            move || {
                let ui = ui_handle.unwrap();

                let now = Local::now();
                ui.global::<TimeAdapter>().set_time(slint::format!("{}", now.format("%H:%M:%S")));
                ui.global::<TimeAdapter>().set_date(slint::format!("{}", now.format("%Y-%m-%d")));
            }
        });
        timer
    };


    let battery_service = services::battery::BatteryService::new(services::battery::BatteryIcons::Internal)
        .await
        .expect("DBus connection couldn't be established for batteries");
    ui.global::<BatteryAdapter>().set_percentage(battery_service.read().await.data.read().await.percent as i32);
    {
        let ui_handle = ui.as_weak();
        let battery_service = battery_service.clone();
        let mut battery_rx = battery_service.read().await.sender.changed.subscribe();

        slint::spawn_local(async move {
            while let Ok(battery_data) = battery_rx.recv().await {
                let ui = ui_handle.unwrap();
                let battery_data = battery_data.read().await;
                ui.global::<BatteryAdapter>().set_percentage(battery_data.percent as i32);
            }
        }).unwrap();
    }

    let brightness_service = services::brightness::BrightnessService::new().await.unwrap();

    let bluetooth_service = services::bluetooth::BlueToothService::new().await;

    let cliphist_service = services::cliphist::CliphistService::new(50).await;


    // let mut pawrapper = services::audio::PulseWrapper::new();
    // let x = pawrapper.get_sources();
    let audio_service = services::audio::AudioService::new().unwrap();
    let application_service = services::applications::ApplicationService::new().await.unwrap();
    // dbg!(&application_service.read().await.data);
    let hyprland_service = services::hyprland::HyprlandService::new();




    // ui.on_request_increase_value({
    //     let ui_handle = ui.as_weak();
    //     move || {
    //         let ui = ui_handle.unwrap();
    //         ui.set_counter(ui.get_counter() + 1);
    //     }
    // });
    // Ok(())
    ui.run()
}
