pub struct InterpValue<T> {
    pub start: T,
    pub target: T,
    pub duration_sec: f32,
    pub elapsed: f32,
}

impl<T> InterpValue<T>
where
    T: Copy
        + std::ops::Sub<Output = T>
        + std::ops::Add<Output = T>
        + std::ops::Div<f32, Output = T>
        + std::ops::Mul<f32, Output = T>,
{
    pub fn new(start: T, target: T, duration_sec: f32) -> Self {
        Self {
            start,
            target,
            duration_sec,
            elapsed: 0.0,
        }
    }

    pub fn update(&mut self, delta_sec: f32) {
        self.elapsed += delta_sec;
    }

    pub fn value(&self) -> T {
        if self.elapsed >= self.duration_sec {
            self.target
        } else {
            self.start + (self.target - self.start) / self.duration_sec * self.elapsed
        }
    }
}
