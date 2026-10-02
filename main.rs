//! Neon Breakout: a tiny Rust game. Every brick plays a note.
//! Sounds are generated in code (sine waves), so there are no asset files.

use macroquad::audio::{load_sound_from_bytes, play_sound, PlaySoundParams, Sound};
use macroquad::prelude::*;

const W: f32 = 800.0;
const H: f32 = 600.0;

const PADDLE_W: f32 = 110.0;
const PADDLE_H: f32 = 14.0;
const PADDLE_Y: f32 = H - 40.0;
const PADDLE_SPEED: f32 = 600.0;

const BALL_R: f32 = 8.0;
const START_SPEED: f32 = 380.0;
const MAX_SPEED: f32 = 700.0;

const COLS: usize = 10;
const ROWS: usize = 6;
const BRICK_H: f32 = 22.0;
const GAP: f32 = 6.0;

// C major pentatonic, top row = highest note
const NOTES: [f32; ROWS] = [1046.50, 880.00, 783.99, 659.25, 587.33, 523.25];

fn window_conf() -> Conf {
    Conf {
        window_title: "Neon Breakout".to_owned(),
        window_width: W as i32,
        window_height: H as i32,
        window_resizable: false,
        ..Default::default()
    }
}

// ---------- colors & glow ----------

fn row_color(row: usize) -> Color {
    match row {
        0 => Color::new(1.00, 0.25, 0.65, 1.0), // pink
        1 => Color::new(1.00, 0.55, 0.20, 1.0), // orange
        2 => Color::new(1.00, 0.90, 0.25, 1.0), // yellow
        3 => Color::new(0.30, 1.00, 0.55, 1.0), // green
        4 => Color::new(0.25, 0.85, 1.00, 1.0), // cyan
        _ => Color::new(0.70, 0.45, 1.00, 1.0), // purple
    }
}

fn glow_rect(r: Rect, c: Color) {
    for i in (1..=4).rev() {
        let e = i as f32 * 2.5;
        draw_rectangle(
            r.x - e,
            r.y - e,
            r.w + 2.0 * e,
            r.h + 2.0 * e,
            Color::new(c.r, c.g, c.b, 0.07),
        );
    }
    draw_rectangle(r.x, r.y, r.w, r.h, c);
}

fn glow_circle(p: Vec2, radius: f32, c: Color) {
    for i in (1..=4).rev() {
        draw_circle(p.x, p.y, radius + i as f32 * 3.0, Color::new(c.r, c.g, c.b, 0.07));
    }
    draw_circle(p.x, p.y, radius, c);
}

fn center_text(text: &str, y: f32, size: u16, color: Color) {
    let dim = measure_text(text, None, size, 1.0);
    draw_text(text, (W - dim.width) / 2.0, y, size as f32, color);
}

// ---------- sound ----------

/// Builds a tiny mono 16-bit WAV in memory: a sine wave with a fade-out.
fn make_tone(freq: f32, secs: f32) -> Vec<u8> {
    let sample_rate = 44_100u32;
    let n = (sample_rate as f32 * secs) as usize;
    let data_len = (n * 2) as u32;

    let mut wav: Vec<u8> = Vec::with_capacity(44 + n * 2);
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVE");
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes()); // PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&(sample_rate * 2).to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());

    for i in 0..n {
        let t = i as f32 / sample_rate as f32;
        let fade = (1.0 - i as f32 / n as f32).powi(2);
        let s = (t * freq * std::f32::consts::TAU).sin() * fade * 0.6;
        wav.extend_from_slice(&((s * i16::MAX as f32) as i16).to_le_bytes());
    }
    wav
}

struct Sfx {
    notes: Vec<Sound>,
    paddle: Sound,
    wall: Sound,
    lose: Sound,
}

impl Sfx {
    async fn load() -> Self {
        let mut notes = Vec::new();
        for f in NOTES {
            notes.push(load_sound_from_bytes(&make_tone(f, 0.25)).await.unwrap());
        }
        Sfx {
            notes,
            paddle: load_sound_from_bytes(&make_tone(260.0, 0.10)).await.unwrap(),
            wall: load_sound_from_bytes(&make_tone(180.0, 0.06)).await.unwrap(),
            lose: load_sound_from_bytes(&make_tone(110.0, 0.60)).await.unwrap(),
        }
    }
}

