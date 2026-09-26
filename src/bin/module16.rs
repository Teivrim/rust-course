//! ============================================================================
//! МОДУЛЬ 16 — ТЕСТИРОВАНИЕ И БЕНЧМАРКИ
//! ============================================================================
//!
//! ЧАСТЬ 16.1 — ТЕОРИЯ: виды тестов
//! ─────────────────────────────────────────────────────────────────────────────
//! ТРИ ВИДА, которые реально полезны:
//!
//!   1. Модульные (`#[cfg(test)] mod tests`) — рядом с кодом, видят
//!      приватные поля. Основа юнит-тестов.
//!   2. Интеграционные (`tests/*.rs`) — видят только публичный API.
//!      Именно они ловят «утечку приватности» и неправильный API.
//!      ⚠ Их НЕЛЬЗЯ положить в тот же файл: они компилируются как
//!      отдельный крейт, и это осознанное ограничение.
//!   3. Док-тесты (``` в `///`) — примеры в документации, которые
//!      компилируются и выполняются при `cargo test`. Лучшая
//!      документация — та, которая проверяется.
//!
//! ЧТО ТЕСТИРОВАТЬ В ДВИЖКЕ (в порядке ценности):
//!   1. Раскладку структур (задание 11.3) — ошибка молчаливая.
//!   2. Инварианты данных: длина буфера == n * size_of::<Vertex>().
//!   3. Парсеры: битый вход не должен паниковать.
//!   4. Математику: перспектива даёт w > 0, ортогональность сохраняется.
//!   5. Порядок уничтожения Vulkan-объектов (модуль 18).
//!
//! ЧЕГО НЕ ТЕСТИРОВАТЬ:
//!   * что `add` складывает — это тест для компилятора;
//!   * детали реализации, которые завтра изменятся;
//!   * тайминги ( flaky-тесты — хуже, чем их отсутствие).
//!
//! ДЕТЕРМИНИРОВАННОСТЬ. Тест, который падает один раз из ста,
//! хуже, чем неудачный тест: он учит тебя игнорировать красное.
//! Поэтому: фиксированный seed (модуль Curriculum/src/rng.rs),
//! никаких `SystemTime` в логике, никаких HashMap-итераций
//! (порядок не гарантирован) без сортировки перед assert'ом.
//!
//! `#[should_panic(expected = "...")]` — проверка, что паника ИМЕННО
//! такая. Ловит регрессии, когда вместо panic! ты случайно убрал
//! проверку.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;
use curriculum::rng::Rng;

fn main() {
    harness::module(16, "Тестирование и бенчмарки");

    part_16_1(); // assert-хелперы, детерминированный PRNG, should_panic
    part_16_2(); // property-based тестирование своими руками
    part_16_3(); // бенчмарки: зачем black_box, что мерить

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("16.1  Набор assert-хелперов и табличный тест парсера", task_16_1);
    r.task("16.2  Property-тесты: инварианты на случайных данных + shrink", task_16_2);
    r.task("16.3  Свой бенчмарк-харнесс и поиск узкого места", task_16_3);
    r.summary(16);
}

/// Модульный тест: живёт рядом с кодом, видит приватные поля.
/// Запускается через `cargo test --bin module16`.
#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn option_basics() {
        assert_eq!(Some(1).map(|x| x + 1), Some(2));
        assert_eq!(None::<i32>.unwrap_or_default(), 0);
    }

    #[test]
    fn panics_with_message() {
        let v: Vec<u8> = vec![];
        let r = std::panic::catch_unwind(|| v[0]);
        assert!(r.is_err(), "выход за границу — паника, а не UB");
    }

    /// `should_panic` проверяет, что паника ИМЕННО с ожидаемым текстом.
    /// Это ловит регрессию «случайно убрали assert!».
    #[test]
    #[should_panic(expected = "граница")]
    fn should_panic_on_zero_chunk() {
        let n = 0usize;
        assert!(n > 0, "граница: нельзя делить на ноль");
    }

    #[test]
    fn rng_is_deterministic() {
        let a: Vec<u32> = { let mut r = Rng::new(42); (0..5).map(|_| r.next_u32()).collect() };
        let b: Vec<u32> = { let mut r = Rng::new(42); (0..5).map(|_| r.next_u32()).collect() };
        assert_eq!(a, b, "один seed → одна последовательность");
        let c: Vec<u32> = { let mut r = Rng::new(43); (0..5).map(|_| r.next_u32()).collect() };
        assert_ne!(a, c, "разный seed → разная последовательность");
    }

    #[test]
    fn rng_below_is_in_range() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            assert!(r.below(10) < 10);
        }
    }
}

