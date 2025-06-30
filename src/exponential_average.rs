#[derive(Copy, Clone)]
pub struct ExponentialAverage {
    alpha: f64,
    current_average: Option<f64>,
}

impl ExponentialAverage {
    pub fn new(alpha: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&alpha),
            "Alpha must be between 0 and 1"
        );
        ExponentialAverage {
            alpha,
            current_average: None,
        }
    }

    pub fn update(&mut self, new_value: f64) {
        self.current_average = Some(match self.current_average {
            None => new_value,
            Some(avg) => avg * (1.0 - self.alpha) + new_value * self.alpha,
        });
    }

    pub fn get_average(&self) -> Option<f64> {
        self.current_average
    }
}
