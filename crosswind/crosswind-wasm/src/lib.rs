use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn calculate_crosswind(runway: f64, wind_dir: f64, wind_speed: f64) -> f64 {
    let angle_rad = (wind_dir - runway).to_radians();
    (wind_speed * angle_rad.sin()).abs().round()
}

#[wasm_bindgen]
pub fn calculate_headwind(runway: f64, wind_dir: f64, wind_speed: f64) -> f64 {
    let angle_rad = (wind_dir - runway).to_radians();
    (wind_speed * angle_rad.cos()).round()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_crosswind() {
        // Wind 90° off runway → full crosswind, zero headwind
        assert_eq!(calculate_crosswind(270.0, 360.0, 20.0), 20.0);
        assert_eq!(calculate_headwind(270.0, 360.0, 20.0), 0.0);
    }

    #[test]
    fn direct_headwind() {
        // Wind aligned with runway → zero crosswind, full headwind
        assert_eq!(calculate_crosswind(270.0, 270.0, 15.0), 0.0);
        assert_eq!(calculate_headwind(270.0, 270.0, 15.0), 15.0);
    }

    #[test]
    fn direct_tailwind() {
        // Wind 180° off runway → zero crosswind, negative (tail) wind
        assert_eq!(calculate_crosswind(90.0, 270.0, 10.0), 0.0);
        assert_eq!(calculate_headwind(90.0, 270.0, 10.0), -10.0);
    }

    #[test]
    fn forty_five_degrees() {
        // 45° angle → equal crosswind and headwind components
        let xwind = calculate_crosswind(360.0, 45.0, 20.0);
        let hwind = calculate_headwind(360.0, 45.0, 20.0);
        assert_eq!(xwind, 14.0);
        assert_eq!(hwind, 14.0);
    }

    #[test]
    fn zero_wind_speed() {
        assert_eq!(calculate_crosswind(180.0, 90.0, 0.0), 0.0);
        assert_eq!(calculate_headwind(180.0, 90.0, 0.0), 0.0);
    }

    #[test]
    fn crosswind_always_positive() {
        // Crosswind should be absolute value regardless of wind side
        let left = calculate_crosswind(270.0, 230.0, 15.0);
        let right = calculate_crosswind(270.0, 310.0, 15.0);
        assert!(left > 0.0);
        assert_eq!(left, right);
    }
}
