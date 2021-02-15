pub struct FpsCounter {
    frame_time_history: [f32; 60],
    index: usize,
    total_frame_time: f32,
}

impl FpsCounter {
    pub fn new() -> Self {
        FpsCounter {
            frame_time_history: [0.; 60],
            index: 0,
            total_frame_time: 0.,
        }
    }

    pub fn update_fps(&mut self, frame_time: f32) -> f32 {
        self.index = (self.index + 1) % self.frame_time_history.len();
        self.total_frame_time -= self.frame_time_history[self.index];
        self.total_frame_time += frame_time;
        self.frame_time_history[self.index] = frame_time;

        self.frame_time_history.len() as f32 / self.total_frame_time
    }
}
