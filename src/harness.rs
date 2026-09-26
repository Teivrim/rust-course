//! Харнесс для обучения: баннеры, отчёт по заданиям, микро-бенчмарк.
//!
//! Механика отчёта: невыполненное задание паникует через `todo!`,
//! раннер ловит панику и печатает `[todo]` вместо падения. Поэтому
//! `cargo run --bin moduleNN` **всегда** отрабатывает и показывает, что
//! осталось. Это важно: иначе первый же `todo!` обрывает программу, и ты
//! не видишь примеры ниже по файлу.

use std::fmt;
use std::panic::AssertUnwindSafe;
use std::time::Instant;

// ─────────────────────────────────────────────────────────────────────────────
// Баннеры: дублируют структуру из комментариев в начале выполнения,
// чтобы ориентироваться не перечитывая 400 строк теории.
// ─────────────────────────────────────────────────────────────────────────────

pub fn module(number: u32, title: &str) {
    println!();
    println!("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}");
    println!("  МОДУЛЬ {number:02} — {title}");
    println!("\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}\u{2550}");
}

/// Начало части: `N.M`. Части в файле нумеруются сквозным порядком
/// ТЕОРИЯ -> ПРИМЕР -> ЗАДАНИЕ, но номер у части сквозной.
pub fn part(index: &str, title: &str) {
    println!();
    println!("\u{2500}\u{2500} ЧАСТЬ {index}: {title} \u{2500}\u{2500}");
}

/// Плашка «это пример, он самодостаточен».
pub fn example(label: &str) {
    println!("   \u{1F7E} пример: {label}");
}

/// Плашка «это теория из комментариев, читать выше по файлу».
pub fn theory_ref(label: &str) {
    println!("   \u{1F4D6} теория: {label}");
}

/// Плашка «это задание, которое надо написать самому».
pub fn task_ref(label: &str) {
    println!("   \u{270F} задание: {label}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Задания
// ─────────────────────────────────────────────────────────────────────────────

/// Маркер паники «задание не сделано». Отдельный тип, чтобы раннер отличал
/// «задание не сделано» от «задание сделано, но упало с ошибкой».
#[derive(Debug)]
pub struct Todo(pub String);

/// Пометить задание как несделанное. Бросает панику с [`Todo`], раннер
/// ловит и печатает строку с причиной.
///
/// Именно **макрос**, а не функция: тело задания не может просто
/// «вернуться», оно обязано остановиться. Если бы это была функция,
/// компилятор требовал бы `return not_yet(..)` и компилировал код,
/// который никогда не выполнится.
///
/// Внутри — `stringify!`, а НЕ `format!`: в заданиях полно `{}` от
/// формат-строк и от синтаксиса Rust (`Vec { .. }`, `Some(x)`).
/// `format!` пытался бы их разворачивать и падал на этапе компиляции.
#[macro_export]
macro_rules! not_yet {
    ($($arg:tt)*) => {
        ::std::panic::panic_any($crate::harness::Todo(::std::stringify!($($arg)*).to_string()))
    };
}

pub struct Report {
    done: usize,
    total: usize,
}

impl Report {
    /// Запустить задание. Имя — то, что увидишь в отчёте.
    pub fn task(&mut self, name: &str, f: impl FnOnce()) {
        self.total += 1;

        // Глушим сообщение паники: без этого каждое незакрытое задание
        // печатает 3 строки backtrace-мусора.
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));

        let result = std::panic::catch_unwind(AssertUnwindSafe(f));

        std::panic::set_hook(prev);

        match result {
            Ok(()) => {
                self.done += 1;
                println!("   \u{2713} {name}");
            }
            Err(payload) => {
                if let Some(m) = payload.downcast_ref::<Todo>() {
                    // stringify! оставляет кавычки и пробелы — подчищаем.
                    let reason = m.0.trim().trim_matches('"');
                    println!("   \u{2717} {name}\n       \u{2190} НЕ СДЕЛАНО: {reason}\n");
                } else {
                    // Задание было написано, но упало с assert! — покажи это честно.
                    let text = payload
                        .downcast_ref::<&str>()
                        .map(|s| s.to_string())
                        .or_else(|| payload.downcast_ref::<String>().cloned())
                        .unwrap_or_else(|| "паника с неизвестным payload".into());
                    println!("   \u{2717} {name}\n       \u{2190} упало: {text}\n");
                }
            }
        }
    }

    pub fn summary(&self, module: u32) -> bool {
        println!("\n   ── Итог модуля {module:02}: {}/{} заданий ──", self.done, self.total);
        // ASCII-строка: удобно грепать из PowerShell/скриптов, когда
        // экранная кодировка не UTF-8 и русский текст не матчится.
        println!("   TASKS: {}/{} done", self.done, self.total);
        if self.done == self.total {
            println!("   Все задания закрыты. Можно идти дальше.");
            true
        } else {
            println!("   Не сделано: {}. Загляни в конец файла.", self.total - self.done);
            false
        }
    }
}

