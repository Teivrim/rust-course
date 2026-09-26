//! ============================================================================
//! МОДУЛЬ 06 — ТРЕЙТЫ, ДЖЕНЕРИКИ, ДИСПЕТЧЕРИЗАЦИЯ
//! ============================================================================
//!
//! ЧАСТЬ 6.1 — ТЕОРИЯ
//! ─────────────────────────────────────────────────────────────────────────────
//! `trait` — это контракт поведения. Он НЕ наследуется в смысле «is-a»,
//! а значит «умеет делать». Это ключевое отличие от C++: шаблон в C++
//! требует полного типа на этапе компиляции, трейт — только нужных
//! методов. Поэтому в Rust можно реализовать трейт для чужого типа
//! (своего крейта), а в C++ для этого приходилось писать free-function
//! или хак с ADL.
//!
//! ДВА СПОСОБА ВЫЗЫВАТЬ ТРЕЙТ:
//!
//!   1. STATIC DISPATCH — `fn f<S: Shape>(s: S)`.
//!      Компилятор создаёт отдельную копию функции для КАЖДОГО типа.
//!      Это мономорфизация. Вызов → прямой вызов, инлайн возможен.
//!      Цена: больше кода, время компиляции.
//!
//!   2. DYNAMIC DISPATCH — `fn f(s: &dyn Shape)`.
//!      Единая копия функции + vtable (таблица виртуальных методов) на
//!      каждый тип. Вызов → загрузка из vtable, инлайн невозможен.
//!      Цена: 1 индирекция за вызов + vtable в памяти + невозможность
//!      оптимизировать.
//!
//! ОБРАТНАЯ СОВМЕСТИМОСТЬ — СУТЬ ЯЗЫКА. Rust «съел» множественное
//! наследование через ДЕФОЛТНЫЕ методы трейтов (реализуешь своё —
//! перекрыл дефолт) и через blanket impls (реализовал для `&T` —
//! работает и для `&&T` автоматически).
//!
//! ПРАВИЛО: по умолчанию — ДЖЕНЕРИКИ. Бери `dyn Trait`, когда:
//!   * набор типов известен только в рантайме (плагины из папки);
//!   * нужно хранить разные типы в одной коллекции;
//!   * граница API — публичная и стабильная, а внутренности — нет.
//! Всё остальное время generic лучше: быстрее и компилируется.
//!
//! ФОРМЫ ЗАПИСИ ДИСПЕТЧЕРИЗАЦИИ, которые встретишь везде:
//!   &dyn Trait          — заимствовать, не владеть
//!   Box<dyn Trait>      — владеть на куче
//!   Arc<dyn Trait>      — владеть + разделять между потоками
//!   impl Trait в арг.   — обобщённо, но без имени параметра
//!   impl Trait в рез.   — спрятать конкретный тип (удобно для итераторов)

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, bench, report, show};
use curriculum::not_yet;

