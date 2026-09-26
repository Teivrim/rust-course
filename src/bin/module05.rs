//! ============================================================================
//! МОДУЛЬ 05 — ТИПЫ: NEWTYPE, STRUCT, ENUM, OPTION, ПАТТЕРНЫ
//! ============================================================================
//!
//! ЧАСТЬ 5.1 — ТЕОРИЯ
//! ─────────────────────────────────────────────────────────────────────────────
//! Rust почти не имеет «примитивных» типов в привычном смысле. Любой тип —
//! это отдельный идентификатор, и на этом языке строятся три вещи,
//! которых в C++ нет на уровне типов:
//!
//! 1. NEWTYPE — обёртка `struct Meters(f32);`. Размер ноль байт extra,
//!    скорость ноль, но `Meters` и `Feet` — РАЗНЫЕ типы, и `a + b` для
//!    разных единиц не компилируется. В C++ пришлось бы делать отдельный
//!    класс с операторами, а если класс не нужен — комментарий «units are
//!    meters» и 500 мест, где его забыли.
//!
//! 2. SUM TYPE — `enum`, который хранит ОДНО из разных значений.
//!    Это «tagged union, где тег проверил компилятор».
//!
//! 3. OPTION — «может быть или может не быть» как ТИП, а не как
//!    соглашение про null.
//!
//! НОВЫЙ ТИП — ГЛАВНЫЙ ИНСТРУМЕНТ. Стоит задать себе вопрос «а нельзя ли
//! вместо `usize` завести `struct EntityId(usize)`?» — и половина багов
//! исчезает сама. `fn remove_entity(&mut self, id: EntityId)` нельзя
//! вызвать с `usize` индекса массива, `fn set_width(EntityId, u32)` —
//! нельзя перепутать порядок. Сравни с C++, где все эти параметры —
//! `uint32_t` и ошибка «перепутал id с width» находится только в рантайме.
//!
//! СТОИМОСТЬ NEWTYPE: компилятор гарантирует `repr(Rust)` отсутствие
//! накладных расходов в большинстве случаев, но строго это не значит.
//! Проверяется в модуле 11/13 через `size_of`.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

fn main() {
    harness::module(5, "Типы, newtype, enum, Option, паттерны");

    part_5_1(); // newtype: бесплатно и полезно
    part_5_2(); // Option и раскладка памяти
    part_5_3(); // enum, match, паттерны
    part_5_4(); // match ergonomics, if let / let else

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("5.1  Типизированные ID: EntityId, WindowId, AssetId", task_5_1);
    r.task("5.2  Сцена: Option-снаряжение без единого unwrap", task_5_2);
    r.task("5.3  Конечный автомат сцены на enum", task_5_3);
    r.task("5.4  Niche: NonZero и repr(u8)", task_5_4);
    r.summary(5);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 5.1 — ПРИМЕР: newtype
// ─────────────────────────────────────────────────────────────────────────────
fn part_5_1() {
    harness::part("5.1", "ПРИМЕР: newtype — бесплатная страховка");

    // Три формы структур в Rust:
    let p = Point3 { x: 1.0, y: 2.0, z: 3.0 }; // именованные поля
    let c = (1.0f32, 2.0f32); // кортеж: удобно для пар, нечитаемо для 6 полей
    let u = Units; // unit-структура: размер 0 байт, только как маркер
    println!("  named={p:?} tuple={c:?} unit={u:?} (size = {})", size_of::<Units>());

    // ── Newtype: единицы измерения ─────────────────────────────────────────
    let dist = Meters(100.0);
    let speed = Mps(5.0);
    println!("  dist={dist:?} speed={speed:?}");

    // ⛔ dist + speed   // E0369: cannot add `Meters` + `Mps` — это ХОРОШО
    println!("  за 20 секунд пролетит: {:?} (Mps * f32 = Meters)", speed * 20.0);

    // ⛔ let wrong: Meters = 100.0;  // E0308 mismatched types

    // ── Newtype: идентификаторы ────────────────────────────────────────────
    let e = EntityId(7);
    let s = ShaderId(7);
    // ⛔ take_entity(e) с ShaderId — ошибка компиляции
    println!("  EntityId(7) и ShaderId(7) — оба usize, но разные типы: {e:?} {s:?}");

    // Форматирование/парсинг через трейты:
    println!("  Display: {e} | Debug: {e:?} | парс: {:?}", EntityId::from_str_lossy("42"));

    // ── Преобразования ─────────────────────────────────────────────────────
    let raw: u32 = 1000;
    let ent = EntityId::from_raw(raw);
    println!("  EntityId::from_raw({raw}) = {ent:?} (с проверкой старших бит)");
    assert!(EntityId::from_raw(u32::MAX).is_none(), "зарезервированное значение 0xFFFF_FFFF отклонено");

    // ── newtype поверх Option для «ненулевого» значения ───────────────────
    // Это ровно то, из чего состоит std::NonZeroU32 — см. часть 5.4.
}

/// Точка в 3D.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Point3 {
    x: f32,
    y: f32,
    z: f32,
}

