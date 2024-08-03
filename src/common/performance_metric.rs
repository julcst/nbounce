pub struct PerformanceMetrics<const BUFFER_SIZE: usize> {
    last_frame: Option<std::time::Instant>,
    curr_frame_time: std::time::Duration,
    time_since_start: std::time::Duration,
    // Ring buffer of frame times
    frame_time_buffer: [std::time::Duration; BUFFER_SIZE],
    idx: usize,
    n_frames: usize,
    summed_frame_time: std::time::Duration,
}

impl<const BUFFER_SIZE: usize> Default for PerformanceMetrics<BUFFER_SIZE>{
    fn default() -> Self {
        Self {
            last_frame: None,
            curr_frame_time: std::time::Duration::default(),
            time_since_start: std::time::Duration::default(),
            frame_time_buffer: [std::time::Duration::default(); BUFFER_SIZE],
            idx: 0,
            n_frames: 0,
            summed_frame_time: std::time::Duration::default(),
        }
    }
}

impl<const BUFFER_SIZE: usize> PerformanceMetrics<BUFFER_SIZE> {
    pub fn next_frame(&mut self) {
        match self.last_frame {
            None => {
                self.last_frame = Some(std::time::Instant::now());
            }
            Some(last_frame) => {
                let now = std::time::Instant::now();
                self.curr_frame_time = now.duration_since(last_frame);
                self.last_frame = Some(now);
                self.time_since_start += self.curr_frame_time;

                // Update sum
                self.summed_frame_time += self.curr_frame_time;
                if self.n_frames < BUFFER_SIZE {
                    self.n_frames += 1;
                } else {
                    self.summed_frame_time -= self.frame_time_buffer[self.idx];
                }

                // Update ring buffer
                self.frame_time_buffer[self.idx] = self.curr_frame_time;
                self.idx = (self.idx + 1) % BUFFER_SIZE;
            }
        }
    }

    pub fn pause(&mut self) {
        self.last_frame = None;
    }

    pub fn time_since_start(&self) -> std::time::Duration {
        self.time_since_start
    }

    pub fn avg_frame_time(&self) -> std::time::Duration {
        self.summed_frame_time.checked_div(self.n_frames as u32).unwrap_or_default()
    }

    pub fn curr_frame_time(&self) -> std::time::Duration {
        self.curr_frame_time
    }

    pub fn avg_frame_rate(&self) -> f32 {
        1.0 / self.avg_frame_time().as_secs_f32()
    }

    pub fn curr_frame_rate(&self) -> f32 {
        1.0 / self.curr_frame_time.as_secs_f32()
    }
}