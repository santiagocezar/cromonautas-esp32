use color::{ColorSpace, Hsl, LinearSrgb, Srgb};

pub fn linear_to_srgb(color: &[u8; 3]) -> [u8; 3] {
    Srgb::convert::<LinearSrgb>(color.map(|c| (c as f32) / 255.)).map(|c| ((1. - c) * 255.) as u8)
}

pub fn rgb8_to_hsl(rgb: &[u8; 3]) -> [f32; 3] {
    Srgb::convert::<Hsl>(rgb.map(|c| c as f32 / 255.))
}

pub fn hsl_to_xy(&[h, s, l]: &[f32; 3]) -> (f32, f32) {
    let h = h.to_radians();
    let s = s / 100.;
    let l = l / 100.;
    let r = (1. - (l - 0.5).abs() * 2.) * s;

    (r * h.cos() * 50., r * h.sin() * 50.)
}

pub fn closeness(c1: &[f32; 3], c2: &[f32; 3]) -> f32 {
    let (x1, y1) = hsl_to_xy(c1);
    let (x2, y2) = hsl_to_xy(c2);
    return 100. - ((x1 - x2).powi(2) + (y1 - y2).powi(2)).sqrt();
}

#[cfg(test)]
mod tests {
    use float_cmp::assert_approx_eq;

    use super::*;

    const EPSILON_POSITION: f32 = 0.5;
    const EPSILON_CLOSENESS: f32 = 0.5;

    fn compare_f32s(a: impl IntoIterator<Item = f32>, b: impl IntoIterator<Item = f32>) {
        a.into_iter()
            .zip(b.into_iter())
            .for_each(|(a, b)| assert_approx_eq!(f32, a, b, (0.0, 2)));
    }

    #[test]
    fn test_hsl() {
        let c = rgb8_to_hsl(&[0, 0, 0]);

        compare_f32s(c, [0., 0., 0.]);

        let c = rgb8_to_hsl(&[255, 255, 255]);

        compare_f32s(c, [0., 0., 100.]);

        let c = rgb8_to_hsl(&[255, 0, 0]);

        compare_f32s(c, [0., 100., 50.]);

        let c = rgb8_to_hsl(&[0, 255, 0]);

        compare_f32s(c, [120., 100., 50.]);
    }

    #[test]
    fn test_position() {
        let c = rgb8_to_hsl(&[0, 0, 0]);

        let (x, y) = hsl_to_xy(&c);

        assert_approx_eq!(f32, x, 0., (EPSILON_POSITION, 2));
        assert_approx_eq!(f32, y, 0., (EPSILON_POSITION, 2));

        let c = rgb8_to_hsl(&[255, 0, 0]);

        let (x, y) = hsl_to_xy(&c);

        assert_approx_eq!(f32, x, 50., (EPSILON_POSITION, 2));
        assert_approx_eq!(f32, y, 0., (EPSILON_POSITION, 2));

        let c = rgb8_to_hsl(&[128, 255, 0]);

        let (x, y) = hsl_to_xy(&c);

        assert_approx_eq!(f32, x, 0., (EPSILON_POSITION, 2));
        assert_approx_eq!(f32, y, 50., (EPSILON_POSITION, 2));
    }

    #[test]
    fn test_closeness_equal() {
        let c1 = rgb8_to_hsl(&[0, 0, 0]);
        let c2 = rgb8_to_hsl(&[0, 0, 0]);

        let val = closeness(&c1, &c2);

        assert_approx_eq!(f32, val, 100., (EPSILON_CLOSENESS, 2));

        let c1 = rgb8_to_hsl(&[255, 0, 0]);
        let c2 = rgb8_to_hsl(&[0, 0, 0]);

        let val = closeness(&c1, &c2);

        assert_approx_eq!(f32, val, 50., (EPSILON_CLOSENESS, 2));

        let c1 = rgb8_to_hsl(&[255, 255, 0]);
        let c2 = rgb8_to_hsl(&[0, 0, 0]);

        let val = closeness(&c1, &c2);

        assert_approx_eq!(f32, val, 50., (EPSILON_CLOSENESS, 2));

        let c1 = rgb8_to_hsl(&[128, 128, 0]);
        let c2 = rgb8_to_hsl(&[0, 0, 0]);

        let val = closeness(&c1, &c2);

        assert_approx_eq!(f32, val, 75., (EPSILON_CLOSENESS, 2));
    }

    #[test]
    fn test_closeness_near() {}
}