fn play(s: &Sound) {
    play_sound(s, PlaySoundParams { looped: false, volume: 0.4 });
}

// ---------- game ----------

struct Brick {
    rect: Rect,
    row: usize,
}

struct Particle {
    pos: Vec2,
    vel: Vec2,
    life: f32,
    color: Color,
}

#[derive(PartialEq)]
enum State {
    Serve,
    Playing,
    Won,
    GameOver,
}

struct Game {
    state: State,
    paddle_x: f32, // center of the paddle
    ball: Vec2,
    vel: Vec2,
    speed: f32,
    bricks: Vec<Brick>,
    particles: Vec<Particle>,
    score: u32,
    lives: u32,
    last_mouse_x: f32,
}

fn build_bricks() -> Vec<Brick> {
    let margin = 30.0;
    let brick_w = (W - 2.0 * margin - (COLS as f32 - 1.0) * GAP) / COLS as f32;
    let mut bricks = Vec::new();
    for row in 0..ROWS {
        for col in 0..COLS {
            bricks.push(Brick {
                rect: Rect::new(
                    margin + col as f32 * (brick_w + GAP),
                    70.0 + row as f32 * (BRICK_H + GAP),
                    brick_w,
                    BRICK_H,
                ),
                row,
            });
        }
    }
    bricks
}

fn circle_hits_rect(c: Vec2, r: &Rect) -> bool {
    let cx = c.x.clamp(r.x, r.x + r.w);
    let cy = c.y.clamp(r.y, r.y + r.h);
    vec2(c.x - cx, c.y - cy).length_squared() < BALL_R * BALL_R
}

impl Game {
    fn new() -> Self {
        Game {
            state: State::Serve,
            paddle_x: W / 2.0,
            ball: vec2(W / 2.0, PADDLE_Y - BALL_R - 1.0),
            vel: Vec2::ZERO,
            speed: START_SPEED,
            bricks: build_bricks(),
            particles: Vec::new(),
            score: 0,
            lives: 3,
            last_mouse_x: mouse_position().0,
        }
    }

    fn update(&mut self, dt: f32, sfx: &Sfx) {
        // paddle: keyboard (arrows / A D) or mouse
        let mut dx = 0.0;
        if is_key_down(KeyCode::Left) || is_key_down(KeyCode::A) {
            dx -= 1.0;
        }
        if is_key_down(KeyCode::Right) || is_key_down(KeyCode::D) {
            dx += 1.0;
        }
        self.paddle_x += dx * PADDLE_SPEED * dt;
        let mx = mouse_position().0;
        if (mx - self.last_mouse_x).abs() > 0.5 {
            self.paddle_x = mx;
        }
        self.last_mouse_x = mx;
        self.paddle_x = self.paddle_x.clamp(PADDLE_W / 2.0, W - PADDLE_W / 2.0);

        self.update_particles(dt);

        let action = is_key_pressed(KeyCode::Space) || is_mouse_button_pressed(MouseButton::Left);
        match self.state {
            State::Serve => {
                self.ball = vec2(self.paddle_x, PADDLE_Y - BALL_R - 1.0);
                if action {
                    self.vel = vec2(0.3, -1.0).normalize() * self.speed;
                    self.state = State::Playing;
                }
            }
            State::Playing => self.step_ball(dt, sfx),
            State::Won | State::GameOver => {
                if action || is_key_pressed(KeyCode::R) {
                    *self = Game::new();
                }
            }
        }
    }

