#[cfg(not(target_arch = "wasm32"))]
use vellum_grid_core::date_serial;

const SECONDS_PER_DAY: f64 = 86_400.0;
const UNIX_EPOCH_SERIAL: f64 = 25_569.0;

#[cfg(not(target_arch = "wasm32"))]
#[repr(C)]
struct Tm {
    sec: i32,
    min: i32,
    hour: i32,
    mday: i32,
    mon: i32,
    year: i32,
    wday: i32,
    yday: i32,
    isdst: i32,
    extra: [i64; 2],
}

#[cfg(target_os = "windows")]
unsafe extern "C" {
    fn _localtime64_s(tm: *mut Tm, time: *const i64) -> i32;
}

#[cfg(all(unix, not(target_arch = "wasm32")))]
unsafe extern "C" {
    fn localtime_r(time: *const i64, tm: *mut Tm) -> *mut Tm;
}

#[cfg(not(target_arch = "wasm32"))]
fn local_time(seconds: i64) -> Option<Tm> {
    let mut tm = Tm {
        sec: 0,
        min: 0,
        hour: 0,
        mday: 0,
        mon: 0,
        year: 0,
        wday: 0,
        yday: 0,
        isdst: 0,
        extra: [0; 2],
    };
    #[cfg(target_os = "windows")]
    let ok = unsafe { _localtime64_s(&mut tm, &seconds) == 0 };
    #[cfg(all(unix, not(target_arch = "wasm32")))]
    let ok = unsafe { !localtime_r(&seconds, &mut tm).is_null() };
    ok.then_some(tm)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> f64 {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let local = local_time(elapsed.as_secs() as i64).and_then(|tm| {
        let day = date_serial(
            i64::from(tm.year) + 1900,
            u32::try_from(tm.mon + 1).ok()?,
            u32::try_from(tm.mday).ok()?,
        )?;
        let time = f64::from(tm.hour * 3600 + tm.min * 60 + tm.sec) / SECONDS_PER_DAY;
        Some(day + time)
    });
    local.unwrap_or_else(|| elapsed.as_secs_f64() / SECONDS_PER_DAY + UNIX_EPOCH_SERIAL)
}

#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    const SECONDS_PER_MINUTE: f64 = 60.0;
    let date = js_sys::Date::new_0();
    let seconds = date.get_time() / 1000.0 - date.get_timezone_offset() * SECONDS_PER_MINUTE;
    seconds / SECONDS_PER_DAY + UNIX_EPOCH_SERIAL
}

pub fn today() -> f64 {
    now().floor()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_is_a_recent_local_serial() {
        let now = now();
        assert!(now > 46_000.0 && now < 60_000.0);
        assert_eq!(today(), now.floor());
        let utc = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            / SECONDS_PER_DAY
            + UNIX_EPOCH_SERIAL;
        assert!((now - utc).abs() < 1.0);
    }
}