/// Unit-структура: маркер без данных, размер 0 байт.
#[derive(Debug, Clone, Copy)]
struct Units;

/// Единица нового типа. Никакого extra-поля нет — компилятор
/// гарантирует, что repr совпадает с внутренним f32.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Meters(f32);

/// Скорость. `Meters + Mps` не собирается — и это цель.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Mps(f32);

impl std::ops::Add<Meters> for Mps {
    type Output = Meters;
    fn add(self, rhs: Meters) -> Meters {
        Meters(self.0 * rhs.0)
    }
}

impl std::ops::Mul<f32> for Mps {
    type Output = Meters;
    fn mul(self, k: f32) -> Meters {
        Meters(self.0 * k)
    }
}

impl std::fmt::Display for Meters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:.2} m", self.0)
    }
}

/// Идентификатор сущности. `generation` отделён от `index`, чтобы
/// переиспользованные индексы не путали старые ссылки с новыми (ECS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct EntityId(u32);

impl EntityId {
    const RESERVED: u32 = u32::MAX;

    fn from_raw(raw: u32) -> Option<Self> {
        if raw == Self::RESERVED {
            None
        } else {
            Some(EntityId(raw))
        }
    }

    fn from_str_lossy(s: &str) -> Option<EntityId> {
        // В задании 5.1 это будет настоящий парсер без unwrap.
        s.parse::<u32>().ok().and_then(EntityId::from_raw)
    }
}

impl std::fmt::Display for EntityId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "#{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ShaderId(u32);

use std::mem::size_of;

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 5.2 — ТЕОРИЯ: Option и NICHЕ
// ─────────────────────────────────────────────────────────────────────────────
//
// `Option<T>` = `enum { None, Some(T) }`. Но компилятор умеет хитрее:
// он находит в раскладке типа «дырку» (niche) — неиспользуемое
// значение — и хранит `Option<T>` БЕЗ дополнительного байта.
//
// Доказательство прямо в примере ниже: `size_of::<Option<&T>>() == 8`,
// столько же, сколько `&T`. Потому что у ссылки есть зарезервированное
// значение «null», которое не может быть настоящим указателем. Rust
// говорит: используем 0 как «нет значения». Ноль байт!
//
// ⛔ НО: работает не для всех типов.
//   Option<u32>    → 8 байт  (у u32 нет «дырки», все 2^32 значения живые)
//   Option<&u32>   → 8 байт  (null у ссылки)
//   Option<bool>   → 1 байт  (у bool только 2 значения из 256)
//   Option<Box<T>> → 8 байт  (null у Box)
//   Option<NonZeroU32> → 4 байта ← ЭТО и есть NonZeroU32!
//
// СРАВНИ С C++: `std::optional<T>` тоже пользуется niche, но
// `unique_ptr<T> p = nullptr` — это всё равно «допустимое» значение,
// которое означает «объекта нет», и ничто не мешает ему забыть проверить.
// В Rust «объекта нет» — это отдельный вариант типа, и `p.field`
// не скомпилируется без проверки.
//
// ЭТО ОБЪЯСНЯЕТ, ПОЧЕМУ Option<Vec<T>> — ЭТО 24 БАЙТА, А Vec<T> — 24.
// `Option<Vec<T>>` не нужно ни « дописать флаг: null-указателя Vec
// достаточно.