pub fn report() -> Report {
    println!();
    println!("\u{2550}\u{2550}\u{2550} ЗАДАНИЯ \u{2550}\u{2550}\u{2550}");
    Report { done: 0, total: 0 }
}

// ─────────────────────────────────────────────────────────────────────────────
// Микро-бенчмарк
// ─────────────────────────────────────────────────────────────────────────────

/// Замер: гоняет `f` `iters` раз, отдаёт статистику по времени.
///
/// Ключевая деталь — `std::hint::black_box`: компилятор имеет право
/// выбросить вычисление, результат которого никто не использует
/// (в C++ он для этого прячет указатель). `black_box` говорит LLVM:
/// «это значение может быть прочитано кем угодно, не оптимизируй».
/// Без него бенчмарк «замеряет» пустоту.
pub struct Bench {
    pub name: String,
    pub iters: u32,
    pub min_ns: f64,
    pub median_ns: f64,
    pub mean_ns: f64,
    pub allocs: usize,
    pub bytes: usize,
}

impl Bench {
    pub fn ns_per_iter(&self) -> f64 {
        self.min_ns
    }
}

pub fn bench<T>(name: &str, iters: u32, mut f: impl FnMut(u32) -> T) -> Bench {
    // Прогрев: раскладка кода, ленивые статики, аллокации кеша.
    for i in 0..(iters / 10).max(1) {
        std::hint::black_box(f(i));
    }

    let before = crate::alloc::snapshot();
    let mut samples: Vec<f64> = Vec::with_capacity(iters as usize);
    for i in 0..iters {
        let t = Instant::now();
        std::hint::black_box(f(i));
        samples.push(t.elapsed().as_nanos() as f64);
    }
    let after = crate::alloc::snapshot();

    samples.sort_by(|a, b| a.partial_cmp(b).expect("время не может быть NaN"));
    let sum: f64 = samples.iter().sum();
    Bench {
        name: name.to_string(),
        iters,
        min_ns: samples[0],
        median_ns: samples[samples.len() / 2],
        mean_ns: sum / samples.len() as f64,
        // Разница снимков — грубо, но для «аллокаций в цикле» сходится.
        allocs: after.allocs.saturating_sub(before.allocs),
        bytes: after.alloc_bytes.saturating_sub(before.alloc_bytes),
    }
}

impl fmt::Display for Bench {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:<34} min {:>9.1} ns | med {:>9.1} ns | alloc {:>5}",
            self.name, self.min_ns, self.median_ns, self.allocs
        )?;
        if self.allocs > 0 {
            write!(f, " ({} B)", self.bytes)?;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Диаграммы — визуально быстрее, чем куча `assert_eq!`
// ─────────────────────────────────────────────────────────────────────────────

/// Печатает распределение значений `0..=max` (для f32 приводит к 0..1).
pub fn histogram(values: &[f32], buckets: usize) {
    let mut counts = vec![0usize; buckets];
    for &v in values {
        let t = v.clamp(0.0, 1.0);
        let b = ((t * buckets as f32) as usize).min(buckets - 1);
        counts[b] += 1;
    }
    let max = counts.iter().copied().max().unwrap_or(1).max(1);
    for (i, c) in counts.iter().enumerate() {
        let bar = "█".repeat((c * 40 / max).max(if *c > 0 { 1 } else { 0 }));
        println!("   {:>5.2} | {bar:<40} {c}", i as f32 / buckets as f32);
    }
}

/// Маленькая таблица «сложность»: печатает замер сразу после описания.
pub fn show(b: &Bench) {
    println!("   {b}");
}