fn part_16_1() {
    harness::part("16.1", "ПРИМЕР: assert-хелперы и детерминированность");

    // --- Хелпер, который читается как предложение --------------------------
    check_i32("сумма пустого массива", 0, sum_i32(&[]));
    check_i32("сумма 1..=10", 55, sum_i32(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]));
    // Эта проверка УПАЛА бы с информативным сообщением, а не просто "assert failed".
    check_i32("намеренно неверный результат", 999, sum_i32(&[1, 2, 3]));

    // --- Детерминированность -------------------------------------------------
    let a: Vec<u32> = { let mut r = Rng::new(12345); (0..8).map(|_| r.next_u32()).collect() };
    let b: Vec<u32> = { let mut r = Rng::new(12345); (0..8).map(|_| r.next_u32()).collect() };
    assert_eq!(a, b);
    println!("  один seed → одинаковая последовательность: {a:?}");
    let histogram = { let mut r = Rng::new(1); (0..10_000).map(|_| r.f32()).collect::<Vec<_>>() };
    harness::histogram(&histogram, 10);
    println!("  ⚠ если бы seed был временем, тест падал бы иногда. Фиксируй seed.");

    // --- Порядок итерации не гарантирован ------------------------------------
    use std::collections::HashMap;
    let mut m: HashMap<u32, u32> = HashMap::new();
    for i in 0..5u32 {
        m.insert(i, i * i);
    }
    let mut keys: Vec<u32> = m.keys().copied().collect();
    keys.sort_unstable(); // ⚠ ОБЯЗАТЕЛЬНО перед assert_eq!
    println!("  ключи HashMap после сортировки: {keys:?} (порядок итерации не гарантирован)");
}

fn check_i32(what: &str, expected: i32, got: i32) {
    if expected == got {
        println!("  ✓ {what}: {got}");
    } else {
        println!("  ✗ {what}: ожидалось {expected}, получено {got}");
        panic!("{what}: ожидалось {expected}, получено {got}");
    }
}

