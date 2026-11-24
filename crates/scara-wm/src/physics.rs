#[derive(Debug, Clone)]
pub struct Spring {
    pub value: f32,
    pub target: f32,
    pub velocity: f32,
    pub stiffness: f32, // Kekakuan pegas (contoh: 320.0)
    pub damping: f32,   // Redaman (contoh: 28.0 untuk smooth tanpa overshoot berlebihan)
}

impl Spring {
    pub fn new(initial: f32, stiffness: f32, damping: f32) -> Self {
        Self { value: initial, target: initial, velocity: 0.0, stiffness, damping }
    }

    /// Update fisika per frame (dt dalam detik, misal 1.0 / 120.0)
    pub fn step(&mut self, dt: f32) -> bool {
        let force = (self.target - self.value) * self.stiffness;
        let damping_force = self.velocity * self.damping;
        let accel = force - damping_force;

        self.velocity += accel * dt;
        self.value += self.velocity * dt;

        // Kembalikan true jika animasi masih bergerak
        (self.target - self.value).abs() > 0.01 || self.velocity.abs() > 0.01
    }
}