fn part_5_2() {
    harness::part("5.2", "ПРИМЕР: Option, NICHЕ и комбинаторы");

    println!("  ── раскладка памяти ──");
    println!("  u32 = {}, Option<u32> = {}   (у u32 нет дырки → +4 байта)",
        size_of::<u32>(), size_of::<Option<u32>>());
    println!("  &u32 = {}, Option<&u32> = {}   (null у ссылки → дырка есть, 0 байт)",
        size_of::<&u32>(), size_of::<Option<&u32>>());
    println!("  bool = {}, Option<bool> = {}   (2 значения из 256 → дырка)",
        size_of::<bool>(), size_of::<Option<bool>>());
    println!("  Vec<u8> = {}, Option<Vec<u8>> = {}   (null у Vec → дырка)",
        size_of::<Vec<u8>>(), size_of::<Option<Vec<u8>>>());
    println!("  NonZeroU32 = {}, Option<NonZeroU32> = {}   (0 зарезервирован → дырка)",
        size_of::<std::num::NonZeroU32>(), size_of::<Option<std::num::NonZeroU32>>());

    // ── Базовые операции ───────────────────────────────────────────────────
    let some: Option<i32> = Some(42);
    let none: Option<i32> = None;
    println!("\n  some={some:?} none={none:?}");
    println!("  unwrap_or        = {}", some.unwrap_or(0));
    println!("  unwrap_or_else   = {}", none.unwrap_or_else(|| -1));
    println!("  map              = {:?}", some.map(|x| x * 2));
    println!("  and_then         = {:?}", some.and_then(|x| if x > 0 { Some(x as u32) } else { None }));
    println!("  filter           = {:?}", some.filter(|x| *x > 100)); // None
    println!("  ok_or            = {:?}", some.ok_or(-1));
    println!("  is_some/map_or   = {}/{}", some.is_some(), none.map_or(false, |x| x > 0));
    println!("  take             = {}", some.unwrap_or(0)); // ⚠ see below

    // ── as_deref: снять один слой Option ──────────────────────────────────
    let opt_name: Option<String> = Some(String::from("vulkan"));
    let opt_len: Option<usize> = opt_name.as_deref().map(str::len);
    println!("  as_deref().map(len) = {opt_len:?}  (без клонирования строки)");

    // ── Option<Vec<T>> вместо Vec<Option<T>> ───────────────────────────────
    println!("\n  ── два способа хранить опциональные данные ──");
    let mut dense: Vec<Option<u32>> = vec![Some(1), None, Some(3)];
    dense[0] = dense[0].or(Some(99)); // заполнить дырку
    println!("  Vec<Option<u32>>     = {dense:?} (лишняя память на каждый элемент)");

    let mut sparse: Option<Vec<u32>> = Some(vec![1, 3]);
    if let Some(v) = &mut sparse {
        v.push(4);
    }
    println!("  Option<Vec<u32>>     = {sparse:?} (0 байт на «отсутствие»)");

    // ── Комбинатор take: забрать значение, оставив None ───────────────────
    let mut cfg = Some(String::from("режим"));
    let taken = cfg.take(); // cfg теперь None
    println!("  take: taken={taken:?}, cfg={cfg:?}  (аналог mem::take из модуля 1)");
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 5.3 — ТЕОРИЯ: enum и match
// ─────────────────────────────────────────────────────────────────────────────
//
// `enum` в Rust — это ОБЪЕДИНЕНИЕ типов (sum type): значение занимает
// место ОДНОГО из вариантов, плюс скрытый тег. Компилятор сам выбирает
// размер как максимум по вариантам + тег (и применяет niche, если может).
//
// СРАВНИ С C++:
//   C++ tagged union руками: union U { A a; B b; }; enum Kind kind;  ← 3 ошибки
//   возможны: забыл про тег, скопировал не тот вариант, тип невалиден.
//   Rust: enum E { A(A), B(B) }  ← невозможно получить B, когда в E лежит A.
//
// MATCH — ЭТО НЕ ПРОВЕРКА, А ИСЧЕРПЫВАЮЩИЙ РАЗБОР.
//   `match` обязан покрыть все варианты, иначе ошибка компиляции.
//   Новый вариант в enum → компилятор указывает все `match`, которые
//   «забыли» про него. Это называется exhaustiveness, и в C++ нет
//   ничего подобного: забытый `case` — это тихая поломка в рантайме.
//
// ЭТО ПОЗВОЛЯЕТ РЕФАКТОРИТЬ: добавил `LoadState::Compiling` — компилятор
// перечислил все места, где надо подумать. Ни один компилятор C++
// так не умеет.

fn part_5_3() {
    harness::part("5.3", "ПРИМЕР: match и его паттерны");

    let states = [
        LoadState::Idle,
        LoadState::Loading { progress: 0.5, name: String::from("hero.mesh") },
        LoadState::Failed { code: 404, reason: String::from("not found") },
        LoadState::Ready,
    ];

    for s in &states {
        // --- Базовый match с привязкой полей --------------------------------
        let text = match s {
            LoadState::Idle => String::from("пусто"),
            LoadState::Loading { progress, name } => {
                format!("грузим {name} на {:.0}%", progress * 100.0)
            }
            LoadState::Failed { code, reason } => format!("ошибка {code}: {reason}"),
            LoadState::Ready => String::from("готов"),
        };
        println!("  {text}");
    }

    // --- GUARD: условие внутри паттерна -------------------------------------
    // Тип u32 выбран не случайно: для него `n < 0` бессмысленно, и
    // компилятор так и отметил бы. Порядок веток тоже важен: сначала
    // конкретные, потом guards, потом `_`.
    let x: u32 = 5;
    let desc = match x {
        0 => "ноль".to_string(),
        1..=9 => "однозначное".to_string(),   // RANGE-паттерн
        n if n.is_power_of_two() => format!("степень двойки: {n}"), // GUARD
        _ => "многозначное".to_string(),        // WILDCARD
    };
    println!("  match с guard/range: {x} → {desc}");

    // --- @-BINDING: поймать и проверить, и не разворачивать вручную ----------
    let y = 42;
    match y {
        n @ 10..=50 => println!("  @binding: {n} попал в 10..=50"),
        n => println!("  @binding: {n} вне диапазона"),
    }

    // --- | — ОР-ПАТТЕРН: несколько вариантов одной рукой --------------------
    let c = 'q';
    let is_stop = match c {
        'q' | 'Q' | '\x1b' => true,
        _ => false,
    };
    println!("  or-паттерн: '{c}' → is_stop = {is_stop}");

    // --- ВЛОЖЕННЫЕ ПАТТЕРНЫ -------------------------------------------------
    let pairs = vec![Some(Ok(3)), None, Some(Err("bad"))];
    for p in &pairs {
        let text = match p {
            Some(Ok(n)) => format!("ок {n}"),
            Some(Err(e)) => format!("ошибка {e}"),
            None => "ничего".to_string(),
        };
        println!("  вложенный: {text}");
    }

    // --- SLICE-ПАТТЕРНЫ: прямой разбор массива ------------------------------
    // ⚠ Массив `[i32; 3]` имеет ДЛИНУ В ТИПЕ, поэтому паттерны переменной
    //   длины (`[]`, `[only]`, `[first, .., last]`) к нему не подходят.
    //   Нужен срез: `arr.as_slice()`. Это частая ошибка новичков.
    let arr = [1, 2, 3];
    let name: String = match arr.as_slice() {
        [] => "пусто".to_string(),
        [only] => format!("ровно один элемент: {only}"),
        [first, .., last] => format!("first={first} last={last} middle={}", arr.len() - 2),
    };
    println!("  slice-паттерн: {name}");

    // --- matches! и if let ---------------------------------------------------
    println!("  matches!(10) = {}", matches!(10, 1..=20));
    if let Some(v) = Some(7) {
        println!("  if let: {v}");
    }
}

fn return_early(n: i32) -> &'static str {
    "ровно один элемент"
}