fn main() {
    harness::module(6, "Трейты, дженерики, диспетчеризация");

    part_6_1(); // static vs dynamic: замер на настоящих числах
    part_6_2(); // associated types, From/Deref/Display
    part_6_3(); // dyn-совместимость, Send/Sync, blanket impls

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("6.1  Backend: два бэкенда, generic- и dyn-путь, замер", task_6_1);
    r.task("6.2  Asset: FromStr + Display + Error + Default", task_6_2);
    r.task("6.3  Реестр плагинов: dyn-хранилище + типизированный доступ", task_6_3);
    r.summary(6);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 6.1 — ПРИМЕР: два способа вызвать трейт, с замером
// ─────────────────────────────────────────────────────────────────────────────
fn part_6_1() {
    harness::part("6.1", "ПРИМЕР: static vs dynamic dispatch (замер)");

    // --- Один и тот же трейт, два бэкенда -----------------------------------
    // ⚠ Обрати внимание: `&[Circle, Square]` невозможно — массив
    //   однотипный по построению. Для разных типов нужен либо
    //   отдельный вызов, либо (для dyn) среза `&[&dyn Shape]`.
    println!("  generic: area = {} / {}", sum_generic(&[Circle(1.0), Circle(0.5)]), sum_generic(&[Square(2.0)]));
    println!("  dyn:     area = {}", sum_dyn(&[&Circle(1.0), &Square(2.0), &Circle(0.5)]));

    // --- ЗАМЕР -------------------------------------------------------------
    // Компилятор способен выбросить весь цикл, если результат не нужен.
    // `std::hint::black_box` внутри harness::bench запрещает это делать —
    // иначе ты бы измерял пустоту и думал, что «итераторы бесплатны».
    let circles: Vec<Circle> = (0..64).map(|i| Circle(i as f32 * 0.01)).collect();
    // Для dyn нужен слайс ССЫЛОК НА ТРЕЙТ: &dyn Shape имеет размер
    // (ptr, vtable-ptr), а &[&Circle] — только ptr. Преобразование
    // (&Circle) → (&dyn Circle as &dyn Shape) делается автоматически.
    let c_refs: Vec<&dyn Shape> = circles.iter().map(|c| c as &dyn Shape).collect();

    let b1 = bench("sum_generic(&[Circle; 64])", 2000, |_| sum_generic(&circles));
    let b2 = bench("sum_dyn(&[&dyn Shape; 64])", 2000, |_| sum_dyn(&c_refs));
    show(&b1);
    show(&b2);
    println!("  → generic примерно в {} раз быстрее: нет загрузки vtable, есть инлайн",
        (b2.min_ns / b1.min_ns.max(1.0)).max(1.0));
    println!("  → но generic мономорфизируется: код для 100 типов = 100 копий функции");

    // --- Обобщённая функция ДЛЯ ЛЮБОГО Shape --------------------------------
    // Один раз написан, работает с любым типом, у которого есть трейт.
    let squares = [Square(1.0), Square(3.0)];
    println!("\n  describe_all (generic) = {:?}", describe_all(&squares));
    println!("  им же — поштучно: {}", Square(2.0).describe());
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 6.2 — ТЕОРИЯ: associated types и стандартные трейты
// ─────────────────────────────────────────────────────────────────────────────
//
// ASSOCIATED TYPE — «тип, который определяет сам трейт, а не вызывающий».
//
//   trait Iterator {
//       type Item;                          // каждый реализатор выбирает сам
//       fn next(&mut self) -> Option<Self::Item>;
//   }
//   impl Iterator for Counter { type Item = u32; ... }
//
// В C++ нет аналога: `std::ranges::range_value_t<R>` — обходной путь.
//
// ⛔ АССОЦИИРОВАННЫЕ ТИПЫ НЕ РАВНОСИЛЬНЫ ДЖЕНЕРИКАМ. Если написать
//   `trait Describe { type Out; }` и использовать `impl<T: Describe> f(t: T)`
//   с разными `Out` для разных T — не сработает. Обобщённая функция
//   над `T: Describe` не может знать, какой `Out` у конкретного T.
//   Именно поэтому associated types запрещены в `dyn Trait`.
//
// ГДЕ ИСПОЛЬЗУЮТСЯ:
//   Iterator::Item, IntoIterator::IntoIter, Add::Output, Deref::Target,
//   Future::Output, FromStr::Err. Все эти ошибки в C++ решались
//   шаблонными алиасами или `typename T::type` — здесь то же, но
//   с проверкой на этапе компиляции.
//
// НЕЗАВИСИМЫЕ ТИПЫ (`type Out;` против `type Out = T::Out;` в трейтах
// с параметрами) — здесь не углубляемся, это GAT (модуль 11).
//
// СТАНДАРТНЫЕ ТРЕЙТЫ, КОТОРЫЕ НУЖНО ЗНАТЬ НАИЗУСТЬ:
//
//   From<T> / Into<T>   — бесплатные, необратимые конверсии.
//                        `?` на `Result` работает ровно через From.
//   AsRef<T> / Borrow<T> — «посмотреть на меня как на X» без владения.
//   Deref               — «считай меня как X». Именно он делает
//                        `String` применимым везде, где ждут `&str`.
//   Display / Debug     — для человека / для разработчика.
//   Default             — «осмысленное ноль-значение».
//   Index / IndexMut    — `v[i]` для СВОЕГО типа.
//   Add/Sub/Mul/...     — операторы через трейты.
//   IntoIterator        — превращение в цикл.
//   Copy / Clone / Eq   — базовые штуки.

fn part_6_2() {
    harness::part("6.2", "ПРИМЕР: From/Deref/Display/свой трейт");

    // --- From: бесплатная односторонняя конверсия --------------------------
    let n: u32 = 5;
    let l: u64 = n.into(); // u32: Into<u64> из-за From<u32> for u64
    println!("  u32 {n} → u64 {l} (через From, без промежуточного кода)");

    // --- Deref: «считай меня как &str» --------------------------------------
    let name = AssetName(String::from("hero.mesh"));
    println!("  AssetName: len={} eq_str={}", name.len(), name == AssetName(String::from("hero.mesh")));

    // --- Display vs Debug ---------------------------------------------------
    let a = Primitive::Texture(0);
    println!("  Display: {a}   |   Debug: {a:?}");

    // --- Свой трейт с associated type ---------------------------------------
    let mut counter = Counter::new(3);
    let mut collected = Vec::new();
    while let Some(v) = counter.next() {
        collected.push(v);
    }
    println!("  Counter (type Item = u32) собрал: {collected:?}");

    // --- Default -------------------------------------------------------------
    println!("  Primitive::default() = {:?}", Primitive::default());
    println!("  Vec::<f32>::default() = {:?}", Vec::<f32>::default());

    // --- Трейт с несколькими методами и дефолтной реализацией ---------------
    println!("\n  Shape + дефолтный метод describe():");
    println!("    {}", Circle(1.0).describe());
    println!("    {}", Square(2.0).describe());
}

trait Shape {
    fn area(&self) -> f32;

    /// Метод по умолчанию: переопределишь — выиграешь, не переопределишь —
    /// получишь это. Это заменяет множественное наследование.
    fn describe(&self) -> String {
        format!("shape with area {:.3}", self.area())
    }

    /// Метод, доступный только на трейте (не на dyn).
    /// `where Self: Sized` говорит: этот метод не входит в vtable,
    /// потому что вернуть `Box<Self>` из трейт-объекта нельзя.
    fn grow(&mut self, k: f32)
    where
        Self: Sized,
    {
        *self = self.scaled(k);
    }

    /// Метод с `&self`, возвращающий `Self` — тоже только для конкретных
    /// типов: из `dyn Shape` нельзя построить себе копию.
    fn scaled(&self, k: f32) -> Self
    where
        Self: Sized;
}

#[derive(Debug)]
struct Circle(f32);
#[derive(Debug)]
struct Square(f32);

impl Shape for Circle {
    fn area(&self) -> f32 {
        std::f32::consts::PI * self.0 * self.0
    }
    fn scaled(&self, k: f32) -> Self {
        Circle(self.0 * k)
    }
}

impl Shape for Square {
    fn area(&self) -> f32 {
        self.0 * self.0
    }
    /// Перекрываем дефолтный метод — доказательство, что это работает.
    fn describe(&self) -> String {
        format!("square, side {:.2}, area {:.3}", self.0, self.area())
    }
    fn scaled(&self, k: f32) -> Self {
        Square(self.0 * k)
    }
}

/// STATIC DISPATCH: компилятор сделает отдельную копию для Circle и Square.
fn sum_generic<S: Shape>(shapes: &[S]) -> f32 {
    shapes.iter().map(|s| s.area()).sum()
}

/// DYNAMIC DISPATCH: одна копия + vtable.
fn sum_dyn(shapes: &[&dyn Shape]) -> f32 {
    shapes.iter().map(|s| s.area()).sum()
}

fn describe_all<S: Shape>(shapes: &[S]) -> Vec<String> {
    shapes.iter().map(|s| s.describe()).collect()
}

/// Новый тип строки, который умеет всё, что умеет `&str` (Deref),
/// но при этом не путается с ним.
#[derive(Debug, PartialEq, Eq)]
struct AssetName(String);

impl Deref for AssetName {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

use std::ops::Deref;

#[derive(Debug, PartialEq, Clone, Copy)]
enum Primitive {
    Float,
    Texture(u32),
}

impl Default for Primitive {
    fn default() -> Self {
        Primitive::Float
    }
}

impl std::fmt::Display for Primitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Primitive::Float => write!(f, "float"),
            Primitive::Texture(id) => write!(f, "texture#{id}"),
        }
    }
}