fn sum_i32(v: &[i32]) -> i32 {
    v.iter().sum()
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 16.2 — ТЕОРИЯ: property-based тестирование
// ─────────────────────────────────────────────────────────────────────────────
//
// ПРИМЕРНЫЕ ТЕСТЫ проверяют конкретный случай. PROPERTY-тест проверяет
// СВОЙСТВО на тысячах случайных:
//
//   property: normalize(v) даёт единичную длину
//   property: encode(decode(x)) == x
//   property: sort(v) отсортирован и содержит те же элементы
//
// Реальный `proptest` делает так: генерирует значение, если тест упал —
// пытается УМЕНЬШИТЬ (shrink) его до минимального, и печатает именно
// минимальный контрпример. Это в 10 раз полезнее обычного теста,
// потому что ты сразу видишь минимальный случай, а не «Random(983472)».
//
// МЫ НАПИШЕМ СВОЮ УПРОЩЁННУЮ ВЕРСИЮ: генератор (Rng из Curriculum),
// набор проверяемых свойств, и shrink для чисел/строк.
//
// ГЛАВНОЕ ПРАВИЛО: свойство должно быть ПРОВЕРЯЕМЫМ и ЗАВИСИМЫМ от данных,
// а не от реализации. «Результат равен сумме входа» — плохое свойство
// (оно просто повторяет определение). «Длина нормализованного вектора
// равна 1 с точностью до 1e-5» — хорошее.
//
// ЧТО ИМЕННО ТЕСТИРОВАТЬ В ГРАФИКЕ (это твоя специфика):
//   * нормализация: |v| == 1;
//   * перспектива: точка перед камерой даёт w > 0;
//   * mul_mat_vec: v * M * M.inverse() == v;
//   * кодирование текста: байты UTF-8 всегда декодируются;
//   * парсер: любой вход → Ok(_) или Err(_), но НЕ паника (fuzz-lite);
//   * индексация: любой id в пределах массива даёт верный элемент.

fn part_16_2() {
    harness::part("16.2", "ПРИМЕР: свойства, fuzz-lite, shrink");

    // --- Свойство: нормализация даёт единичную длину -----------------------
    let mut r = Rng::new(99);
    let mut worst = 0.0f32;
    for i in 0..2000 {
        let v = [r.range_f32(-10.0, 10.0), r.range_f32(-10.0, 10.0), r.range_f32(-10.0, 10.0)];
        let n = normalize(v);
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        let err = (len - 1.0).abs();
        if err > worst {
            worst = err;
        }
        if err > 1e-5 && len > 0.0 {
            println!("  ✗ нормализация сломалась на {v:?} → {n:?} (len={len})");
            break;
        }
        if i == 0 {
            println!("  первый вектор {v:?} → {n:?} (len={len:.6})");
        }
    }
    println!("  2000 случайных векторов: максимальная ошибка {worst:.2e}");

    // --- Свойство: нулевой вектор не ломает нормализацию -------------------
    let z = normalize([0.0, 0.0, 0.0]);
    assert!(z.iter().all(|c| c.is_finite()), "нулевой вектор → конечный результат, не NaN");
    println!("  нулевой вектор → {z:?} (не NaN!)");

    // --- Fuzz-lite: random strings must not panic ---
    let mut r = Rng::new(4242);
    let mut crashes: usize = 0;
    for _ in 0..5000 {
        let mut s = String::new();
        let n = r.below(64) as usize;
        r.ascii(&mut s, n);
        // РґРѕР±Р°РІР»СЏРµРј Рё РјСѓСЃРѕСЂРЅС‹Рµ Р±Р°Р№С‚С‹
        if r.below(4) == 0 {
            s.push('\u{FFFD}');
        }
        if s.contains('\0') {
            s = s.replace('\0', " ");
        }
        match std::panic::catch_unwind(|| parse_kv_line(&s)) {
            Ok(_) => {}
            Err(_) => crashes += 1,
        }
    }
    println!("  fuzz-lite: 5000 random strings, panics: {crashes}");

    // --- SHRINK: уменьшаем контрпример --------------------------------------
    // Сценарий: нашли падающий вход, «сжимаем» до минимального.
    let bad = find_first_failing_seed(2000);
    match bad {
        Some(seed) => {
            let shrunk = shrink_counterexample(seed);
            println!("  найден падающий seed {seed} → после shrink: {shrunk:?}");
        }
        None => println!("  падающих seed не найдено (функция корректна)"),
    }
}

fn normalize(v: [f32; 3]) -> [f32; 3] {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > f32::EPSILON {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0, 0.0, 0.0]
    }
}

fn parse_kv_line(line: &str) -> Result<(&str, &str), String> {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') {
        return Err("пусто или комментарий".into());
    }
    match line.split_once('=') {
        Some((k, v)) if !v.trim().is_empty() => Ok((k.trim(), v.trim())),
        _ => Err("нет значения".into()),
    }
}

/// Имитация поиска контрпримера: перебираем seed'ы.
fn find_first_failing_seed(n: u32) -> Option<u32> {
    (0..n).find(|&seed| {
        let mut r = Rng::new(seed as u64);
        let v = [r.range_f32(-1.0, 1.0); 3];
        normalize(v).iter().any(|c| !c.is_finite())
    })
}

