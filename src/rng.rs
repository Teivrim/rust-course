//! Мини-PRNG (xorshift64*).
//!
//! Реальный проект: `rand = "0.9"` (ChaCha12) — криптостойкий и быстрый.
//! Здесь — потому что (а) сети нет, (б) полезно понимать, что ГПСЧ в
//! тестах обязан быть **детерминированным**: тот же seed -> тот же результат.
//! Тест, который падает один раз из ста, хуже, чем неудачный тест.

/// Генератор. `Copy`, потому что состояние — просто `u64` и копировать дёшево.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rng(u64);

impl Rng {
    /// Из `seed` делаем состояние: 0 — вырожденное (xorshift застревает).
    pub fn new(seed: u64) -> Self {
        Self(if seed == 0 { 0x9E37_79B9_7F4A_7C15 } else { seed })
    }

    /// Псевдослучайный `u64` (xorshift64*).
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// `u32` из старших битов — старшие биты xorshift* хорошие,
    /// младшие могут коррелировать (это ловушка `rand_next_u32`).
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// Равномерно в `0..n`. Использует Lemire: без modulo bias.
    pub fn below(&mut self, n: u32) -> u32 {
        assert!(n > 0, "below(0) — бессмысленно");
        // Rejection sampling: отбрасываем окно, чтобы остатки были равномерны.
        let threshold = n.wrapping_neg() % n;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % n;
            }
        }
    }

    /// `usize` в `0..n`.
    pub fn below_usize(&mut self, n: usize) -> usize {
        self.below(n as u32) as usize
    }

    /// `f32` в `[0, 1)`: 24 старших бита = 24 бита мантиссы float'а.
    pub fn f32(&mut self) -> f32 {
        (self.next_u32() >> 8) as f32 / (1u32 << 24) as f32
    }

    /// `f32` в `[lo, hi)`.
    pub fn range_f32(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.f32()
    }

    /// Случайная строка из ASCII — для тестов парсеров и строковых задач.
    pub fn ascii(&mut self, out: &mut String, len: usize) {
        use std::fmt::Write;
        for _ in 0..len {
            let c = 0x20 + self.below(0x5F); // 0x20..=0x7E
            let _ = write!(out, "{}", c as u8 as char);
        }
    }
}

impl Default for Rng {
    fn default() -> Self {
        // Фиксированный seed по умолчанию: тест обязан быть воспроизводимым.
        // Для "случайных" данных передавай seed из аргументов или времени.
        Self::new(0x1234_5678_9ABC_DEF0)
    }
}
