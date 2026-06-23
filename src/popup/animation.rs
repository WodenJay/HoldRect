/// Critically-damped (or underdamped) spring position.
/// Returns the value at time `elapsed_secs` given spring parameters.
/// At t=0 returns `start`. As t->inf returns `target`.
pub fn spring_position(elapsed_secs: f64, start: f64, target: f64, omega_n: f64, zeta: f64) -> f64 {
    let displacement = start - target;
    let t = elapsed_secs;

    if t <= 0.0 {
        return start;
    }

    if zeta >= 1.0 {
        // Critically damped or overdamped
        // x(t) = target + displacement * (1 + omega_n * t) * exp(-omega_n * t)
        let decay = (-omega_n * t).exp();
        target + displacement * (1.0 + omega_n * t) * decay
    } else {
        // Underdamped
        // x(t) = target + displacement * exp(-zeta * omega_n * t) *
        //         (cos(omega_d * t) + (zeta / sqrt(1 - zeta^2)) * sin(omega_d * t))
        let omega_d = omega_n * (1.0 - zeta * zeta).sqrt();
        let decay = (-zeta * omega_n * t).exp();
        let cos_part = (omega_d * t).cos();
        let sin_part = (zeta / (1.0 - zeta * zeta).sqrt()) * (omega_d * t).sin();
        target + displacement * decay * (cos_part + sin_part)
    }
}

#[cfg(test)]
mod tests {
    use super::spring_position;

    const EPSILON: f64 = 0.5;

    #[test]
    fn at_t0_returns_start() {
        let pos = spring_position(0.0, -60.0, 0.0, 18.0, 0.82);
        assert!((pos - (-60.0)).abs() < EPSILON);
    }

    #[test]
    fn converges_to_target() {
        let pos = spring_position(10.0, -60.0, 0.0, 18.0, 0.82);
        assert!((pos - 0.0).abs() < EPSILON);
    }

    #[test]
    fn underdamped_overshoots() {
        // zeta < 1 should overshoot target
        let mut max_pos = f64::MIN;
        for i in 0..500 {
            let t = i as f64 * 0.001;
            let pos = spring_position(t, -60.0, 0.0, 18.0, 0.82);
            if pos > max_pos {
                max_pos = pos;
            }
        }
        assert!(max_pos > 0.0, "underdamped spring should overshoot past target, got max={}", max_pos);
    }

    #[test]
    fn critically_damped_no_overshoot() {
        // zeta = 1.0 should never exceed target
        let mut max_pos = f64::MIN;
        for i in 0..1000 {
            let t = i as f64 * 0.001;
            let pos = spring_position(t, -60.0, 0.0, 22.0, 1.0);
            if pos > max_pos {
                max_pos = pos;
            }
        }
        assert!(max_pos <= 0.5, "critically damped should not overshoot, got max={}", max_pos);
    }

    #[test]
    fn converges_within_500ms() {
        let pos = spring_position(0.5, -60.0, 0.0, 18.0, 0.82);
        assert!((pos - 0.0).abs() < 2.0, "should be near target at 500ms, got {}", pos);
    }

    #[test]
    fn overshoot_is_roughly_4_percent() {
        // start=-60, target=0, displacement=60.
        // With zeta=0.82, theoretical overshoot ~ exp(-zeta*pi/sqrt(1-zeta^2)) ~ 1.1%
        // = ~0.67px past 0.
        let mut max_pos = f64::MIN;
        for i in 0..500 {
            let t = i as f64 * 0.001;
            let pos = spring_position(t, -60.0, 0.0, 18.0, 0.82);
            if pos > max_pos {
                max_pos = pos;
            }
        }
        // overshoot should be positive but bounded (1-5% of displacement)
        assert!(max_pos > 0.5 && max_pos < 5.0,
            "overshoot should be ~0.5-5px, got {}", max_pos);
    }
}
