//! ============================================================================
//! МОДУЛЬ 09 — ЗАМЫКАНИЯ, ИТЕРАТОРЫ, ФУНКЦИОНАЛЬНЫЙ СТИЛЬ
//! ============================================================================
//!
//! ЧАСТЬ 9.1 — ТЕОРИЯ: замыкания и три вида захвата
//! ─────────────────────────────────────────────────────────────────────────────
//! Замыкание `|x| x + 1` — это анонимная функция. Может захватывать
//! переменные из окружения, и СПОСОБ захвата определяется тем, что
//! замыкание делает с ними. Rust проверяет это на этапе компиляции
//! и выбирает один из трёх трейтов:
//!
//!   FnOnce  — забрать захваченное ПО ССЫЛКЕ (move). Можно вызвать ОДИН раз.
//!   FnMut   — изменить захваченное по ссылке (`&mut`). Много раз.
//!   Fn      — только читать (`&`). Много раз, и можно передавать
//!             другим потокам (требования выше, чем у FnMut).
//!
//! ⚠ ВАЖНО: это НЕ НАСЛЕДОВАНИЕ. Трейты Fn* НЕ связаны отношением
//! подтипа. Каждый замыкание реализует ровно те из них, которые может.
//! `Fn: FnMut: FnOnce` верно только в смысле «может использоваться как».
//!
//! СРАВНИ С C++: лямбда в C++ захватывает либо по значению, либо по
//! ссылке — синтаксисом `[&]`/`[=]`. В Rust выбор делает компилятор по
//! фактическому использованию, и `move` заставляет его копировать всё.
//! Это убирает целый класс ошибок «случайно захватил по ссылке».
//!
//! `move` — модификатор: «захватить всё по значению». Обязателен,
//! когда замыкание уходит в поток или живёт дольше кадра стека:
//!
//! ```ignore
//! let v = vec![1, 2, 3];
//! std::thread::spawn(move || println!("{v:?}"));   // без move — E0373
//! ```
//!
//! ⚠ `move` НЕ означает «глубокое копирование». `move || v` с `v: Vec`.
//!   захватит сам дескриптор Vec (3 слова), а данные останутся в куче.
//!   Дёшево. С `String` — то же самое.
//!
//! ХРАНЕНИЕ ЗАМЫКАНИЙ В СТРУКТУРЕ — НЕЛЬЗЯ без параметра типа:
//!   struct Middleware<F> { f: F }      // generic — бесплатно, инлайн
//!   struct Middleware { f: Box<dyn Fn(..)> }  // dyn — гибко, но с vtable
//! В движке: use-case узкие → generic; набор плагинов известен в
//! рантайме → Box<dyn>.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, bench, report, show};
use curriculum::not_yet;

