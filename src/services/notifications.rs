use image::{DynamicImage, ImageBuffer};




#[derive(Clone, Debug)]
pub struct Action {
    pub id: String,
    pub label: String
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Urgency {
    Critical = 2,
    Normal = 1,
    Low = 0
}

impl From<u8> for Urgency {
    fn from(value: u8) -> Self {
        if value == 0 {
            Urgency::Low
        } else if value == 2 {
            Urgency::Critical
        } else {
            Urgency::Normal
        }
    }
}



#[derive(Clone, Debug)]
pub struct Hints {
    pub actionIcons: bool,
    pub category: String, 
    pub desktopEntry: String,
    pub imageData: DynamicImage,
    pub imagePath: String,
    pub resident: bool,
    pub soundFile: String,
    pub soundName: String,
    pub supressSound: bool,
    pub transient: bool,
    pub urgency: Urgency,
    pub x: i64,
    pub y: i64,
}
//         const [w, h, rs, alpha, bps, _, data] = imageData // iiibiiay
//             .recursiveUnpack<[number, number, number, boolean, number, number, GLib.Bytes]>();