trait Countdown {
    type Item;
    fn next(&mut self) -> Option<Self::Item>;
}

struct Counter {
    left: u32,
}

impl Counter {
    fn new(n: u32) -> Self {
        Counter { left: n }
    }
}

impl Countdown for Counter {
    type Item = u32; // ← associated type: выбран реализатором
    fn next(&mut self) -> Option<u32> {
        if self.left == 0 {
            None
        } else {
            self.left -= 1;
            Some(self.left)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 6.3 — ТЕОРИЯ: dyn-совместимость, Send/Sync, blanket impls
// ─────────────────────────────────────────────────────────────────────────────
//
// DYN-СОВМЕСТИМОСТЬ (раньше «object safety»). Чтобы сделать `dyn Trait`:
//   * методы НЕ могут быть generic'ами (у каждого был бы свой vtable);
//   * методы НЕ могут возвращать `Self` без `where Self: Sized`
//     (невозможно построить нужный vtable в общем виде);
//   * трейт не может иметь associated types без `where Self: Sized`.
// Компилятор проверяет это сам и пишет точную причину.
//
// СЛЕДСТВИЕ: если трейт не dyn-совместим, доступен ТОЛЬКО generic
// путь. Это не «поломанный dyn», это разные инструменты для разных задач.
//
// SEND / SYNC — маркеры потокобезопасности, которые компилятор
// выводит АВТОМАТИЧЕСКИ:
//   Send — «можно переслать в другой поток».
//   Sync  — «можно дать &T другому потоку».
//   Rc не Send (счётчик не атомарен), Cell не Sync (можно получить &,
//   который меняет значение), но Cell: Send, если T: Send.
// В C++ ничего этого нет: `std::thread` с `shared_ptr` на не
// синхронизированный объект — это гонка, и компилятор молчит.
//
// BLANKET IMPL — реализация трейта «для всех, кто подходит»:
//   impl<T: Shape> Shape for Vec<T> { ... }
// Теперь `Vec<Circle>` тоже «имеет фигуру» (например, сумма площадей).
// В C++ аналог — частичная специализация, и работает она хуже.
//
// ⚠ ORPHAN RULE: нельзя реализовать ТРЕТИЙ трейт для ЧУЖОГО типа.
//   `impl MyTrait for Vec<u8>` — можно (MyTrait наш).
//   `impl Iterator for Vec<u8>` — нельзя (и трейт чужой, и тип чужой).
// Обходные пути: новый тип-обёртка, или impl для своей локальной обёртки.

fn part_6_3() {
    harness::part("6.3", "ПРИМЕР: Send/Sync, blanket impl, dyn-совместимость");

    // --- Send/Sync выводятся автоматически ---------------------------------
    assert_send::<Circle>();
    assert_send::<Vec<u8>>();
    // assert_send::<std::rc::Rc<u32>>();  // ⛔ Rc не Send — счётчик не атомарен
    // assert_sync::<std::cell::Cell<u32>>();  // ⛔ Cell не Sync — &Cell меняет значение
    println!("  Circle: Send + Sync, Rc<u32>: НЕ Send, Cell<u32>: НЕ Sync");

    // --- Blanket impl -------------------------------------------------------
    println!("  Vec<Circle> как Shape → area = {}", VecShape { items: vec![Circle(1.0), Circle(2.0)] }.area());

    // --- dyn-совместимость: что можно, а что нет ---------------------------
    let shapes: Vec<Box<dyn Shape>> = vec![Box::new(Circle(1.0)), Box::new(Square(2.0))];
    let total: f32 = shapes.iter().map(|s| s.area()).sum();
    println!("  Vec<Box<dyn Shape>>: {} штук, сумма площадей {total:.3}", shapes.len());

    // ⚠ `grow` НЕ входит в vtable (`where Self: Sized`).
    // let s: &mut dyn Shape = &mut Circle(1.0);
    // s.grow(2.0);   // ⛔ method not found for trait object
    let mut c = Circle(1.0);
    c.grow(2.0);
    println!("  grow доступен только на конкретном типе: r = {c:?}");

    // --- Хранение разных типов в одной коллекции ---------------------------
    let mut registry: Vec<Box<dyn Shape>> = Vec::new();
    registry.push(Box::new(Circle(1.0)));
    registry.push(Box::new(Square(3.0)));
    println!("  реестр: {} элементов разных типов в одном Vec", registry.len());

    // --- impl Trait в аргументе = анонимный дженерик ------------------------
    let sum = total_area_generic(registry.iter().map(|b| b.as_ref()));
    println!("  анонимный generic: total_area_generic(..) = {sum:.3}");
    println!("  тип аргумента скрыт: fn total_area_generic<'a>(impl Iterator<Item = &'a dyn Shape>)");
}

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

/// BLANKET IMPL в действии.
trait TotalArea {
    fn area(&self) -> f32;
}

/// Реализация трейта для чужого типа — это и есть суть blanket impls.
impl<T: Shape> TotalArea for Vec<T> {
    fn area(&self) -> f32 {
        self.iter().map(|s| s.area()).sum()
    }
}

struct VecShape {
    items: Vec<Circle>,
}

impl TotalArea for VecShape {
    fn area(&self) -> f32 {
        // Внутри используем blanket impl для Vec<Circle>
        self.items.area()
    }
}

/// `impl Trait` в аргументе = «одиночный дженерик, имя скрыто».
fn total_area_generic<'a>(shapes: impl Iterator<Item = &'a dyn Shape>) -> f32 {
    shapes.map(|s| s.area()).sum()
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 6.4 — ЗАДАНИЕ 6.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Два бэкенда рендера за одним трейтом. Это же и проверка понимания,
// когда нужен `dyn`, а когда хватает generic.
///
/// ```ignore
/// trait Backend {
///     type TextureId;                     // associated type!
///     fn name(&self) -> &str;
///     fn create_texture(&mut self, w: u32, h: u32) -> Result<Self::TextureId, BackendError>;
///     fn upload(&mut self, id: &Self::TextureId, data: &[u8]) -> Result<(), BackendError>;
///     fn stats(&self) -> (u32, usize);
/// }
///
/// struct GpuBackend { name: &'static str, textures: Vec<GpuTexture>, bytes: usize }
/// struct CpuBackend { ... }
///
/// fn run_generic<B: Backend>(b: &mut B, w: u32, h: u32) -> Result<u32, BackendError>;
/// fn run_dyn(b: &mut dyn Backend, w: u32, h: u32) -> Result<u32, BackendError>;
/// ```
///
/// ⚠ С `dyn Backend` будет проблема: associated type запрещён в
/// dyn-совместимом трейте без `where Self: Sized`. Разберись, как это
/// обойти (ответ: убрать associated type, заменить на общий тип),
/// и прокомментируй в коде, почему так правильнее для плагин-системы.
///
/// Требования:
///   * `BackendError` — свой тип (Display + Debug + Error), минимум
///     варианта `OutOfMemory { w, h }`.
///   * `run_generic` и `run_dyn` оба компилируются. Для `run_dyn`
///     тебе придётся сузить трейт. Прокомментируй решение.
fn task_6_1() {
    use std::fmt;

    #[derive(Debug, PartialEq)]
    struct BackendError {
        what: &'static str,
    }

    impl fmt::Display for BackendError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            not_yet!("write!(f, \"{{}}: {{}}\", self.what, ...)");
        }
    }

    impl std::error::Error for BackendError {}

    // Associated type УБРАН — иначе трейт не dyn-совместим и `dyn Backend`
    // невозможен. Идентификаторы стали u32 у обоих бэкендов.
    trait Backend {
        fn name(&self) -> &str;
        fn create_texture(&mut self, w: u32, h: u32) -> Result<u32, BackendError>;
        fn upload(&mut self, id: &u32, data: &[u8]) -> Result<(), BackendError>;
        fn stats(&self) -> (u32, usize);
    }

    struct GpuBackend {
        textures: Vec<(u32, u32)>,
        bytes: usize,
    }

    impl GpuBackend {
        fn new() -> Self {
            GpuBackend { textures: Vec::new(), bytes: 0 }
        }
    }

    impl Backend for GpuBackend {
        fn name(&self) -> &str {
            "gpu"
        }
        fn create_texture(&mut self, w: u32, h: u32) -> Result<u32, BackendError> {
            not_yet!("w*h*4 == 0 -> Err; иначе push и верни индекс");
        }
        fn upload(&mut self, id: &u32, data: &[u8]) -> Result<(), BackendError> {
            not_yet!("self.textures.get_mut(*id) -> Err если нет; иначе bytes += data.len()");
        }
        fn stats(&self) -> (u32, usize) {
            (self.textures.len() as u32, self.bytes)
        }
    }

    struct CpuBackend {
        textures: Vec<Vec<u8>>,
    }

    impl CpuBackend {
        fn new() -> Self {
            CpuBackend { textures: Vec::new() }
        }
    }

    impl Backend for CpuBackend {
        fn name(&self) -> &str {
            "cpu"
        }
        fn create_texture(&mut self, w: u32, h: u32) -> Result<u32, BackendError> {
            not_yet!("та же логика");
        }
        fn upload(&mut self, id: &u32, data: &[u8]) -> Result<(), BackendError> {
            not_yet!("та же логика");
        }
        fn stats(&self) -> (u32, usize) {
            (self.textures.len() as u32, self.textures.iter().map(|t| t.len()).sum())
        }
    }

    fn run_generic<B: Backend>(b: &mut B, w: u32, h: u32) -> Result<u32, BackendError> {
        not_yet!("create_texture + upload 4 байта + верни количество текстур");
    }

    fn run_dyn(b: &mut dyn Backend, w: u32, h: u32) -> Result<u32, BackendError> {
        not_yet!("то же самое, но через dyn — и это работает, потому что трейт dyn-совместим");
    }

    let mut g = GpuBackend::new();
    assert_eq!(run_generic(&mut g, 4, 4).unwrap(), 1);
    assert_eq!(run_dyn(&mut g, 8, 8).unwrap(), 2);
    assert_eq!(g.stats(), (2, 4 + 256));

    let mut c = CpuBackend::new();
    assert_eq!(run_generic(&mut c, 2, 2).unwrap(), 1);
    assert!(run_generic(&mut c, 0, 0).is_err(), "нулевой размер — ошибка");
}

/// ЧАСТЬ 6.5 — ЗАДАНИЕ 6.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Трейт `Asset` плюс полный набор стандартных трейтов, которые нужен
/// любому типу в движке. Это ровно то, что делает `serde::Deserialize`
/// в реальном проекте, только на голом `std`.
///
/// ```ignore
/// #[derive(Debug, Clone, PartialEq, Eq, Hash)]
/// struct AssetId(u32);
///
/// #[derive(Debug, Clone, PartialEq)]
/// enum Asset {
///     Mesh { name: String, vertex_count: u32 },
///     Texture { name: String, w: u32, h: u32 },
///     Shader { name: String, code: String },
/// }
///
/// impl AssetId { fn next(counter: &mut u32) -> AssetId; }
///
/// impl std::str::FromStr for Asset {          // "mesh:hero:1024"
///     type Err = ParseAssetError;
///     fn from_str(s: &str) -> Result<Asset, Self::Err>;
/// }
///
/// impl std::fmt::Display for Asset { ... }      // обратно в строку
/// impl Default for Asset { ... }                // пустой меш
/// impl std::error::Error for ParseAssetError { ... }
/// ```
///
/// Требования:
///   * `from_str`/`Display` — взаимно обратные: `to_string().parse() == to_string()`.
///   * `ParseAssetError` — enum с минимум двумя вариантами (плохой префикс,
///     плохие числа). Никаких `unwrap`.
///   * `Default` — `Mesh` с пустым именем и 0 вершин.
fn task_6_2() {
    use std::str::FromStr;

    #[derive(Debug, Clone, PartialEq, Eq, Hash)]
    struct AssetId(u32);

    impl AssetId {
        fn next(counter: &mut u32) -> AssetId {
            not_yet!("AssetId(*counter) и увеличить");
        }
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Asset {
        Mesh { name: String, vertex_count: u32 },
        Texture { name: String, w: u32, h: u32 },
        Shader { name: String, code: String },
    }

    #[derive(Debug, PartialEq)]
    enum ParseAssetError {
        BadKind(String),
        BadNumber { field: &'static str, value: String },
    }

    impl std::fmt::Display for ParseAssetError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            not_yet!("match по вариантам, ничего не паникуя");
        }
    }

    impl std::error::Error for ParseAssetError {}

    impl FromStr for Asset {
        type Err = ParseAssetError;
        fn from_str(s: &str) -> Result<Asset, Self::Err> {
            not_yet!("split(':') — kind:name:числа, parse().map_err(BadNumber)");
        }
    }

    impl std::fmt::Display for Asset {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            not_yet!("обратная к from_str форма");
        }
    }

