//! ============================================================================
//! МОДУЛЬ 10 — МОДУЛИ, ВИДИМОСТЬ, CRATE'Ы, CARGO
//! ============================================================================
//!
//! ЧАСТЬ 10.1 — ТЕОРИЯ: модули и видимость
//! ─────────────────────────────────────────────────────────────────────────────
//! Rust разбивает код на `module`. Модуль — это НЕ папка и НЕ просто
//! namespace. Модуль — это граница видимости и одновременно единица
//! компиляции внутри крейта.
//!
//! ЧЕТЫРЕ УРОВНЯ ВИДИМОСТИ (в порядке убывания):
//!
//!   (пусто)      — приватно. Видно ТОЛЬКО в текущем модуле и его детях.
//!   pub(super)   — виден в родительском модуле.
//!   pub(crate)   — виден во всём крейте. ⚠ Это рабочая лошадь: всё
//!                  внутреннее — `pub(crate)`, наружу — минимум.
//!   pub          — виден всем, кто подключил крейт.
//!   pub(in path) — видимость по конкретному пути (`pub(in crate::render)`).
//!
//! СРАВНИ С C++:
//!   `private:` в заголовке  ≈ без модификатора
//!   `protected:`            ≈ `pub(super)` (примерно)
//!   `public:`               ≈ `pub`
//!   `friend class`          ≈ `pub(crate)` или приватный модуль
//!
//! ГЛАВНОЕ ПРАВИЛО, КОТОРОГО НЕТ В C++ — «УТЕЧКА ПРИВАТНОСТИ» (leak).
//!
//! Если приватная функция возвращает публичный тип, в котором есть
//! приватные поля, наружу её отдать нельзя: пользователь не сможет
//! вызвать, не имея доступа к полям. В C++ тот же код сработал бы, и
//! пользователь получил бы объект, который не может правильно
//! скопировать. Rust заставляет сделать API честным: сначала реши,
//! кому поле нужно, потом ставь `pub`.
//!
//! ⚠ ИНФОРМАЦИЯ ТЕЧЁТ ЧЕРЕЗ ТИПЫ. Приватное поле публичного типа уже
//!   «выдаёт» наружу сведения о существовании этого типа. Rust считает
//!   это допустимым — осознанный размен в пользу простоты.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

