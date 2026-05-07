const BAR_WIDTH: u64 = 100;

pub struct Progress {
    state: u64,
    max_state: u64,
}

impl Progress {
    pub fn new(max_state: u64, start: Option<u64>) -> Self {
        let state = start.unwrap_or(0);
        Progress { state, max_state }
    }

    pub fn next(&mut self) {
        self.state += 1;
        self.print_status();
    }

    fn bar(&self) -> String {
        let filled = if self.max_state == 0 {
            BAR_WIDTH
        } else {
            (self.state.min(self.max_state) * BAR_WIDTH) / self.max_state
        };

        format!(
            "[{}{}]",
            "=".repeat(filled as usize),
            ".".repeat((BAR_WIDTH - filled) as usize)
        )
    }

    pub fn start_print(&self) {
        print!("{}", self.bar());
    }

    pub fn print_status(&self) {
        if self.state >= self.max_state {
            println!("\r{}", self.bar());
            println!("Finished!");
            return;
        }

        print!("\r{}", self.bar());
    }
}