    fn step_ball(&mut self, dt: f32, sfx: &Sfx) {
        self.ball += self.vel * dt;

        // walls
        if self.ball.x < BALL_R {
            self.ball.x = BALL_R;
            self.vel.x = self.vel.x.abs();
            play(&sfx.wall);
        }
        if self.ball.x > W - BALL_R {
            self.ball.x = W - BALL_R;
            self.vel.x = -self.vel.x.abs();
            play(&sfx.wall);
        }
        if self.ball.y < BALL_R {
            self.ball.y = BALL_R;
            self.vel.y = self.vel.y.abs();
            play(&sfx.wall);
        }

        // paddle (hit position changes the bounce angle)
        let paddle = Rect::new(self.paddle_x - PADDLE_W / 2.0, PADDLE_Y, PADDLE_W, PADDLE_H);
        if self.vel.y > 0.0 && circle_hits_rect(self.ball, &paddle) {
            let offset = ((self.ball.x - self.paddle_x) / (PADDLE_W / 2.0)).clamp(-1.0, 1.0);
            self.vel = vec2(offset * 0.9, -1.0).normalize() * self.speed;
            self.ball.y = PADDLE_Y - BALL_R - 0.5;
            play(&sfx.paddle);
        }

        // bricks
        if let Some(i) = self.bricks.iter().position(|b| circle_hits_rect(self.ball, &b.rect)) {
            let brick = self.bricks.remove(i);
            let r = brick.rect;
            if self.ball.x >= r.x && self.ball.x <= r.x + r.w {
                self.vel.y = -self.vel.y;
            } else {
                self.vel.x = -self.vel.x;
            }
            self.score += 10 * (ROWS - brick.row) as u32;
            self.speed = (self.speed + 4.0).min(MAX_SPEED);
            self.vel = self.vel.normalize() * self.speed;
            self.spawn_burst(r.center(), row_color(brick.row));
            play(&sfx.notes[brick.row]);
            if self.bricks.is_empty() {
                self.state = State::Won;
            }
        }

        // ball fell off the bottom
        if self.ball.y > H + 20.0 {
            play(&sfx.lose);
            self.lives -= 1;
            self.speed = START_SPEED;
            self.state = if self.lives == 0 { State::GameOver } else { State::Serve };
        }
    }

    fn spawn_burst(&mut self, pos: Vec2, color: Color) {
        for _ in 0..14 {
            let angle = rand::gen_range(0.0, std::f32::consts::TAU);
            let speed = rand::gen_range(60.0, 240.0);
            self.particles.push(Particle {
                pos,
                vel: vec2(angle.cos(), angle.sin()) * speed,
                life: 0.6,
                color,
            });
        }
    }

    fn update_particles(&mut self, dt: f32) {
        for p in &mut self.particles {
            p.pos += p.vel * dt;
            p.life -= dt;
        }
        self.particles.retain(|p| p.life > 0.0);
    }

    fn draw(&self) {
        clear_background(Color::from_rgba(10, 8, 24, 255));

        for b in &self.bricks {
            glow_rect(b.rect, row_color(b.row));
        }

        let paddle = Rect::new(self.paddle_x - PADDLE_W / 2.0, PADDLE_Y, PADDLE_W, PADDLE_H);
        glow_rect(paddle, Color::new(0.25, 0.9, 1.0, 1.0));
        glow_circle(self.ball, BALL_R, WHITE);

        for p in &self.particles {
            let mut c = p.color;
            c.a = (p.life / 0.6).clamp(0.0, 1.0);
            draw_rectangle(p.pos.x - 2.0, p.pos.y - 2.0, 4.0, 4.0, c);
        }

        // HUD
        draw_text(&format!("SCORE {}", self.score), 20.0, 38.0, 30.0, WHITE);
        for i in 0..self.lives {
            glow_circle(vec2(W - 30.0 - i as f32 * 26.0, 30.0), 7.0, Color::new(1.0, 0.25, 0.65, 1.0));
        }

        match self.state {
            State::Serve => center_text("SPACE or CLICK to launch", H / 2.0 + 60.0, 32, WHITE),
            State::Won => {
                center_text("YOU WIN", H / 2.0 + 40.0, 64, Color::new(0.3, 1.0, 0.55, 1.0));
                center_text("SPACE or R to play again", H / 2.0 + 85.0, 28, WHITE);
            }
            State::GameOver => {
                center_text("GAME OVER", H / 2.0 + 40.0, 64, Color::new(1.0, 0.25, 0.65, 1.0));
                center_text("SPACE or R to try again", H / 2.0 + 85.0, 28, WHITE);
            }
            State::Playing => {}
        }
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    let sfx = Sfx::load().await;
    let mut game = Game::new();

    loop {
        let dt = get_frame_time().min(0.033);
        game.update(dt, &sfx);
        game.draw();
        next_frame().await;
    }
}