fn main() {
    harness::module(9, "Замыкания, итераторы, функциональный стиль");

    part_9_1(); // три вида захвата, move, хранение в структуре
    part_9_2(); // ленивые итераторы и пайплайны
    part_9_3(); // fold/scan/peekable + СВОЙ итератор

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("9.1  Конвейер middleware на трех замыканиях", task_9_1);
    r.task("9.2  Переписать императивный код на итераторы (и замерить)", task_9_2);
    r.task("9.3  Свой итератор: Window с внутренним буфером", task_9_3);
    r.summary(9);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 9.1 — ПРИМЕР
// ─────────────────────────────────────────────────────────────────────────────
fn part_9_1() {
    harness::part("9.1", "ПРИМЕР: захваты, move, хранение замыканий");

    // --- Fn: только читает --------------------------------------------------
    let base = 10;
    let add_base = |x: i32| x + base; // захватил &base, компилятор выбрал Fn
    println!("  Fn    : add_base(5) = {}", add_base(5));
    assert_eq!(add_base(1), add_base(1), "Fn можно звать сколько угодно");

    // --- FnMut: изменяет захваченное ---------------------------------------
    let mut count = 0;
    let mut bump = || {
        count += 1;
        count
    };
    println!("  FnMut : bump() = {}, bump() = {}", bump(), bump());
    assert_eq!(count, 2);

    // --- FnOnce: забирает по значению, один раз ----------------------------
    let name = String::from("vulkan");
    let consume = move || name; // move-захват
    let got = consume();
    println!("  FnOnce: забрал {got:?}, второй вызов невозможен (тип сдвинут)");
    // let got2 = consume();  // ⛔ E0382: use of moved value

    // --- move не значит «глубокая копия» ----------------------------------
    // Массив — Copy, поэтому его можно «переместить» и не потерять.
    // Данные лежат на стеке, и мы видим, что адрес НЕ изменился.
    let data: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let ptr_before = data.as_ptr();
    let print_len = move || data.len();
    println!("  move || data.len() = {}, адрес массива {:p} (был {:p}) — тот же",
        print_len(), data.as_ptr(), ptr_before);
    println!("    Для Vec было бы то же самое: move забирает ДЕСКРИПТОР (3 слова), не данные.");
    println!("    Именно поэтому `move` в спавн потока дешёвый.");

    // А вот настоящий move — Vec:
    let owned = vec![1u8, 2, 3];
    let take_len = move || owned.len();
    assert_eq!(take_len(), 3);
    // owned больше не существует: ⛔ E0382 если обратиться

    // --- Хранение в структуре: generic vs dyn ------------------------------
    let doubler = Double { f: |x: u32| x * 2 }; // static dispatch, инлайн
    let tripler = Triple { f: |x: u32| x * 3 };
    println!("  struct+generic: 4*2={} 4*3={}", doubler.apply(4), tripler.apply(4));

    let f: Box<dyn Fn(u32) -> u32> = Box::new(|x| x + 100);
    println!("  Box<dyn Fn>: 4+100 = {}", f(4));

    // --- Замыкание, возвращающее замыкание ---------------------------------
    let adder = make_adder(10);
    println!("  замыкание из замыкания: adder(5) = {}", adder(5));

    // --- Порядок drop у захватов -------------------------------------------
    println!("  (порядок drop см. ниже: захваты живут до конца замыкания)");
    {
        let _a = Named("outer");
        let _f = move || println!("    замыкание живёт дольше? нет — _a умрёт перед main");
    }

    println!("\n  --- порядок уничтожения захватов ---");
    struct Named(&'static str);
    impl Drop for Named {
        fn drop(&mut self) {
            println!("    drop {}", self.0);
        }
    }
    {
        let _x = Named("x");
        let _y = Named("y");
        let _z = Named("z");
    } // порядок: z, y, x — обратный объявлению

    // --- Передача замыкания в функцию ---------------------------------------
    // ⚠ ЛОВУШКА: нельзя передать `&mut log` И замыкание, которое тоже
    //   захватывает `log` — это два изменяющих займа на одно место (E0499).
    //   Правильный API: замыкание не знает про log, пишет наружу.
    let mut log = Vec::new();
    with_logger(&mut log, |msg| {
        println!("    [log] {msg}");
    });
    println!("  with_logger собрал: {log:?}");

    // --- Замыкание должно быть Fn для double_ended -----------------------
    // let pair = (1..=10).rev().map(|x| x * 2).sum::<i32>();
    let sum: i32 = (1..=100).filter(|x| x % 3 == 0).sum();
    println!("  сумма кратных 3 до 100 = {sum}");
}

struct Double<F> {
    f: F,
}

impl<F: Fn(u32) -> u32> Double<F> {
    fn apply(&self, x: u32) -> u32 {
        (self.f)(x)
    }
}

struct Triple<F> {
    f: F,
}

impl<F: Fn(u32) -> u32> Triple<F> {
    fn apply(&self, x: u32) -> u32 {
        (self.f)(x)
    }
}

/// Фабрика замыканий: принимает значение, возвращает замыкание.
fn make_adder(n: i32) -> impl Fn(i32) -> i32 {
    move |x| x + n // `impl Trait` в возврате скрывает тип
}

fn with_logger(log: &mut Vec<String>, write: impl Fn(&str)) {
    write("старт");
    write("стоп");
    log.push("-- конец --".into());
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 9.2 — ТЕОРИЯ: итераторы
// ─────────────────────────────────────────────────────────────────────────────
//
// Итератор в Rust — ЛЕНИВЫЙ. `map` не делает ничего, пока ты не
// «потянешь» результат через `collect`, `for`, `sum`.
//
// Почему ленивость — нормально для горячего кода, а не накладные
// расходы: компилятор склеивает цепочку адаптеров в один цикл.
// `(0..n).map(f).filter(g).sum()` превращается в один for с двумя
// условиями. Ноль аллокаций, ноль вызовов функций. Это и есть
// «zero-cost abstraction». Проверяется в модуле 13 замерами.
//
// ⚠ ЛОВУШКА: если вставить `.collect::<Vec<_>>()` в середину цепочки
//   ради `debug_assert!` — инлайн ломается, и в release будет медленнее.
//
// ТРИ СПОСОБА ОБОЙТИ, и они НЕ взаимозаменяемы:
//
//   for x in &v     → x: &T,   v не тронута
//   for x in &mut v → x: &mut T, v изменяется
//   for x in v      → x: T,    v СЪЕДЕНА (это for (auto x : v) из C++
//                              с копированием, но с move)
//
// `iter()` vs `&v` — то же самое, просто явнее. `iter_mut()` vs `&mut v`.
//
// `by_ref` — «обойти, не забирая владение». Нужен, когда внутри
// цикла ты хочешь передать `&mut` той же коллекции в другую функцию:
//
// ```ignore
// let mut v = vec![1, 2, 3];
// for x in v.by_ref().take(2) {
//     v.push(*x);   // ⛔ push требует &mut v, а x держит заём
// }
// ```

fn part_9_2() {
    harness::part("9.2", "ПРИМЕР: итераторные пайплайны на живых данных");

    // Данные «кадра»: видимость объектов сцены.
    let frame: Vec<(u32, f32)> = (0..12)
        .map(|i| (i, 100.0 - i as f32 * 7.0)) // id, distance
        .collect();

    // --- Фильтрация + маппинг + collect ------------------------------------
    let visible: Vec<u32> = frame.iter().filter(|(_, d)| *d < 50.0).map(|(id, _)| *id).collect();
    println!("  видны (d < 50): {visible:?}");

    // --- Сортировка по ключу из кортежа ------------------------------------
    let mut sorted = frame.clone();
    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).expect("f32 не NaN"));
    println!("  по расстоянию: {:?}", sorted.iter().take(3).collect::<Vec<_>>());

    // --- fold: свёртка --------------------------------------------------
    let total_dist: f32 = frame.iter().map(|(_, d)| d).sum();
    let max_d = frame.iter().map(|(_, d)| *d).fold(f32::MIN, |a, b| a.max(b));
    let (near, far) = frame.iter().fold((f32::MAX, f32::MIN), |(n, f), (_, d)| (n.min(*d), f.max(*d)));
    println!("  sum={total_dist:.1} max={max_d:.1} near={near:.1} far={far:.1}");

    // --- enumerate, zip, chain ---------------------------------------------
    let numbered: Vec<String> = frame.iter().take(3).enumerate().map(|(i, (id, d))| format!("#{i}:{id}@{d:.0}")).collect();
    println!("  enumerate: {numbered:?}");

    let names = ["pos", "nrm", "uv"];
    let zipped: Vec<(&str, u32)> = names.iter().copied().zip(0..3u32).collect();
    println!("  zip: {zipped:?}");

    let joined: Vec<u8> = vec![1, 2].into_iter().chain(vec![3, 4]).collect();
    println!("  chain: {joined:?}");

    // --- window / chunks / scan --------------------------------------------
    // ⚠ scan: `d` уже f32 (мы разыменовали на map), поэтому `*d` здесь
    //   было бы ошибкой. Типы в цепочке меняются на каждом адаптере.
    let running_max: Vec<f32> = frame.iter().map(|(_, d)| *d).scan(f32::MIN, |m, d| {
        *m = m.max(d);
        Some(*m)
    }).collect();
    println!("  scan (running max): {running_max:?}");

    // --- partition / group ---------------------------------------------------
    // partition возвращает (Vec<Item>, Vec<Item>). Item здесь — &(u32,f32),
    // поэтому сначала копируем в кортежи, иначе получим Vec<&(u32, f32)>.
    let (near, far): (Vec<(u32, f32)>, Vec<(u32, f32)>) = frame
        .iter()
        .copied()
        .partition(|(_, d)| *d < 50.0);
    println!("  partition: near={near:?} far={far:?}");

    // --- collect в разные контейнеры ---------------------------------------
    let as_vec: Vec<u32> = frame.iter().map(|(id, _)| *id).collect();
    let as_string: String = frame.iter().map(|(id, _)| id.to_string()).collect::<Vec<_>>().join(",");
    let as_bool: Vec<bool> = frame.iter().map(|(_, d)| *d < 50.0).collect();
    let (evens, odds): (Vec<u32>, Vec<u32>) = (0..5u32).partition(|x| x % 2 == 0);
    println!("  collect: vec={} шт, string={as_string:?}, bool={}", as_vec.len(), as_bool.len());
    println!("  partition: evens={evens:?} odds={odds:?}");

    // --- ЗАМЕР: итераторы не медленнее цикла ------------------------------
    let n = 1000usize;
    let b_loop = bench("for i in 0..n { sum += i*2 }", 2000, |_| {
        let mut sum = 0u64;
        for i in 0..n {
            sum += (i as u64) * 2;
        }
        sum
    });
    let b_iter = bench("sum = (0..n).map(|i| i*2).sum()", 2000, |_| {
        (0..n).map(|i| (i as u64) * 2).sum::<u64>()
    });
    show(&b_loop);
    show(&b_iter);
    println!("  → компилятор превращает цепочку в тот же цикл: разница только в шуме");
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 9.3 — ТЕОРИЯ: fold, peekable, свои итераторы
// ─────────────────────────────────────────────────────────────────────────────
//
// `fold(init, f)` — универсальная свёртка. Через неё выражается всё:
// `sum`, `all`, `any`, `min`, `max`, `position`, `count`. Полезен, когда
// нужно совместить несколько агрегатов в один проход (без двух `iter`).
//
// `scan` — «fold, но выдаёт промежуточные значения». Отлично для
// накопления состояния, которое видно в каждом шаге.
//
// `peekable` — один шаг вперёд. Нужен для парсинга: чтобы понять,
// является ли текущий токен концом списка, не съедая его.
//
// `try_fold` — как fold, но может остановиться с ошибкой. `collect`
// поверх fallible-итераторов даёт `Result<Vec<_>, _>`.
//
// СВОЙ ИТЕРАТОР — это просто `impl Iterator for MyType { type Item = T; fn next(...) }`.
// Нужно знать минимум три трейта:
//   Iterator          — обязателен. `next`, и всё остальное с дефолтами.
//   ExactSizeIterator — «я знаю свой len()». Разблокирует `.len()`.
//   DoubleEndedIterator — `.rev()` и `.next_back()`. Бесплатно из `iter().rev()`.
//
// ⚠ Если ленивость не нужна, `impl Iterator` может работать МЕДЛЕННЕЕ
//   `collect`: компилятор не знает размер, и `Vec::push` не
//   переиспользуется. Для горячего цикла с известным размером —
//   возвращай `Vec`, а не итератор.

fn part_9_3() {
    harness::part("9.3", "ПРИМЕР: fold/peekable/try_fold и свой итератор");

    // --- fold: несколько агрегатов за один проход --------------------------
    let v = vec![3u32, 1, 4, 1, 5, 9, 2, 6];
    let stats = v.iter().fold((0u32, 0u32, u32::MAX, u32::MIN), |(s, c, mn, mx), x| {
        (s + x, c + 1, mn.min(*x), mx.max(*x))
    });
    println!("  fold: sum={} count={} min={} max={}", stats.0, stats.1, stats.2, stats.3);

    // --- МОЖНО БЫЛО, НО ДОРОЖЕ: три отдельных прохода ------------------------
    let sum2: u32 = v.iter().sum();
    let cnt = v.len() as u32;
    let mn2 = *v.iter().min().expect("не пусто");
    let mx2 = *v.iter().max().expect("не пусто");
    println!("  три прохода: sum={sum2} count={cnt} min={mn2} max={mx2}");

    // --- peekable: смотрим на следующий, не забирая ------------------------
    // ⚠ `it.peek()` даёт Option<&&i32> (итератор отдаёт &i32).
    //   Двойное разыменование в паттерне — нормальная практика.
    let mut it = [1, 2, 3].iter().peekable();
    let mut desc = Vec::new();
    while let Some(&x) = it.next() {
        match it.peek() {
            Some(&&next) if next > x => desc.push(format!("{x}↑")),
            Some(&&next) if next < x => desc.push(format!("{x}↓")),
            _ => desc.push(format!("{x}=")),
        }
    }
    println!("  peekable: {desc:?}");

    // --- try_fold: сворачиваем с ошибкой -----------------------------------
    let checked: Result<u32, String> = [10u32, 20, 0, 30].iter().try_fold(0u32, |acc, x| {
        if *x == 0 {
            Err("ноль".into())
        } else {
            Ok(acc + x)
        }
    });
    println!("  try_fold: {checked:?}");

    // --- СВОЙ ИТЕРАТОР ------------------------------------------------------
    let names = ["alpha", "beta", "gamma"];
    let w = Window::new(&names, 2);
    for window in w {
        println!("  Window: {window:?}");
    }

    let counter = Fib { a: 0, b: 1, limit: 10 };
    let nums: Vec<u64> = counter.collect();
    println!("  Fib: {nums:?}");

    // --- Свой итератор с двусторонним доступом -----------------------------
    let de = MyRange { start: 0, end: 5 };
    let fwd: Vec<i32> = de.collect();
    let mut de2 = MyRange { start: 0, end: 5 };
    let rev: Vec<i32> = de2.by_ref().rev().collect();
    println!("  MyRange: fwd={fwd:?} rev={rev:?} (DoubleEnded)");
}

/// Свой итератор: скользящее окно.
///
/// Урок: итератор хранит СОСТОЯНИЕ (здесь — остаток среза) и каждый
/// вызов `next` двигает его. В «настоящем» варианте окно лежит в буфере и
/// перезаписывается, а наружу отдаётся срез — но это требует `unsafe`
/// и ручного доказательства lifetime'ов, см. модуль 12.
struct Window<'a, T> {
    rest: &'a [T],
    size: usize,
}

impl<'a, T: Copy> Window<'a, T> {
    fn new(data: &'a [T], size: usize) -> Self {
        // size == 0 — вырожденный случай: пустой срез, ноль элементов.
        let rest = if size == 0 { &data[data.len()..] } else { data };
        Window { rest, size }
    }
}

impl<'a, T: Copy + std::fmt::Debug> Iterator for Window<'a, T> {
    type Item = Vec<T>;

    fn next(&mut self) -> Option<Vec<T>> {
        if self.rest.len() < self.size {
            return None;
        }
        let out = self.rest[..self.size].to_vec();
        self.rest = &self.rest[1..];
        Some(out)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = if self.size == 0 { 0 } else { self.rest.len().saturating_sub(self.size) + 1 };
        (n, Some(n))
    }
}

struct Fib {
    a: u64,
    b: u64,
    limit: usize,
}

impl Iterator for Fib {
    type Item = u64;
    fn next(&mut self) -> Option<u64> {
        if self.limit == 0 {
            return None;
        }
        self.limit -= 1;
        let r = self.a;
        let next = self.a + self.b;
        self.a = self.b;
        self.b = next;
        Some(r)
    }
}

/// Итератор с двусторонним доступом и точной длиной.
struct MyRange {
    start: i32,
    end: i32,
}

impl Iterator for MyRange {
    type Item = i32;
    fn next(&mut self) -> Option<i32> {
        if self.start < self.end {
            self.start += 1;
            Some(self.start)
        } else {
            None
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = (self.end - self.start).max(0) as usize;
        (n, Some(n))
    }
}

impl DoubleEndedIterator for MyRange {
    fn next_back(&mut self) -> Option<i32> {
        if self.start < self.end {
            self.end -= 1;
            Some(self.end)
        } else {
            None
        }
    }
}

impl ExactSizeIterator for MyRange {}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 9.4 — ЗАДАНИЕ 9.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Конвейер middleware. Каждая стадия получает строку и возвращает
/// строку. Часть стадий падает — и это должно быть видно в типах.
///
/// ```ignore
/// fn apply_chain(input: &str, stages: &[Box<dyn Fn(String) -> Result<String, String>>])
///     -> Result<String, String>;
/// fn make_logger(prefix: &'static str) -> impl Fn(String) -> Result<String, String>;
/// fn make_upper() -> impl Fn(String) -> Result<String, String>;
/// fn make_reject_empty() -> impl Fn(String) -> Result<String, String>;
/// fn compose<A, B, C>(f: impl Fn(A) -> B, g: impl Fn(B) -> C) -> impl Fn(A) -> C;
/// ```
///
/// Требования:
///   * `apply_chain` останавливается на ПЕРВОЙ ошибке и возвращает её
///     с указанием индекса стадии (`"стадия 1: ..."`).
///   * `compose` — point-free композиция двух замыканий.
///   * `compose(make_logger("dbg"), make_upper())` должен работать:
///     `Logger` логирует в буфер, `upper`uppercase'ит.
///   * ⚠ Заметь разницу типов: `make_logger` возвращает `impl Fn`,
///     а `apply_chain` принимает `&[Box<dyn Fn(..)>]`. Расскажи в
///     комментарии, почему нельзя просто собрать `Vec<impl Fn>`.
fn task_9_1() {
    use std::cell::RefCell;

    fn make_logger(prefix: &'static str) -> impl Fn(String) -> Result<String, String> {
        let log = RefCell::new(Vec::new());
        move |s: String| {
            log.borrow_mut().push(format!("{prefix}: {s}"));
            Ok(s)
        }
    }

    // ⚠ Заглушка `not_yet!` имеет тип `!` (never). Он приводится к ЛЮБОМУ
    //   типу, поэтому может стоять последним выражением функции. Но НЕ может
    //   быть телом функции, возвращающей `impl Fn(...)`: `!` не замыкание.
    //   Поэтому заглушка всегда ВНУТРИ замыкания.
    fn make_upper() -> impl Fn(String) -> Result<String, String> {
        move |s: String| {
            not_yet!("сделай to_uppercase(), а для пустой строки верни Err");
        }
    }

    fn make_reject_empty() -> impl Fn(String) -> Result<String, String> {
        move |s: String| {
            not_yet!("верни Err(\"пустая строка\"), если s.trim().is_empty()");
        }
    }

    fn apply_chain(
        input: &str,
        stages: &[Box<dyn Fn(String) -> Result<String, String>>],
    ) -> Result<String, String> {
        not_yet!("fold по стадиям, на ошибке верни format!(\"стадия {i}: {e}\")");
    }

    fn compose<A, B, C>(
        f: impl Fn(A) -> B,
        g: impl Fn(B) -> C,
    ) -> impl Fn(A) -> C {
        move |x: A| {
            // Заглушка вызывает f и g, чтобы компилятор ВЫВЕЛ типы B и C.
            // Потом эти две строки удаляешь и оставляешь только `g(f(x))`.
            let _b = f(x);
            let _c = g(_b);
            not_yet!("точка-свободная композиция: g(f(x))");
        }
    }

    let stages: Vec<Box<dyn Fn(String) -> Result<String, String>>> = vec![
        Box::new(make_reject_empty()),
        Box::new(make_upper()),
    ];
    assert_eq!(apply_chain("hello", &stages).unwrap(), "HELLO");
    assert_eq!(apply_chain("   ", &stages).unwrap_err(), "стадия 0: пустая строка");

    // ⚠ Композиция «склеивает» ТИПЫ: g принимает ровно то, что вернул f,
    //   то есть `Result<String, String>`, а не `String`. Это и есть
    //   главное свойство point-free композиции: ошибки не теряются,
    //   потому что промежуточный тип — тот же Result.
    let upper_then_exclaim = compose(
        make_upper(),
        |r: Result<String, String>| -> Result<String, String> { r.map(|s| format!("{s}!")) },
    );
    assert_eq!(upper_then_exclaim("hi".to_string()).unwrap(), "HI!");
    assert!(upper_then_exclaim("   ".to_string()).is_err(), "ошибка прошла дальше");

    // Vec<impl Fn> невозможен: impl Trait — это анонимный ТИП, и разные
    // вызовы make_upper() дают РАЗНЫЕ типы. Их нельзя положить в один Vec.
    // Нужен либо общий dyn-тип (Box<dyn Fn>), либо обобщённая структура.
}

/// ЧАСТЬ 9.5 — ЗАДАНИЕ 9.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Тот же результат тремя способами. Задача — не «напиши итераторы»,
/// а ПОНЯТЬ, что разница только в читаемости, а не в скорости.
///
/// ```ignore
/// fn imperative(v: &[u32]) -> (u32, u32, u32);   // sum, max, count_even
/// fn lazy(v: &[u32]) -> (u32, u32, u32);        // три прохода, map/filter/sum
/// fn one_pass(v: &[u32]) -> (u32, u32, u32);    // один fold
/// ```
///
/// Требования:
///   * Никаких `Vec` в возвращаемых значениях.
///   * В `one_pass` — ровно один проход по данным (никаких `.iter()`
///     внутри `.iter()`).
///   * ⚠ БОНУС (объясни в комментарии): посчитай `min` вместо `max`.
///
/// Замерь все три через `curriculum::harness::bench` и выведи.
/// Удивишься, что разница в пределах шума.
fn task_9_2() {
    use curriculum::harness::bench;

    fn imperative(v: &[u32]) -> (u32, u32, u32) {
        let mut sum = 0u32;
        let mut max = 0u32;
        let mut count_even = 0u32;
        for &x in v {
            sum += x;
            if x > max {
                max = x;
            }
            if x % 2 == 0 {
                count_even += 1;
            }
        }
        (sum, max, count_even)
    }

    fn lazy(v: &[u32]) -> (u32, u32, u32) {
        not_yet!("три отдельных прохода: sum, max через max/fold, count_even");
    }

    fn one_pass(v: &[u32]) -> (u32, u32, u32) {
        not_yet!("ОДИН fold с тремя аккумуляторами");
    }

    let data: Vec<u32> = (0..10_000u32).map(|i| (i * 7919) % 1000).collect();
    assert_eq!(imperative(&data), lazy(&data));
    assert_eq!(imperative(&data), one_pass(&data));

    let b1 = bench("imperative", 500, |_| imperative(&data));
    let b2 = bench("lazy (3 прохода)", 500, |_| lazy(&data));
    let b3 = bench("one_pass (fold)", 500, |_| one_pass(&data));
    println!("   {b1}\n   {b2}\n   {b3}");
}

/// ЧАСТЬ 9.6 — ЗАДАНИЕ 9.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Свой итератор без единой аллокации на элемент. Заодно учимся
/// делать итератор `ExactSizeIterator` и `DoubleEndedIterator`.
///
/// ```ignore
/// struct Chunks2<'a, T> { data: &'a [T], size: usize, pos: usize }
///
/// impl<'a, T> Chunks2<'a, T> {
///     fn new(data: &'a [T], size: usize) -> Self;
/// }
///
/// impl<'a, T: Copy> Iterator for Chunks2<'a, T> {
///     type Item = &'a [T];
///     fn next(&mut self) -> Option<&'a [T]>;
/// }
///
/// struct Countdown(u32);
/// impl Iterator for Countdown { type Item = u32; }
/// ```
///
/// Требования:
///   * `Chunks2` возвращает **срез** (нулевую копию), а не `Vec` —
///     в отличие от демонстрационного `Window` выше.
///   * `size == 0` → пустой итератор (не паниковать, не зациклиться).
///   * `Countdown` — `DoubleEndedIterator` + `ExactSizeIterator`.
///   * Реализуй `size_hint` у обоих.
fn task_9_3() {
    struct Chunks2<'a, T> {
        data: &'a [T],
        size: usize,
        pos: usize,
    }

    impl<'a, T> Chunks2<'a, T> {
        fn new(data: &'a [T], size: usize) -> Self {
            not_yet!("size == 0 -> pos = usize::MAX (пустой), иначе 0");
        }
    }

    impl<'a, T> Iterator for Chunks2<'a, T> {
        type Item = &'a [T];
        fn next(&mut self) -> Option<&'a [T]> {
            not_yet!("верни срез self.data[self.pos..min(pos+size, len)]");
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            not_yet!("остаток / size");
        }
    }

    struct Countdown(u32);

    impl Iterator for Countdown {
        type Item = u32;
        fn next(&mut self) -> Option<u32> {
            not_yet!("уменьши, отдай; 0 -> None");
        }
        fn size_hint(&self) -> (usize, Option<usize>) {
            not_yet!("(self.0, Some(self.0))");
        }
    }

    impl DoubleEndedIterator for Countdown {
        fn next_back(&mut self) -> Option<u32> {
            not_yet!("0 -> None, иначе верни верх и уменьши");
        }
    }

    impl ExactSizeIterator for Countdown {}

    let data = [1, 2, 3, 4, 5, 6, 7];
    let parts: Vec<&[i32]> = Chunks2::new(&data, 3).collect();
    assert_eq!(parts, vec![&[1, 2, 3][..], &[4, 5, 6][..], &[7][..]]);
    assert_eq!(Chunks2::new(&data, 0).count(), 0);
    assert_eq!(Chunks2::new(&data, 2).size_hint(), (3, Some(3)));

    let mut cd = Countdown(3);
    assert_eq!(cd.next(), Some(3));
    assert_eq!(cd.next_back(), Some(1));
    assert_eq!(cd.len(), 1);
    assert_eq!(cd.collect::<Vec<_>>(), vec![2]);
    println!("  Chunks2 + Countdown: без аллокаций, двусторонний, с точной длиной");
}