    impl Default for Asset {
        fn default() -> Self {
            not_yet!("пустой меш");
        }
    }

    let mut counter = 0;
    assert_eq!(AssetId::next(&mut counter), AssetId(0));
    assert_eq!(AssetId::next(&mut counter), AssetId(1));

    let a: Asset = "mesh:hero:1024".parse().expect("валидный");
    assert_eq!(a, Asset::Mesh { name: "hero".into(), vertex_count: 1024 });

    let roundtrip = a.to_string().parse::<Asset>().expect("round-trip");
    assert_eq!(roundtrip, a, "Display и FromStr должны быть обратными");

    assert!("tex:sky:x".parse::<Asset>().is_err());
    assert!("cube:a:b".parse::<Asset>().is_err());
    assert_eq!(Asset::default(), Asset::Mesh { name: String::new(), vertex_count: 0 });
}

/// ЧАСТЬ 6.6 — ЗАДАНИЕ 6.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Реестр плагинов. Архитектура, которая будет в `Main/src`.
///
/// ```ignore
/// struct Registry { plugins: Vec<Box<dyn Plugin>> }
/// trait Plugin {
///     fn name(&self) -> &str;
///     fn priority(&self) -> i32;                  // меньше = раньше
///     fn configure(&mut self, cfg: &str) -> Result<(), String>;
/// }
///
/// impl Registry {
///     fn add(&mut self, p: Box<dyn Plugin>);
///     fn run_all(&mut self, cfg: &str) -> Vec<String>;   // сортировка по priority
///     fn find_by_name(&self, name: &str) -> Option<&dyn Plugin>;
/// }
/// ```
///
/// Требования:
///   * `run_all` возвращает отчёт: `"имя: ok"` или `"имя: ошибка — текст"`.
///     Ошибка одного плагина **не** должна отменять остальные.
///   * Сортировка по `priority` (стабильная, `sort_by_key`).
///   * `find_by_name` возвращает `&dyn Plugin` — «абстрактная ссылка».
///   * Добавь `fn total_priority(&self) -> i32` как **generic** на `&self`
///     с `impl Iterator<Item = &dyn Plugin>` — покажи, что `impl Trait`
///     в аргументе позволяет и обобщённость, и отсутствие vtable.
fn task_6_3() {
    trait Plugin {
        fn name(&self) -> &str;
        fn priority(&self) -> i32;
        fn configure(&mut self, cfg: &str) -> Result<(), String>;
    }

    struct Registry {
        plugins: Vec<Box<dyn Plugin>>,
    }

    impl Registry {
        fn add(&mut self, p: Box<dyn Plugin>) {
            not_yet!("push");
        }

        fn run_all(&mut self, cfg: &str) -> Vec<String> {
            not_yet!("сортируй копии приоритетов, потом configure у каждого, ошибки не отменяют остальных");
        }

        fn find_by_name(&self, name: &str) -> Option<&dyn Plugin> {
            not_yet!("iter().find(|p| p.name() == name).map(|p| p.as_ref())");
        }
    }

    struct Recorder {
        name: &'static str,
        prio: i32,
        fails: bool,
    }

    impl Plugin for Recorder {
        fn name(&self) -> &str {
            self.name
        }
        fn priority(&self) -> i32 {
            self.prio
        }
        fn configure(&mut self, cfg: &str) -> Result<(), String> {
            if self.fails {
                Err(format!("{}: не нравится cfg={cfg}", self.name))
            } else {
                Ok(())
            }
        }
    }

    let mut r = Registry { plugins: Vec::new() };
    r.add(Box::new(Recorder { name: "late", prio: 10, fails: false }));
    r.add(Box::new(Recorder { name: "early", prio: 1, fails: true }));
    r.add(Box::new(Recorder { name: "mid", prio: 5, fails: false }));

    let report = r.run_all("msaa=4");
    assert_eq!(report.len(), 3, "ошибка одного не отменяет остальных");
    assert!(report[0].starts_with("early"), "сортировка по priority: {report:?}");
    assert!(report[0].contains("не нравится"));
    assert!(report[2].starts_with("late"), "{report:?}");

    assert!(r.find_by_name("mid").is_some());
    assert!(r.find_by_name("nope").is_none());
    assert_eq!(r.find_by_name("early").map(|p| p.priority()), Some(1));
}