fn main() {
    harness::module(10, "Модули, видимость, crate'ы, Cargo");

    part_10_1(); // видимость, приватность, re-export
    part_10_2(); // Cargo: крейты, features, profiles, env!, cfg

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("10.1  Скелет движка: модули с правильной видимостью", task_10_1);
    r.task("10.2  Feature-флаги и build.rs", task_10_2);
    r.task("10.3  Узкий публичный API vs широкий pub(crate)", task_10_3);
    r.summary(10);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 10.1 — ПРИМЕР: дерево модулей одного файла
// ─────────────────────────────────────────────────────────────────────────────

/// Это «модуль-файл» (module 10). Его видимость снаружи определяется тем,
/// что помечено `pub` на верхнем уровне.
mod engine {
    /// Публично наружу.
    pub struct App {
        pub(crate) window: Window,
        config: Config, // приватно
    }

    impl App {
        pub fn new(name: &str) -> App {
            App { window: Window::new(name), config: Config::default() }
        }

        /// Приватный метод — вызывается только внутри `engine`.
        #[allow(dead_code)]
        fn tune(&mut self) -> &mut Config {
            &mut self.config
        }

        /// `pub(crate)`: доступен из `crate::debug_tools` внизу,
        /// но не снаружи крейта.
        pub(crate) fn window_size(&self) -> (u32, u32) {
            self.window.size
        }

        /// Публичный метод. Приватное поле конфига наружу не торчит:
        /// отдаётся ЗНАЧЕНИЕ, а не поле.
        pub fn quality(&self) -> Quality {
            self.config.quality
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
    pub enum Quality {
        Low,
        #[default]
        Medium,
        High,
    }

    /// Приватная структура: наружу не отдаётся никогда.
    #[derive(Default)]
    struct Config {
        quality: Quality,
        vsync: bool,
    }

    pub struct Window {
        pub(crate) size: (u32, u32),
        title: String, // приватно
    }

    impl Window {
        pub fn new(title: &str) -> Window {
            Window { size: (1920, 1080), title: title.to_string() }
        }

        /// Геттер, потому что поле приватно. Это не бюрократия:
        /// так можно потом добавить проверку или ленивое кеширование.
        pub fn title(&self) -> &str {
            &self.title
        }
    }

    /// Вложенный публичный модуль. Снаружи доступен как `engine::render`.
    pub mod render {
        use super::{Quality, Window};

        pub struct Renderer {
            passes: u32,
        }

        impl Renderer {
            pub fn new(w: &Window) -> Renderer {
                Renderer { passes: if w.title() == "main" { 3 } else { 1 } }
            }
            pub fn passes(&self) -> u32 {
                self.passes
            }
        }

        /// Quality публичен в `engine`, а не в `engine::render`:
        /// путь `engine::render::Quality` не существует, нужно
        /// `use super::Quality`. Это нормально — смотри на re-export ниже.
        pub fn preferred() -> Quality {
            Quality::High
        }
    }

    /// Приватный модуль — доступен только внутри `engine`.
    mod internal {
        pub fn helper() -> u32 {
            42
        }
    }

    pub fn call_internal() -> u32 {
        // Приватный модуль виден здесь, потому что мы внутри `engine`.
        internal::helper()
    }
}

/// `crate::debug_tools` видит `pub(crate)` поля, но не приватные.
mod debug_tools {
    use crate::engine::App;

    pub fn dump(app: &App) -> String {
        // window_size — pub(crate), доступно.
        let (w, h) = app.window_size();
        format!("window {w}x{h}, quality {:?}", app.quality())
    }
}

fn part_10_1() {
    harness::part("10.1", "ПРИМЕР: видимость, приватность, re-export");

    let mut app = engine::App::new("main");
    let r = engine::render::Renderer::new(&app.window);
    println!("  App::quality()    = {:?}", app.quality());
    println!("  window_size()     = {:?}", app.window_size());
    println!("  Renderer::passes()= {}", r.passes());
    println!("  render::preferred = {:?}", engine::render::preferred());
    println!("  call_internal()   = {}", engine::call_internal());
    println!("  debug_tools::dump = {}", debug_tools::dump(&app));

    // app.tune();                  // ⛔ E0624: method `tune` is private
    // println!("{:?}", app.config);  // ⛔ E0616: field `config` is private

    app.window.size = (1280, 720); // size — pub(crate), из другого модуля крейта ОК
    println!("  после app.window.size = ...: {:?}", app.window_size());

    println!("\n  Window::title() = {:?} (приватное поле через геттер)", app.window.title());
    println!("  ⚠ публичная функция, возвращающая публичную структуру с приватными");
    println!("    полями и ДОЛЖНАЯ быть вызванной снаружи = E0446. Приватность течёт.");
}

/// Демонстрация утечки приватности: ЕСЛИ БЫ ЭТА ФУНКЦИЯ БЫЛА `pub`,
/// компилятор потребовал бы сделать `Window.title` публичным, иначе
/// внешний код не смог бы её вызвать.
#[allow(dead_code)]
fn leak_demo(w: &engine::Window) -> engine::Quality {
    let _ = w.title();
    engine::Quality::Medium
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 10.2 — ТЕОРИЯ: Cargo и крейты
// ─────────────────────────────────────────────────────────────────────────────
//
// ЧЕТЫРЕ СУЩНОСТИ, КОТОРЫЕ ПОСТОЯННО ПУТАЮТ:
//
//   package   — то, что лежит в Cargo.toml (имя, версия, зависимости).
//   crate     — результат компиляции одного package: библиотека ИЛИ
//               бинарник. Из одного package можно получить оба.
//   target    — конкретный бинарник или библиотека. Их может быть
//               несколько: [[bin]] x20 в Curriculum.
//   module    — единица видимости ВНУТРИ крейта (часть 10.1).
//
// WORKSPACE — набор package'ов, которые собираются вместе и делят
// target/ и Cargo.lock. В этом репозитории: `Main` (движок) и
// `Curriculum` (курс) — два package'а одного воркспейса.
//
// FEATURES — флаги, которые меняют код, но не добавляют зависимости.
//
// ```toml
// [features]
// default = ["vulkan"]
// hot-reload = []
// ```
//
// В коде: `#[cfg(feature = "vulkan")]`. У зависимости
// `dep = { version = "1", optional = true }` появляется такая же фича.
//
// ⚠ ПРАВИЛО: фичи — это ОБЯЗАТЕЛЬСТВА API, а не «переключатели удобства».
//   Если две фичи могут быть выключены несовместимо — это ошибка
//   проектирования, и компилятор её не поймает.
//
// PROFILES — оптимизации. В этом репозитории `dev` имеет
// `opt-level = 1` намеренно: иначе бенчмарки в модулях 13 и 16
// измеряли бы компилятор, а не твой код.
//
// BUILD.RS — код, который выполняется ДО сборки. Умеет читать env и
// файлы, генерировать код, сообщать cargo о зависимостях
// (`cargo:rerun-if-changed=..`) и находить системные библиотеки.
// ⚠ `build.rs` НЕ ДОЛЖЕН ПИСАТЬ В ИСХОДНИКИ. Только в `OUT_DIR`.

fn part_10_2() {
    harness::part("10.2", "ПРИМЕР: что cargo даёт коду");

    // env! — макрос, подставляет значение НА ЭТАПЕ КОМПИЛЯЦИИ.
    // В рантайме такой константы нет: байты зашиты в бинарник.
    println!("  env!(\"CARGO_PKG_NAME\")    = {}", env!("CARGO_PKG_NAME"));
    println!("  env!(\"CARGO_PKG_VERSION\") = {}", env!("CARGO_PKG_VERSION"));
    println!("  env!(\"CARGO_PKG_AUTHORS\") = {}", env!("CARGO_PKG_AUTHORS"));
    println!("  option_env!(\"OUT_DIR\")     = {}", option_env!("OUT_DIR").unwrap_or("(нет build.rs)"));

    // --- cfg: условная компиляция ------------------------------------------
    #[cfg(target_pointer_width = "64")]
    let ptr_width = 64;
    #[cfg(target_pointer_width = "32")]
    let ptr_width = 32;
    println!("  cfg(target_pointer_width) = {ptr_width}");

    #[cfg(target_os = "windows")]
    let os = "windows";
    #[cfg(target_os = "linux")]
    let os = "linux";
    #[cfg(target_os = "macos")]
    let os = "macos";
    println!("  cfg(target_os) = {os}");

    #[cfg(debug_assertions)]
    let mode = "dev (debug-assertions ВКЛ: переполнение и границы паникуют)";
    #[cfg(not(debug_assertions))]
    let mode = "release (debug-assertions ВЫКЛ: переполнение заворачивается!)";
    println!("  debug_assertions: {mode}");
    println!("    разделитель пути на этой платформе: {:?}", path_separator());

    // cfg_attr — «примени атрибут, если условие верно».
    #[cfg_attr(debug_assertions, inline(never))]
    fn profiled() -> u32 {
        1
    }
    let _ = profiled();
    println!("  cfg_attr(debug_assertions, inline(never)) — в dev не инлайнится");

    // --- Свои фичи (объявлены в Curriculum/Cargo.toml) ---------------------
    // Проверь:
    //   cargo run --bin module10
    //   cargo run --features hot-reload --bin module10
    #[cfg(feature = "hot-reload")]
    println!("  фича hot-reload ВКЛЮЧЕНА");
    #[cfg(not(feature = "hot-reload"))]
    println!("  фича hot-reload выключена (по умолчанию)");

    // --- Файловая система: std::fs ------------------------------------------
    use std::fs;
    let probe = std::env::temp_dir().join("novell_probe.txt");
    let _ = fs::write(&probe, b"data");
    let read_back = fs::read_to_string(&probe).ok();
    let len = fs::metadata(&probe).map(|m| m.len()).unwrap_or(0);
    println!("  fs round-trip: {read_back:?} ({len} байт)");
    let _ = fs::remove_file(&probe);
    // ⚠ Каждый вызов fs возвращает io::Result. В модуле 7 мы превращали
    //   его в EngineError через From — вот зачем нужен From.

    // --- Замер: разница dev/release -----------------------------------------
    let t = std::time::Instant::now();
    let mut acc = 0u64;
    for i in 0..200_000u64 {
        acc = acc.wrapping_add(i);
    }
    println!("  200k итераций за {:?} (acc={acc})", t.elapsed());
    println!("    в release это в 10-50 раз быстрее. Проверь:");
    println!("      cargo run --release --bin module10");
}

/// Разделитель пути — одна функция, две реализации через cfg.
/// Так пишут кроссплатформенный код без макросов.
#[cfg(target_os = "windows")]
const fn path_separator() -> char {
    '\\'
}

#[cfg(not(target_os = "windows"))]
const fn path_separator() -> char {
    '/'
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 10.3 — ЗАДАНИЕ 10.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Разбей монолит на модули. Проверяется не «красиво», а конкретными
/// требованиями по видимости — и компилятором.
///
/// ВАЖНО: модули объявлены на УРОВНЕ ФАЙЛА, а не внутри функции. Так
/// они выглядят в реальном движке. Модуль внутри функции — анонимная
/// область: соседние модули внутри блока не видят друг друга по пути,
/// потому что `mod` не меняет модульное дерево. Именно поэтому
/// `lesson_render` ниже пишет `super::lesson_scene::Scene`, а не
/// `scene::Scene`.
///
/// Требования:
///   * `Scene` — `pub` наружу.
///   * Поле `Scene::transform` — `pub(crate)`: render читает, снаружи нет.
///   * Поле `Scene::debug_name` — приватно, доступ только через геттер.
///   * `mod validation` — приватный; его функция вызывается из `scene`.
///   * `lesson_render` НЕ должен иметь доступа к приватным полям
///     `Scene`: если попытаться — E0616. Это и есть смысл приватности.
///   * `pub use` в корне: наружу красивое имя, внутри длинный путь.
mod lesson_scene {
    use curriculum::not_yet;

    pub struct Scene {
        pub(crate) transform: Transform,
        entities: Vec<u32>,
        debug_name: String,
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub(crate) struct Transform {
        pub x: f32,
        pub y: f32,
    }

    impl Scene {
        pub fn new(name: &str) -> Self {
            not_yet!("transform по умолчанию, пустой entities, debug_name = name");
        }

        pub(crate) fn spawn(&mut self) -> u32 {
            not_yet!("push и верни индекс");
        }

        pub fn len(&self) -> usize {
            self.entities.len()
        }

        pub fn is_empty(&self) -> bool {
            self.entities.is_empty()
        }

        /// Приватное поле наружу — только через геттер.
        pub fn debug_name(&self) -> &str {
            &self.debug_name
        }

        /// Вызов приватного модуля из метода своего модуля.
        pub fn validate(&self) -> Result<(), String> {
            not_yet!("вызови validation::check(self)");
        }
    }

    /// Публичный вложенный модуль: снаружи lesson_scene::graph.
    pub mod graph {
        #[derive(Debug, Default)]
        pub struct Graph {
            pub nodes: usize,
            pub edges: usize,
        }
    }

    /// Приватный модуль: виден только внутри `lesson_scene`.
    mod validation {
        use curriculum::not_yet;
        use super::Scene;

        pub fn check(s: &Scene) -> Result<(), String> {
            not_yet!("сцена не должна быть пустой, а transform — не NaN");
        }
    }
}

mod lesson_render {
    use curriculum::not_yet;
    // render видит `lesson_scene::Transform` (pub(crate)),
    // но НЕ видит приватный debug_name.
    use super::lesson_scene::Scene;

    pub fn draw(s: &Scene) -> String {
        not_yet!("используй ТОЛЬКО s.transform и s.len()");
    }
}

// re-export: наружу торчит красивое имя, внутри остаётся полный путь.
pub use lesson_scene::Scene as EngineScene;

fn task_10_1() {
    use lesson_scene::Scene;

    let mut s = Scene::new("level_1");
    assert_eq!(s.debug_name(), "level_1");
    assert!(s.is_empty());

    let id = s.spawn();
    assert_eq!((id, s.len()), (0, 1));
    assert!(s.validate().is_ok());

    let drawn = lesson_render::draw(&s);
    assert!(drawn.contains("1"), "{drawn}");

    // Приватность снаружи: `debug_name` — только через геттер,
    // а `transform` виден внутри крейта (он pub(crate)).
    let t = s.transform;
    assert_eq!((t.x, t.y), (0.0, 0.0));
}

/// ЧАСТЬ 10.4 — ЗАДАНИЕ 10.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Feature-флаги и `build.rs`. Две части: код и файл.
///
/// ЧАСТЬ А (код). Сделай так, чтобы поведение менялось по фиче:
/// ```ignore
/// fn render_loop_hint() -> String { /* две версии через #[cfg] */ }
/// ```
/// Проверь:
///   cargo run --bin module10
///   cargo run --features hot-reload --bin module10
/// Фича `hot-reload` уже объявлена в Curriculum/Cargo.toml.
///
/// ЧАСТЬ Б (build.rs). Создай Curriculum/build.rs, который:
///   1. Пишет в `$OUT_DIR/generated.rs` строку с числом секунд сборки
///      (только `std::time`, никаких внешних крейтов).
///   2. Печатает `cargo:rerun-if-changed=build.rs`.
///   3. В коде: `include!(concat!(env!("OUT_DIR"), "/generated.rs"));`
///      и константа `BUILD_STAMP` доступна.
///
/// Требования:
///   * Никаких записей в исходники, только в OUT_DIR.
///   * Раскомментируй код и проверь, что он работает.
fn task_10_2() {
    // ── ЧАСТЬ А: две версии функции по фиче ───────────────────────────────
    fn render_loop_hint() -> String {
        #[cfg(feature = "hot-reload")]
        {
            "цикл со слежением за файлами: перезагрузка при изменении".to_string()
        }
        #[cfg(not(feature = "hot-reload"))]
        {
            not_yet!("обычный цикл без слежения за файлами");
        }
    }

    let hint = render_loop_hint();
    assert!(!hint.is_empty());
    println!("  render_loop_hint = {hint}");

    // ── ЧАСТЬ Б: build.rs ─────────────────────────────────────────────────
    // Создай Curriculum/build.rs ровно таким содержимым:
    //
    //     use std::{env, fs, path::PathBuf, time::{SystemTime, UNIX_EPOCH}};
    //
    //     fn main() {
    //         println!("cargo:rerun-if-changed=build.rs");
    //         let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR задан cargo"));
    //         let secs = SystemTime::now()
    //             .duration_since(UNIX_EPOCH)
    //             .map(|d| d.as_secs())
    //             .unwrap_or(0);
    //         let dest = out.join("generated.rs");
    //         fs::write(&dest, format!("pub const BUILD_STAMP: u64 = {secs};\n"))
    //             .expect("пишем в OUT_DIR");
    //     }
    //
    // Затем раскомментируй в этом файле:
    //
    //     include!(concat!(env!("OUT_DIR"), "/generated.rs"));
    //     println!("BUILD_STAMP = {BUILD_STAMP}");
    //
    // и проверь: cargo run --bin module10
    let _ = env!("CARGO_PKG_VERSION");
}

/// ЧАСТЬ 10.5 — ЗАДАНИЕ 10.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Узкий публичный API — не паранойя, а удобство. Наружу торчит ровно
/// то, чем пользуются другие модули; остальное — `pub(crate)`.
///
/// ```ignore
/// mod cache {
///     pub struct Cache<K, V> { ... }
///     impl<K, V> Cache<K, V> {
///         pub fn new(cap: usize) -> Self;         // нужен наружу
///         pub fn len(&self) -> usize;            // нужен наружу
///         pub fn insert/get/clear(...)          // свой API, скрывает устройство
///         pub(crate) fn stat(&self) -> CacheStat;  // ТОЛЬКО внутри крейта
///     }
///     #[derive(Clone, Copy)] pub(crate) struct CacheStat { ... }
/// }
/// ```
///
/// Требования:
///   * LRU на `Vec<K>` для порядка + `HashMap` для значений.
///   * `hits` — приватное поле, отдаётся только через `stat()`.
///   * Добавь новое приватное поле и убедись, что снаружи ничего
///     не изменилось — это и есть смысл сокрытия.
///   * Ответь в комментарии: чем плохо торчать `raw` наружу?
mod task_cache_source {}

mod lesson_cache {
    use curriculum::not_yet;
    use std::collections::HashMap;

    pub struct Cache<K, V> {
        raw: HashMap<K, V>,
        order: Vec<K>, // порядок обращения, для LRU
        hits: u64,
    }

    #[derive(Debug, Clone, Copy, Default)]
    pub(crate) struct CacheStat {
        pub hits: u64,
    }

    impl<K: std::hash::Hash + Eq + Clone, V: Clone> Cache<K, V> {
        pub fn new(cap: usize) -> Self {
            not_yet!("пустые raw/order и order.reserve(cap)");
        }

        pub fn len(&self) -> usize {
            self.raw.len()
        }

        pub fn capacity(&self) -> usize {
            not_yet!("self.order.capacity()");
        }

        pub fn insert(&mut self, k: K, v: V) {
            not_yet!("убери k из order (через position + remove), вставь в конец, raw.insert");
        }

        pub fn get(&mut self, k: &K) -> Option<&V> {
            not_yet!("при попадании hits += 1 и перемести k в конец order");
        }

        pub fn clear(&mut self) {
            self.raw.clear();
            self.order.clear();
        }

        /// ТОЛЬКО внутри крейта: снимок статистики.
        pub(crate) fn stat(&self) -> CacheStat {
            CacheStat { hits: self.hits }
        }
    }
}

fn task_10_3() {
    use lesson_cache::Cache;

    let mut c = Cache::<&str, u32>::new(4);
    c.insert("a", 1);
    c.insert("b", 2);
    assert_eq!(c.len(), 2);
    assert!(c.capacity() >= 4);

    assert_eq!(c.get(&"a"), Some(&1));
    assert_eq!(c.get(&"zz"), None);
    assert_eq!(c.stat().hits, 1, "stat доступен внутри крейта");

    c.clear();
    assert_eq!(c.len(), 0);
    println!("  публичный API узкий: new/insert/get/len/capacity, остальное — pub(crate)");
}