/// SHRINK по целым: начиная с seed, пробуем всё меньшие значения,
/// пока ошибка воспроизводится. Классический алгоритм Proptest,
/// упрощенный до одного измерения.
fn shrink_counterexample(seed: u32) -> Option<u32> {
    let fails = |s: u32| {
        let mut r = Rng::new(s as u64);
        let v = [r.range_f32(-1.0, 1.0); 3];
        normalize(v).iter().any(|c| !c.is_finite())
    };
    if !fails(seed) {
        return None;
    }
    let mut best = seed;
    loop {
        // Делим пополам: так контрпример сжимается быстро.
        let half = best / 2;
        if half == 0 || !fails(half) {
            return Some(best);
        }
        best = half;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 16.3 — ТЕОРИЯ: бенчмарки
// ─────────────────────────────────────────────────────────────────────────────
//
// ГЛАВНОЕ ПРАВИЛО БЕНЧМАРКИНГА В RUST: `std::hint::black_box`.
//
// Компилятор имеет полное право удалить вычисление, результат которого
// никто не читает. Без `black_box` бенчмарк «замеряет» пустоту:
//
// ```ignore
// let t = Instant::now();
// for i in 0..N { sum += i; }   // sum не используется → цикл удалён целиком
// println!("{:?}", t.elapsed());   // 0 ns, «мы очень быстрые»
// ```
//
// `black_box(x)` говорит LLVM: «это значение может быть прочитано
// кем угодно, не оптимизируй». В C++ для того же прячут указатель
// или пишут в `volatile` (что, наоборот, ЗАПРЕЩАет оптимизацию целиком
// и даёт неверные цифры).
//
// ЧТО МЕРИТЬ (в порядке полезности):
//   1. Количество АЛЛОКАЦИЙ (модуль 13). Число, не время.
//   2. Время горячей функции на РЕАЛЬНЫХ данных нужного размера.
//   3. Время на данных в 10 раз меньше, чем в проде (кэш был бы тёплый).
//
// ЧЕГО НЕ МЕРИТЬ:
//   * на данных, помещающихся в L1 (100 байт) — в проде их миллион;
//   * «среднее по 100 прогонам» — лучше минимум (минимум = «сколько
//     стоит без помех»);
//   * в debug-режиме. У нас dev имеет opt-level 1, это минимум приемлемо;
//     для точных цифр — release.
//
// СТАТИСТИКА: `criterion` считает среднее, медиану, отклонение и
// сравнивает с базой. Мы используем `harness::bench` — он берёт
// минимум и медиану, этого достаточно для учебных целей.

fn part_16_3() {
    harness::part("16.3", "ПРИМЕР: black_box, аллокации, размер данных");

    // --- Без black_box компилятор съедает цикл ------------------------------
    let t0 = std::time::Instant::now();
    let mut sum = 0u64;
    for i in 0..10_000_000u64 {
        sum = sum.wrapping_add(i);
    }
    let naive = t0.elapsed();
    std::hint::black_box(sum);
    println!("  10M итераций: {naive:?} (результат прочитан через black_box — цикл остался)");

    // --- С аллокациями: три варианта -----------------------------------------
    let n = 200_000usize;

    let b_plain = harness::bench("Vec::new + push (без capacity)", 20, |_| {
        let mut v: Vec<u64> = Vec::new();
        for i in 0..n as u64 {
            v.push(i);
        }
        std::hint::black_box(v.len())
    });
    let b_cap = harness::bench("Vec::with_capacity + push", 20, |_| {
        let mut v: Vec<u64> = Vec::with_capacity(n);
        for i in 0..n as u64 {
            v.push(i);
        }
        std::hint::black_box(v.len())
    });
    harness::show(&b_plain);
    harness::show(&b_cap);
    println!("  with_capacity быстрее в {:.2}x и делает 1 аллокацию вместо ~18",
        (b_plain.min_ns / b_cap.min_ns.max(1.0)).max(1.0));

    // --- Размер данных меняет всё --------------------------------------------
    for &size in &[1_000usize, 100_000, 1_000_000] {
        let v: Vec<u32> = (0..size as u32).collect();
        let b = harness::bench(&format!("sum, size={size}"), 20, |_| {
            std::hint::black_box(v.iter().map(|x| *x as u64).sum::<u64>())
        });
        harness::show(&b);
    }
    println!("  ⚠ при разных размерах разная пропускная способность (кэш).");
    println!("    Меряй на том размере, который будет в проде.");

    // --- Считаем аллокации рядом со временем ---------------------------------
    let (_, allocs, bytes) = curriculum::alloc::measure(|| {
        let mut v: Vec<u64> = Vec::new();
        for i in 0..n as u64 {
            v.push(i);
        }
    });
    println!("  Vec без capacity на {n} элементов: {allocs} аллокаций, {bytes} байт");
    let (_, a2, b2) = curriculum::alloc::measure(|| {
        let mut v: Vec<u64> = Vec::with_capacity(n);
        for i in 0..n as u64 {
            v.push(i);
        }
    });
    println!("  Vec с capacity:                  {a2} аллокаций, {b2} байт");
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 16.4 — ЗАДАНИЕ 16.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Хелперы для тестов. В C++ это `EXPECT_EQ` / `ASSERT_TRUE` из gtest.
/// В Rust своего фреймворка нет, но нужное есть в std, и писать своё
/// стоит только когда не хватает удобства.
///
/// ```ignore
/// fn check_eq<T: PartialEq + std::fmt::Debug>(what: &str, expected: T, got: T);
/// fn check<T: PartialEq + std::fmt::Debug>(what: &str, cond: T);
/// fn check_close(what: &str, expected: f32, got: f32, eps: f32);
/// fn check_err<T: std::fmt::Debug, E: std::fmt::Debug>(what: &str, r: Result<T, E>);
/// ```
///
/// Требования:
///   * `check_eq` при несовпадении печатает ОБА значения и паникует
///     с информативным сообщением (не `assert_eq!` — его сообщение
///     короткое).
///   * `check_close` допускает NaN==NaN (это нужно для float-тестов!)
///     и проверяет бесконечности отдельно.
///   * `check_err` требует, чтобы результат был `Err`, и печатает `Ok`.
///   * Напиши `#[test]`-ы на всё. Проверь, что `cargo test --bin module16`
///     их находит, и что при故意 сломанной проверке падает именно она.
///   * Добавь `mod unit_tests` с 3 тестами и одним `#[should_panic]`.
fn task_16_1() {
    fn check_eq<T: PartialEq + std::fmt::Debug>(what: &str, expected: T, got: T) {
        not_yet!("сравни, напечатай both и panic с текстом");
    }

    fn check<T: PartialEq + std::fmt::Debug>(what: &str, cond: T) {
        not_yet!("negate, напечатай и panic");
    }

    fn check_close(what: &str, expected: f32, got: f32, eps: f32) {
        not_yet!("учитывай NaN и бесконечности");
    }

    fn check_err<T: std::fmt::Debug, E: std::fmt::Debug>(what: &str, r: Result<T, E>) {
        not_yet!("ожидай Err; если Ok — panic с перепечатанным Ok");
    }

    check_eq("2+2", 4, 2 + 2);
    check("true", true);
    check_close("1/3", 0.333, 1.0 / 3.0, 0.001);
    check_close("NaN == NaN (важно для float-тестов)", f32::NAN, f32::NAN, f32::EPSILON);
    check_close("бесконечности", f32::INFINITY, f32::INFINITY, 0.0);
    check_err::<u32, _>("str::parse::<u32>()", "not_a_number".parse::<u32>());

    // ⚠ А вот это ДОЛЖНО упасть. Раскомментируй и убедись:
    // check_eq("намеренно", 999, 1);

    #[cfg(test)]
    mod my_tests {
        use super::*;

        #[test]
        fn parsers_never_panic() {
            use curriculum::rng::Rng;
            let mut r = Rng::new(1);
            for _ in 0..1000 {
                let mut s = String::new();
                r.ascii(&mut s, r.below(32) as usize);
                let _ = crate::parse_kv_line(&s);
            }
        }

        #[test]
        #[should_panic(expected = "граница")]
        fn panic_has_expected_message() {
            let n = 0usize;
            assert!(n > 0, "граница: нельзя");
        }
    }
}

/// ЧАСТЬ 16.5 — ЗАДАНИЕ 16.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Мини-фреймворк property-тестов. Настоящий `proptest` делает
/// примерно это, плюс автоматический shrink произвольных типов.
///
/// ```ignore
/// trait Check { fn check(&self) -> Result<(), String>; }
///
/// fn check_property<F>(name: &str, seed: u64, trials: u32, f: F) -> bool
/// where F: Fn(&mut Rng) -> Result<(), String>;
///
/// fn shrink(mut candidate: Vec<u8>, fails: impl Fn(&[u8]) -> bool) -> Vec<u8>;
/// ```
///
/// Требования:
///   * `check_property` печатает `seed`, если нашла падение, и
///     возвращает `false` (тест упал — по return false его поймает
///     assert в main).
///   * `shrink` уменьшает вектор: пробует удалить половину элементов,
///     пока длина не перестанет уменьшаться, а потом пробует занулить
///     каждый байт по очереди. Максимум 100 итераций.
///   * Свойства для проверки:
///     1. `normalize(v)` всегда даёт конечные координаты;
///     2. `sum` коммутативен: sum(a) == sum(rev(a));
///     3. `parse_kv_line` никогда не паникует (fuzz-lite);
///     4. `sort_unstable` даёт неубывающую последовательность.
///   * Прогони 5000 trials на каждом свойстве и напечатай, сколько
///     контрпримеров нашлось (ожидается 0).
fn task_16_2() {
    use curriculum::rng::Rng;

    fn normalize(v: [f32; 3]) -> [f32; 3] {
        let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
        if len > f32::EPSILON {
            [v[0] / len, v[1] / len, v[2] / len]
        } else {
            [0.0, 0.0, 0.0]
        }
    }

    fn check_property<F>(name: &str, seed: u64, trials: u32, f: F) -> bool
    where
        F: Fn(&mut Rng) -> Result<(), String>,
    {
        not_yet!("перебери seed..seed+trials, на печатай падение, верни false");
    }

    fn shrink(candidate: Vec<u8>, fails: impl Fn(&[u8]) -> bool) -> Vec<u8> {
        not_yet!("удаляй половины, потом зануляй байты, максимум 100 итераций");
    }

    // --- Свойство 1: нормализация конечна -----------------------------------
    let ok1 = check_property("normalize конечна", 100, 5000, |r| {
        let v = [r.range_f32(-1e3, 1e3), r.range_f32(-1e3, 1e3), r.range_f32(-1e3, 1e3)];
        let n = normalize(v);
        if n.iter().all(|c| c.is_finite()) {
            Ok(())
        } else {
            Err(format!("{v:?} -> {n:?}"))
        }
    });
    assert!(ok1, "свойство 1 нарушено");

    // --- Свойство 2: сумма коммутативна --------------------------------------
    let ok2 = check_property("sum коммутативен", 200, 5000, |r| {
        let v: Vec<u32> = (0..r.below(64)).map(|_| r.next_u32()).collect();
        let a: u64 = v.iter().map(|x| *x as u64).sum();
        let b: u64 = v.iter().rev().map(|x| *x as u64).sum();
        if a == b { Ok(()) } else { Err(format!("{a} != {b}")) }
    });
    assert!(ok2, "свойство 2 нарушено");

    // --- Свойство 3: парсер не паникует --------------------------------------
    let ok3 = check_property("парсер не паникует", 300, 5000, |r| {
        let mut s = String::new();
        let n = r.below(48) as usize;
        r.ascii(&mut s, n);
        let _ = crate::parse_kv_line(&s);
        Ok(())
    });
    assert!(ok3, "свойство 3 нарушено");

    // --- Свойство 4: sort даёт неубывающую ----------------------------------
    let ok4 = check_property("sort неубывающая", 400, 5000, |r| {
        let mut v: Vec<u32> = (0..r.below(128)).map(|_| r.next_u32() % 1000).collect();
        v.sort_unstable();
        if v.windows(2).all(|w| w[0] <= w[1]) { Ok(()) } else { Err(format!("не отсортировано: {v:?}")) }
    });
    assert!(ok4, "свойство 4 нарушено");

    // --- Демонстрация shrink -------------------------------------------------
    // Искусственно сломанная функция: падает на байте == 200.
    let broken = |b: &[u8]| b.contains(&200u8);
    let bad: Vec<u8> = vec![7, 7, 200, 7, 7, 7, 7];
    let small = shrink(bad, broken);
    assert!(broken(&small));
    assert_eq!(small.len(), 1, "shrink должен минимизировать до одного байта");
    println!("  shrink: 7 байт → {} байт: {small:?}", small.len());
}

/// ЧАСТЬ 16.6 — ЗАДАНИЕ 16.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Найти узкое место на реальных данных. Смысл задания — не «написать
/// бенчмарк», а научиться отличать проблему аллокаций от проблемы
/// доступа к памяти.
///
/// Три версии функции суммы квадратов, замер на 500 000 элементов
/// (данные не помещаются в L2 = 512 КБ):
///
/// ```ignore
/// fn sum_sq_plain(v: &[u32]) -> u64;
/// fn sum_sq_soa(pos: &[f32], other: &[f32]) -> f64;  // то же, но SoA
/// fn sum_sq_unrolled(v: &[u32]) -> u64;              // 4 аккумулятора
/// ```
///
/// Требования:
///   * Замерь ВСЕ три через `harness::bench` (минимум 50 итераций).
///   * Для `sum_sq_soa` создай 500k `f32` позиций и 500k мусорных
///     данных, и покажи, что чтение одного массива быстрее чтения
///     структуры с лишним полем (переиспользуй вывод модуля 13).
///   * Замерь количество аллокаций всех трёх (все должны быть 0 —
///     и это тоже результат).
///   * Напиши в комментарии, КАКОЕ из трёх улучшений даст больше
///     всего в реальном кадре, и почему.
///   * Бонус: `fn mean_abs_diff(a: &[f32], b: &[f32]) -> f64` — и
///     проверь `black_box`, что компилятор не выбросил цикл.
fn task_16_3() {
    const N: usize = 500_000;

    fn sum_sq_plain(v: &[u32]) -> u64 {
        not_yet!("iter().map(|x| *x as u64 * *x as u64).sum()");
    }

    fn sum_sq_soa(pos: &[f32], other: &[f32]) -> f64 {
        not_yet!("сумма квадратов только по pos; other читается мимо (для сравнения)");
    }

    fn sum_sq_unrolled(v: &[u32]) -> u64 {
        not_yet!("4 независимых аккумулятора, разворачивание вручную");
    }

    struct VertexLike {
        pos: [f32; 3],
        uv: [f32; 2],
        color: [u8; 4],
    }

    let data: Vec<u32> = (0..N as u32).map(|i| (i % 1000) as u32).collect();
    let pos: Vec<f32> = (0..N).map(|i| (i % 977) as f32).collect();
    let other: Vec<f32> = vec![0.0; N];
    let verts: Vec<VertexLike> = (0..N)
        .map(|i| VertexLike { pos: [(i % 977) as f32, 1.0, 2.0], uv: [0.0, 0.0], color: [0; 4] })
        .collect();

    let expected = sum_sq_plain(&data);
    assert_eq!(sum_sq_unrolled(&data), expected);
    assert!((sum_sq_soa(&pos, &other) - expected as f64).abs() < 1e-3);

    let b1 = harness::bench("sum_sq_plain (500k u32)", 50, |_| sum_sq_plain(&data));
    let b2 = harness::bench("sum_sq_unrolled (500k u32)", 50, |_| sum_sq_unrolled(&data));
    let b3 = harness::bench("sum_sq_soa (500k f32)", 50, |_| sum_sq_soa(&pos, &other));
    let b4 = harness::bench("AoS: только pos, 32 б/вершину", 50, |_| {
        verts.iter().map(|v| v.pos[0] as u64 * v.pos[0] as u64).sum::<u64>()
    });
    harness::show(&b1);
    harness::show(&b2);
    harness::show(&b3);
    harness::show(&b4);

    let (_, a1, _) = curriculum::alloc::measure(|| { std::hint::black_box(sum_sq_plain(&data)); });
    let (_, a2, _) = curriculum::alloc::measure(|| { std::hint::black_box(sum_sq_soa(&pos, &other)); });
    println!("  аллокации: plain={a1} soa={a2} (ожидается 0 и 0)");
    assert_eq!(a1, 0);
    assert_eq!(a2, 0);

    // ⚠ Без black_box этот цикл компилятор удалит целиком:
    // let _ = { let mut s = 0.0; for p in &pos { s += *p; } };
    let measured = {
        let mut s = 0.0f64;
        for p in &pos {
            s += *p as f64;
        }
        s
    };
    std::hint::black_box(measured);
}