enum LoadState {
    Idle,
    Loading { progress: f32, name: String },
    Failed { code: u32, reason: String },
    Ready,
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 5.4 — ТЕОРИЯ: match ergonomics и if let / let else
// ─────────────────────────────────────────────────────────────────────────────
//
// MATCH ERGONOMICS ( ergonomics ) — правило, которое позволяет не писать
// `&` в каждом паттерне, когда подставляешь ссылку. Rust 2021 сделал
// это поведением по умолчанию. Практически: `for x in &vec` даёт
// `x: &T`, и в match пишешь `Some(v)` вместо `Some(&v)`, а `v` будет
// `&T`.
//
// ⚠ ОБРАТНАЯ СТОРОНА, О КОТОРОЙ НАДО ЗНАТЬ: если ошибся и взял не
// ссылку, а значение, `x` в match будет на **КОПИИ**, и компилятор
// подскажет об этом в сообщении («you might want to use a reference
// pattern»). Читай предупреждения компилятора — они учат.
//
// THREE ФОРМЫ ВЫБОРА:
//
//   match x { ... }          — исчерпывающий разбор, возвращает значение
//   if let P = x { ... }     — «если подходит, сделай». Одна проверка.
//                               Работает, только если P опровержим (refutable).
//   let P = x else { ... }   — «иначе выйди». Требует refutable P.
//                               else обязан «разойтись» (return/break/panic).
//
// ПОЧЕМУ НЕТ `try`-конструкции для Option: потому что `?` уже есть
// для Result, и в Rust 1.x его научились применять к Option тоже
// (через `Try`). Если опция `None` — возвращаем None.
//
// LET ELSE — САМЫЙ ИДИОМАТИЧНЫЙ СПОСОБ ВЫРАЗИТЬ «извлеки или уйди».
// Он не съедает вложенность, в отличие от `if let`:
//
// ```ignore
// // if let — два уровня вложенности
// if let Some(cfg) = config.get("renderer") {
//     use_renderer(cfg);
// }
//
// // let else — линейный код, всё важное на верхнем уровне
// let Some(cfg) = config.get("renderer") else { return Default::default() };
// use_renderer(cfg);
// ```

fn part_5_4() {
    harness::part("5.4", "ПРИМЕР: match ergonomics, if let, let else");

    let data = Some(String::from("payload"));
    let opt: Option<String> = data;

    // --- match ergonomics: & без &
    if let Some(ref s) = opt {
        println!("  pattern `ref s` (явный реф): {s}");
    }
    match &opt {
        Some(s) => println!("  match &opt → s: &String, len = {}", s.len()),
        None => println!("  none"),
    }

    // --- let else: линейный стиль
    fn must_get(o: &Option<String>) -> String {
        let Some(s) = o else {
            return "<нет данных>".to_string(); // else обязан разойтись
        };
        // Теперь `s` — просто String, без лишнего уровня вложенности.
        s.to_uppercase()
    }
    println!("  let else: {:?}", must_get(&opt));
    println!("  let else (пусто): {:?}", must_get(&None));

    // --- while let: вычитываем канал/итератор, пока паттерн подходит ------
    let mut queue = vec![1, 2, 3];
    let mut taken = Vec::new();
    while let Some(v) = queue.pop() {
        taken.push(v);
    }
    println!("  while let pop: {:?} (порядок — с конца)", taken);

    // --- if let с else: «что делать, если не подошло»
    let n = 42;
    if let Some(d) = divisor_of(n) {
        println!("  divisor_of({n}) = {d}");
    } else {
        println!("  divisor_of({n}) = None");
    }

    // --- match на ссылке: современный стиль
    let shapes = [Shape::Circle(2.0), Shape::Rect(1.0, 2.0)];
    let areas: Vec<f32> = shapes.iter().map(|s| s.area()).collect();
    println!("  area через match по &enum: {areas:?}");

    // ⛔ НЕЛЬЗЯ забыть ветку: если добавить `Shape::Triangle`, match в
    //    Shape::area перестанет компилироваться. Это и есть exhaustiveness.

    // --- Отличать «нет значения» от «ошибки» — Option vs Result -------------
    let found: Option<u32> = lookup(1);
    let attempt: Result<u32, String> = lookup_err(1);
    println!("\n  Option  (не нашлось — это не страшно): {found:?}");
    println!("  Result  (сломалось — об этом надо сказать): {attempt:?}");

    // ⚠ ЛОВУШКА: Option<u32> и Result<u32, ()> ведут себя ОДИНАКОВО через
    //   `?` в функции, возвращающей Result. Разница только в ЗНАЧЕНИИ ошибки.
    //   Если ошибки всегда одинаковые, `Option` проще и честнее.
}

fn divisor_of(n: u32) -> Option<u32> {
    (2..n).find(|d| n % d == 0)
}

fn lookup(key: u32) -> Option<u32> {
    if key == 1 { Some(100) } else { None }
}

fn lookup_err(key: u32) -> Result<u32, String> {
    if key == 1 {
        Ok(100)
    } else {
        Err(format!("ключ {key} не найден"))
    }
}

enum Shape {
    Circle(f32),
    Rect(f32, f32),
}

impl Shape {
    fn area(&self) -> f32 {
        match self {
            Shape::Circle(r) => std::f32::consts::PI * r * r,
            Shape::Rect(w, h) => w * h,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 5.5 — ЗАДАНИЕ 5.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Типизированные идентификаторы. В C++ это `struct Foo { uint32_t id; }`
/// или просто `uint32_t` со словарём. В Rust — newtype без поля
/// `value`, доступного извне.
///
/// ```ignore
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
/// struct EntityId { index: u32, generation: u32 }
///
/// impl EntityId {
///     fn new(index: u32, generation: u32) -> Self;
///     fn index(&self) -> u32;
///     fn generation(&self) -> u32;
///     fn is_valid(&self) -> bool;               // generation == 0 → мёртвая
///     fn parse(s: &str) -> Option<EntityId>;   // формат "index:generation"
///     fn to_raw(&self) -> u64;                 // пакует в u64
/// }
/// ```
///
/// Плюс `From<EntityId> for u64` и `TryFrom<u64> for EntityId`.
///
/// Требования:
///   * поля приватные, доступ через методы;
///   * `parse` без `unwrap`, `split(':')`, `parse::<u32>().ok()`;
///   * `to_raw`: `(generation as u64) << 32 | index as u64`;
///   * `TryFrom<u64>` отвергает generation == 0.
///   * Реализуй `Display` как `"{index}:{generation}"`.
fn task_5_1() {
    use std::convert::TryFrom;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
    struct EntityId {
        index: u32,
        generation: u32,
    }

    impl EntityId {
        fn new(index: u32, generation: u32) -> Self {
            not_yet!("приватные поля");
        }
        fn index(&self) -> u32 {
            not_yet!("self.index");
        }
        fn generation(&self) -> u32 {
            not_yet!("self.generation");
        }
        fn is_valid(&self) -> bool {
            not_yet!("generation != 0");
        }
        fn parse(s: &str) -> Option<Self> {
            not_yet!("split_once(':') + parse().ok() + From/EntityId");
        }
        fn to_raw(&self) -> u64 {
            not_yet!("(generation << 32) | index");
        }
    }

    impl std::fmt::Display for EntityId {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            not_yet!("write!(f, \"{{}}:{{}}\", self.index, self.generation)");
        }
    }

    impl From<EntityId> for u64 {
        fn from(e: EntityId) -> u64 {
            not_yet!("e.to_raw()");
        }
    }

    impl TryFrom<u64> for EntityId {
        type Error = &'static str;
        fn try_from(v: u64) -> Result<Self, Self::Error> {
            not_yet!("распакуй, проверь generation != 0");
        }
    }

    let e = EntityId::new(3, 1);
    assert!(e.is_valid());
    assert_eq!(e.index(), 3);
    assert_eq!(format!("{e}"), "3:1");
    assert_eq!(u64::from(e), (1u64 << 32) | 3);
    assert_eq!(EntityId::parse("3:1"), Some(e));
    assert_eq!(EntityId::parse("bad"), None);
    assert!(EntityId::try_from(3).is_err(), "generation == 0 → мёртвый id");
    assert_eq!(EntityId::try_from((1u64 << 32) | 3).unwrap(), e);
}

/// ЧАСТЬ 5.6 — ЗАДАНИЕ 5.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// API на `Option` без единого `unwrap` и без «`None` как ошибка».
///
/// ```ignore
/// struct Scene {
///     name: Option<String>,
///     camera: Option<Camera>,
///     overrides: Vec<Override>,
/// }
/// struct Camera { fov: f32, near: f32, far: f32 }
/// struct Override { layer: String, value: f32 }
///
/// impl Scene {
///     fn name_or(&self, d: &str) -> &str;
///     fn camera_or_default(&self) -> Camera;
///     fn override_for(&self, layer: &str) -> Option<f32>;
///     fn set_camera(&mut self, c: Option<Camera>);   // None = сброс
///     fn take_camera(&mut self) -> Option<Camera>;
///     fn is_ready(&self) -> bool;
/// }
/// ```
///
/// Требования:
///   * Никаких `unwrap` / `expect` / `panic!`.
///   * `camera_or_default` возвращает **новое** значение (никакого
///     `&Camera`), используй `Camera: Copy` или `unwrap_or_default`.
///   * `set_camera(None)` — это «сбросить», не «ошибка». Значит
///     `Option` тут правильный тип, а не `Result`.
///   * `take_camera` — забирает владение (модуль 1, `take`).
fn task_5_2() {
    #[derive(Debug, Clone, Copy, PartialEq)]
    struct Camera {
        fov: f32,
        near: f32,
        far: f32,
    }

    impl Default for Camera {
        fn default() -> Self {
            Camera { fov: 60.0, near: 0.1, far: 1000.0 }
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    struct Override {
        layer: String,
        value: f32,
    }

    struct Scene {
        name: Option<String>,
        camera: Option<Camera>,
        overrides: Vec<Override>,
    }

    impl Scene {
        fn name_or<'a>(&'a self, d: &'a str) -> &'a str {
            not_yet!("as_deref().unwrap_or(d) — без копирования");
        }

        fn camera_or_default(&self) -> Camera {
            not_yet!("self.camera.unwrap_or_default()");
        }

        fn override_for(&self, layer: &str) -> Option<f32> {
            not_yet!("iter().find(...).map(|o| o.value)");
        }

        fn set_camera(&mut self, c: Option<Camera>) {
            not_yet!("присваивание, это не ошибка, а состояние");
        }

        fn take_camera(&mut self) -> Option<Camera> {
            not_yet!("self.camera.take()");
        }

        fn is_ready(&self) -> bool {
            not_yet!("camera.is_some()");
        }
    }

    let mut s = Scene { name: None, camera: None, overrides: vec![] };
    assert_eq!(s.name_or("unnamed"), "unnamed");
    assert_eq!(s.camera_or_default(), Camera::default());
    assert!(!s.is_ready());

    s.name = Some(String::from("level_1"));
    assert_eq!(s.name_or("x"), "level_1");
    s.set_camera(Some(Camera { fov: 90.0, near: 0.05, far: 500.0 }));
    assert!(s.is_ready());
    assert_eq!(s.override_for("fog"), None);

    s.overrides.push(Override { layer: "fog".into(), value: 0.5 });
    assert_eq!(s.override_for("fog"), Some(0.5));

    let taken = s.take_camera().expect("камера была");
    assert_eq!(taken.fov, 90.0);
    assert!(!s.is_ready());
}

/// ЧАСТЬ 5.7 — ЗАДАНИЕ 5.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Конечный автомат загрузки. Никаких `state: i32` и `switch` с
/// магическими числами — только `enum` + `match`.
///
/// ```ignore
/// enum LoadState {
///     Idle,
///     Reading { total: u64, done: u64 },
///     Decoding { name: String },
///     Uploading { uploaded: u32, total: u32 },
///     Ready { name: String, size: u64 },
///     Failed { stage: Stage, reason: String },
/// }
/// enum Stage { Io, Decode, Upload }
///
/// fn progress(s: &LoadState) -> Option<f32>;
/// fn advance(s: LoadState) -> Result<LoadState, LoadState>;
/// fn describe(s: &LoadState) -> String;
/// ```
///
/// Правила переходов `advance`:
///   Idle            → Reading { total: 0, done: 0 }
//   Reading{done<t} → Reading { done: t+1 }        (симулируем поток)
//   Reading{done==total} → Decoding { name: "<unnamed>" }
//   Decoding        → Uploading { uploaded: 0, total: 1 }
//   Uploading{u<t}  → Uploading { uploaded: t+1 }
//   Uploading{u==t} → Ready { name: "<unnamed>", size: 0 }
//   Ready           → Err(self)  (уже готов, шагать некуда)
///   Failed          → Err(self)
///
/// Требования:
///   * `progress` возвращает `Some(0.0..=1.0)` для Reading/Uploading,
///     `None` для Idle/Done/Failed. Деление на ноль — вот главная ловушка,
///     обработай `total == 0` явно.
///   * Никаких `if state == 1`. Только `match`.
fn task_5_3() {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Stage {
        Io,
        Decode,
        Upload,
    }

    #[derive(Debug, Clone, PartialEq)]
    enum LoadState {
        Idle,
        Reading { total: u64, done: u64 },
        Decoding { name: String },
        Uploading { uploaded: u32, total: u32 },
        Ready { name: String, size: u64 },
        Failed { stage: Stage, reason: String },
    }

    fn progress(s: &LoadState) -> Option<f32> {
        not_yet!("Reading: done/total, Uploading: uploaded/total, total==0 -> Some(0.0)");
    }

    fn advance(s: LoadState) -> Result<LoadState, LoadState> {
        not_yet!("match по всем вариантам, Ready/Failed -> Err(s)");
    }

    fn describe(s: &LoadState) -> String {
        not_yet!("format! по вариантам");
    }

    let mut s = LoadState::Idle;
    assert_eq!(progress(&s), Some(0.0));
    for _ in 0..6 {
        s = advance(s).expect("переход валиден");
    }
    assert!(matches!(s, LoadState::Ready { .. }), "получили {s:?}");
    println!("  автомат дошёл до: {}", describe(&s));
    // ⚠ `advance(s)` ЗАБИРАЕТ s (move), поэтому после него нельзя
    // упоминать s — компилятор поймает. Сохраняем результат в отдельную
    // переменную, и не печатаем s после move.
    let after_ready = advance(s);
    assert!(after_ready.is_err(), "из Ready шагать некуда");
}

/// ЧАСТЬ 5.8 — ЗАДАНИЕ 5.4
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Две техники, которые невозможно вывести интуитивно — надо увидеть
/// своими глазами, как типы превращаются в байты.
///
/// Задание А: `NonZeroU32` и NICHЕ.
///   Реализуй счётчик слотов через `Option<NonZeroU32>`:
/// ```ignore
///   fn idx(nonzero: Option<std::num::NonZeroU32>) -> Option<usize>;
///   fn make(idx: usize) -> Option<std::num::NonZeroU32>;
///   ```
///   И проверь в коде, что `size_of::<Option<NonZeroU32>>()` равен 4.
///
/// Задание Б: `#[repr(u8)]` — фиксированный размер тега.
///   Реализуй `enum Primitives { ... }` и `fn tag(&self) -> u8`,
///   используя `as u8`. Проверь, что `size_of::<Primitives>() == 1`.
///
/// ВНИМАНИЕ к ответу: `as u8` для enum работает ТОЛЬКО с `#[repr(u8)]`.
/// Без него `as` не компилируется — это защита от «случайной» совместимости
/// раскладки с C. Объясни в комментарии, зачем нужен `repr`.
fn task_5_4() {
    use std::num::NonZeroU32;

    fn idx(nonzero: Option<NonZeroU32>) -> Option<usize> {
        not_yet!("map(|n| n.get() as usize - 1)");
    }

    fn make(i: usize) -> Option<NonZeroU32> {
        not_yet!("i+1 в NonZeroU32 — обработай переполнение");
    }

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Primitives {
        Float = 0,
        Int = 1,
        Bool = 2,
        Texture = 3,
    }

    fn tag(p: Primitives) -> u8 {
        not_yet!("p as u8 — работает только из-за repr(u8)");
    }

    assert_eq!(size_of::<Option<NonZeroU32>>(), 4, "niche: NonZero32 свободен 0");
    assert_eq!(size_of::<Primitives>(), 1, "repr(u8) — тег ровно байт");

    assert_eq!(idx(Some(NonZeroU32::new(1).unwrap())), Some(0));
    assert_eq!(idx(None), None);
    assert_eq!(make(0), NonZeroU32::new(1));
    assert_eq!(make(9), NonZeroU32::new(10));
    assert_eq!(make(usize::MAX), None, "переполнение u32+1 не тихо");
    assert_eq!(tag(Primitives::Texture), 3);
}

