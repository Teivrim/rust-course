//! ============================================================================
//! МОДУЛЬ 07 — ОБРАБОТКА ОШИБОК
//! ============================================================================
//!
//! ЧАСТЬ 7.1 — ТЕОРИЯ
//! ─────────────────────────────────────────────────────────────────────────────
//! В Rust ДВА механизма ошибок, и их нельзя путать:
//!
//!   Result<T, E> — ожидаемая, обрабатываемая ошибка. ЧАСТЬ ТИПА.
//!   panic        — «я не могу продолжить». НЕ является обработкой ошибки.
//!
//! ОТСУТСТВИЕ ИСКЛЮЧЕНИЙ — НЕ «НЕТ ОБРАБОТКИ ОШИБОК», а перенос их в
//! ТИПЫ. `Result<T, MyError>` в сигнатуре функции говорит: «здесь может
//! быть ошибка MyError», и вызывающий обязан это учесть. В C++
//! `void f()` не говорит ничего: `throw` может вылететь откуда угодно.
//!
//! ПОЧЕМУ НЕ ИСКЛЮЧЕНИЯ В RUST:
//!   1. Unwinding (раскрутка стека) дорог и непредсказуем по задержке.
//!   2. Исключение может вылететь из деструктора → double panic → abort.
//!   3. Ловить исключения в C++ — это искать в коде; в Rust ты обязан
//!      разобрать вариант компилятором.
//!   4. `?` в C++ есть только в C++23, и он не умеет конвертировать
//!      типы ошибок. В Rust `?` — одна строка, и он вызывает `From`.
//!
//! ЧТО ПРОИСХОДИТ ПРИ ПАНИКЕ (и почему это не так страшно):
//!   1. Печатается сообщение в stderr.
//!   2. Стек РАСКРУЧИВАЕТСЯ (unwind): вызываются `Drop` для всех живых
//!      значений в порядке, обратном объявлению.
//!   3. Если настроен `panic = "abort"` — раскрутки нет, `Drop` НЕ
//!      вызывается. Это ловушка: в профиле release многие ставят abort
//!      ради скорости и теряют RAII-гарантии. В этом репозитории
//!      abort НАМЕРЕННО не включён, и вот почему.
//!
//! ⚠ Отсюда правило: **никогда не пиши `Drop`, который сам паникует**
//!   иначе будет double panic → abort без раскрутки. В модуле 18 это
//!   критично: Drop у Vulkan-объекта вызывает C-функцию, которая может
//!   запаниковать.
//!
//! `catch_unwind` — «ловить паники вручную». Нужен редко: в FFI-мостах
//!   (чтобы C-исключение не прошло через Rust) и в тестах. Плата:
//!   `UnwindSafe`-типы, запрет на `&mut` без обёртки `AssertUnwindSafe`.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

fn main() {
    harness::module(7, "Обработка ошибок");

    part_7_1(); // Result, ?, цепочка конверсий
    part_7_2(); // паника: что происходит и как её ловить
    part_7_3(); // иерархия ошибок и агрегация

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("7.1  Реализовать From-цепочку конверсий ошибок", task_7_1);
    r.task("7.2  Иерархия EngineError + агрегатор + exit code", task_7_2);
    r.task("7.3  Что паникует, а что возвращает Err", task_7_3);
    r.summary(7);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 7.1 — ТЕОРИЯ: оператор ?
// ─────────────────────────────────────────────────────────────────────────────
//
// `expr?` — это НЕ «try». Это три действия в одном:
//
//     match expr {
//         Ok(v)  => v,
//         Err(e) => return Err(From::from(e)),   // ← вся магия тут
//     }
//
// Два следствия:
//   1. Тип ошибки в `return` КОНВЕРТИРУЕТСЯ через `From`. Поэтому
//      `std::fs::read_to_string(...)?` работает в функции, возвращающей
//      `Result<_, MyError>`, без единого `map_err`.
//   2. Внутри `try`-блока `?` не нужен, но на самом деле
//      `std::fs::read_to_string` просто возвращает `io::Error`.
//
// СРАВНИ С C++23 `std::expected` — там `?` тоже конвертирует, но
// механизм появился на 10 лет позже и работает только с `expected`.
//
// ⚠ ТИПОЧНАЯ ОШИБКА: `?` в функции, возвращающей НЕ Result.
//   Если хочешь «выйти с None» — в Rust 1.x `?` не работает для
//   Option, приходится писать match вручную. Это осознанно:
//   «Option-ошибки» и «Error-ошибки» — разные вещи, смешивать их
//   нельзя, иначе теряется сообщение об ошибке.

fn part_7_1() {
    harness::part("7.1", "ПРИМЕР: Result, ? и цепочка From");

    // --- Базовое различие с Option ------------------------------------------
    let found: Option<u32> = Some(1);
    let failed: Result<u32, String> = Err(String::from("дискуссия"));
    println!("  Option: {found:?}  |  Result: {failed:?}");

    // --- ? подставляет Err, а Ok «пропускает насквозь» ---------------------
    match parse_and_double("21") {
        Ok(v) => println!("  parse_and_double(\"21\") = {v}"),
        Err(e) => println!("  ошибка: {e}"),
    }
    match parse_and_double("abc") {
        Ok(v) => println!("  неожиданно {v}"),
        Err(e) => println!("  parse_and_double(\"abc\") → Err({e})"),
    }

    // --- Вложенные вызовы: ошибка конвертируется НА КАЖДОМ уровне ----------
    let path = "assets/hero.mesh";
    match load_scene(path) {
        Ok(n) => println!("  load_scene → загружено ассетов: {n}"),
        Err(e) => println!("  load_scene → Err: {e}"),
    }
    match load_scene("assets/missing.mesh") {
        Ok(_) => println!("  не должно случиться"),
        Err(e) => println!("  load_scene(несуществующий) → Err: {e}\n     └─ цепочка Io→Loader→Engine сохранена в Display"),
    }

    // --- Где `?` НЕЛЬЗЯ ---------------------------------------------------
    // fn f() -> u32 { let x = g()?; x }   // ⛔ E0277: `?` требует Result/Option
    // fn g() -> Result<u32, String> { Ok(1) }
    println!("\n  `?` требует, чтобы функция возвращала Result/Option/Try — иначе E0277");

    // --- map_err: ручная конверсия, когда From не подходит -----------------
    let r: Result<u32, String> = "42".parse::<u32>().map_err(|e| format!("плохое число: {e}"));
    println!("  map_err: {r:?}");

    // --- Три уровня «неполноты» ошибки --------------------------------------
    println!("\n  ── три уровня ──");
    println!("  1) ожидаем и обработаем: {}", fmt_result(read_ok()));
    println!("  2) сложим и покажем позже: {}", fmt_result(try_many()));
    println!("  3) разберём панику отдельно: {}", fatal());
}

fn fmt_result<T: std::fmt::Debug>(r: Result<T, EngineError>) -> String {
    match r {
        Ok(v) => format!("Ok({v:?})"),
        Err(e) => format!("Err({e})"),
    }
}

// ── Мини-иерархия для примера ───────────────────────────────────────────────

#[derive(Debug)]
struct EngineError {
    kind: ErrorKind,
    context: String,
}

#[derive(Debug)]
enum ErrorKind {
    Io(String),
    Parse(String),
    NotFound(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Контекст печатается ПЕРВЫМ — так сообщение читается как
        // «причина, возникшая при действии X».
        match &self.kind {
            ErrorKind::Io(m) => write!(f, "{}: io: {m}", self.context),
            ErrorKind::Parse(m) => write!(f, "{}: parse: {m}", self.context),
            ErrorKind::NotFound(m) => write!(f, "{}: не найдено: {m}", self.context),
        }
    }
}

impl std::error::Error for EngineError {}

/// Позволяет `?` превращать io::Error в EngineError с контекстом.
/// В реальном проекте это пишет `#[derive(thiserror::Error)]` с
/// `#[error("{context}: {source}")] #[from] source: io::Error`.
impl From<std::io::Error> for EngineError {
    fn from(e: std::io::Error) -> Self {
        EngineError { kind: ErrorKind::Io(e.to_string()), context: "io".into() }
    }
}

// ── Функции, показывающие цепочку `?` ──────────────────────────────────────

fn parse_one(s: &str) -> Result<u32, EngineError> {
    s.parse::<u32>().map_err(|e| EngineError {
        kind: ErrorKind::Parse(e.to_string()),
        context: format!("parse_one({s:?})"),
    })
}

/// Здесь `?` на Result: контекст ставится вручную, потому что
/// контекст зависит от вызова, а не от типа ошибки.
fn parse_and_double(s: &str) -> Result<u32, EngineError> {
    let v = parse_one(s)?;
    Ok(v * 2)
}

/// `std::fs::read_to_string` вернёт io::Error, а `?` сам вызовет
/// `From<io::Error> for EngineError`. Ни одного map_err.
fn read_ok() -> Result<usize, EngineError> {
    // Файла нет — поэтому читаем из единственного источника, который точно есть.
    let cargo = std::env::current_exe().map_err(|e| EngineError {
        kind: ErrorKind::Io(e.to_string()),
        context: "ищем свой бинарник".into(),
    })?;
    Ok(cargo.file_name().map(|n| n.len()).unwrap_or(0))
}

fn try_many() -> Result<(), EngineError> {
    let a = parse_one("1")?;
    let _b = parse_one("oops")?; // упадёт здесь, вернёт Err наружу
    let _c = a;
    Ok(())
}

/// PANIC, потому что помочь нечем. Но сначала попробуем
/// перехватить — так учатся отличать «программа сломалась» от
/// «пользователю показали сообщение».
fn fatal() -> String {
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // глушим вывод в stderr
    let caught = std::panic::catch_unwind(|| load_scene("assets/hero.mesh"));
    std::panic::set_hook(prev);
    match caught {
        Ok(Ok(n)) => format!("ожидалась ошибка, а загрузилось {n}"),
        Ok(Err(e)) => format!("поймали через catch_unwind как Err: {e}"),
        Err(_) => "сработал abort/неперехваченная паника".to_string(),
    }
}

fn load_scene(path: &str) -> Result<usize, EngineError> {
    if !path.contains("assets") {
        return Err(EngineError {
            kind: ErrorKind::Parse("путь не начинается с assets/".into()),
            context: format!("load_scene({path:?})"),
        });
    }
    if !path.contains("hero") {
        return Err(EngineError {
            kind: ErrorKind::NotFound(path.into()),
            context: format!("load_scene({path:?})"),
        });
    }
    Ok(3)
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 7.2 — ТЕОРИЯ: паника как инструмент
// ─────────────────────────────────────────────────────────────────────────────
//
// `panic!` — это НЕ «исключение». Это утверждение «это состояние
// невозможно, и я отказываюсь работать дальше». Причины:
//
//   1. Приглашение (invariants) — нарушен инвариант, который ты
//      доказал при написании кода. Восстановление невозможно.
//   2. Ошибка программиста, а не данных. Пользователь не должен был
//      иметь возможность её вызвать.
//   3. Прототип/заглушка: `unimplemented!("ещё не сделано")`.
//
// ЧЕГО НЕЛЬЗЯ ДЕЛАТЬ В ПАНИКЕ:
//   * `unwrap()` на пользовательских данных («а вдруг файл есть?»).
//     Для данных — Result.
//   * `expect()` в обработке ошибок: `expect` звучит мягче, но это
//     та же паника. `expect` хорош только в тестах и в main().
//   * `panic!` в `Drop` → double panic → abort.
//   * `panic!` в обработчике `&mut self` в середине обхода контейнера
//     → структура осталась в несогласованном состоянии.
//
// ЧТО `unwrap` ДОПУСТИМ:
//   * литералы: `"42".parse().unwrap()` — литерал известен компилятору;
//   * тесты и примеры;
//   * `main()` — «упал, значит не запустился», это понятно пользователю;
//   * массивы внутри тела, где длина доказана логикой выше.
//
// `debug_assert!` — проверка, которая КОМПИЛИРУЕТСЯ ТОЛЬКО В dev.
//   В release исчезает полностью. `assert!` остаётся всегда.
//   Правило: `assert!` — для инвариантов, нарушение которых = UB или
//   «ломаем всё»; `debug_assert!` — для дорогих проверок, которые
//   в release не нужны.
//
// `unreachable!()` — «компилятор не может это знать, но я знаю».
//   ⚠ Если компилятор может доказать, что ветка достижима — паника
//   опасна. `std::hint::unreachable_unchecked()` — это уже unsafe.

fn part_7_2() {
    harness::part("7.2", "ПРИМЕР: assert / debug_assert / unreachable / catch_unwind");

    // --- assert! — всегда в бинарнике ---------------------------------------
    assert!(1 + 1 == 2);
    assert_eq!(2 + 2, 4, "арифметика сломалась");
    assert_ne!(1, 2);

    // --- debug_assert! — только в dev --------------------------------------
    // Смотри Cargo.toml: [profile.dev] opt-level = 1, но debug-assertions
    // включены по умолчанию. В release их нет.
    debug_assert!(true, "это исчезнет в release");
    println!("  debug_assert в dev: активен");

    // --- unreachable! -------------------------------------------------------
    fn which_branch(x: u8) -> &'static str {
        match x {
            0 => "ноль",
            1 => "один",
            other => unreachable!("для {other} ветка не дописана — это ошибка программиста"),
        }
    }
    println!("  which_branch(1) = {}", which_branch(1));

    // --- Ловим панику в тесте, как это делает #[should_panic] --------------
    fn div(a: u32, b: u32) -> u32 {
        assert_ne!(b, 0, "деление на ноль");
        a / b
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let got = std::panic::catch_unwind(|| div(10, 0));
    std::panic::set_hook(prev);
    assert!(got.is_err());
    println!("  catch_unwind поймал: деление на ноль → паника, а не Err");

    // --- Что происходит с Drop при раскрутке --------------------------------
    println!("\n  --- раскрутка стека зовёт Drop (порядок обратный!) ---");
    struct Guard(&'static str);
    impl Drop for Guard {
        fn drop(&mut self) {
            println!("    drop {}", self.0);
        }
    }
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let res = std::panic::catch_unwind(|| {
        let _g1 = Guard("g1");
        let _g2 = Guard("g2");
        let _g3 = Guard("g3");
        panic!("бум");
    });
    std::panic::set_hook(prev);
    assert!(res.is_err());
    println!("  ...а с panic = \"abort\" этих строк не было бы. Внимание:");

    // --- Infallible: «ошибок не бывает» -----------------------------------
    fn guaranteed() -> Result<u32, std::convert::Infallible> {
        Ok(42) // проверить нечего
    }
    println!("\n  Result<_, Infallible> = {:?} — тип, который можно вызвать с ? и он ничего не ломает", guaranteed());

    // --- Когда unwrap законен, а когда нет ---------------------------------
    println!("\n  ✓ unwrap на литерале: {:?}", "42".parse::<u32>().unwrap());
    println!("  ⛔ unwrap на пользовательских данных — это и есть забытый Result");
    let user_input = std::env::var("DEFINITELY_NOT_SET_VARIABLE_XYZ");
    println!("    env::var → {:?} (Result, а не паника)", user_input.err().map(|e| matches!(e, std::env::VarError::NotPresent)));
}

/// Ошибка агрегируется, когда «первая попавшаяся» не информативна.
/// Настоящий пример: `thiserror` так не умеет, для этого есть
/// `anyhow::Error` (dyn-ошибка с контекстом) — здесь пишем своё.
fn collect_results<T>(items: Vec<Result<T, EngineError>>) -> Result<Vec<T>, Vec<EngineError>> {
    let mut ok = Vec::new();
    let mut err = Vec::new();
    for r in items {
        match r {
            Ok(v) => ok.push(v),
            // match ergonomics: e — это EngineError, не &EngineError
            Err(e) => err.push(e),
        }
    }
    if err.is_empty() { Ok(ok) } else { Err(err) }
}

fn part_7_3() {
    harness::part("7.3", "ПРИМЕР: агрегация ошибок");

    let good: Vec<Result<u32, EngineError>> = vec![Ok(1), Ok(2), Ok(3)];
    match collect_results(good) {
        Ok(v) => println!("  все успешны: {v:?}"),
        Err(e) => println!("  ошибки: {}", e.len()),
    }

    let mixed: Vec<Result<u32, EngineError>> = vec![
        Ok(1),
        Err(EngineError { kind: ErrorKind::Io("нет доступа".into()), context: "asset 1".into() }),
        Ok(3),
    ];
    let mixed_total = mixed.len();
    match collect_results(mixed) {
        Ok(v) => println!("  не должно: {v:?}"),
        Err(e) => {
            println!("  частичный успех: {} ok, {} ошибок", mixed_total - e.len(), e.len());
            for (i, err) in e.iter().enumerate() {
                println!("    [{i}] {err}");
            }
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 7.4 — ЗАДАНИЕ 7.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Восстанови цепочку `?`. Сейчас в коде есть ошибка компиляции
/// (посмотри на сигнатуру `parse_nested`) — найди её и почини правильно.
///
/// ```ignore
/// fn parse_nested(s: &str) -> Result<u32, ConfigError> { ... }
/// ```
///
/// Задание: реализуй `parse_nested` так, чтобы `?` работал для обоих
/// шагов: парсинг числа и вычисление.
///
/// `ConfigError` должен быть ИДЕАЛЬНЫМ: `Display` (для человека),
/// `Debug` (через derive), `Error` (чтобы работал `?` на Result),
/// и `From<ParseIntError>` — это и есть конверсия, которая даёт `?`.
///
/// Вопрос для комментария: почему `ConfigError` НЕ `Box<dyn Error>`,
/// хотя можно? (Ответ: теряется информация; `dyn Error` — эрозия типов.)
fn task_7_1() {
    use std::num::ParseIntError;

    #[derive(Debug, PartialEq)]
    enum ConfigError {
        NotANumber { value: String, source: ParseIntError },
        Empty,
    }

    impl std::fmt::Display for ConfigError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            not_yet!("NotANumber -> \"'{value}' не число: {source}\"; Empty -> \"пусто\"");
        }
    }

    impl std::error::Error for ConfigError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            not_yet!("для NotANumber верни Some(self.source), иначе None");
        }
    }

    impl From<ParseIntError> for ConfigError {
        fn from(source: ParseIntError) -> Self {
            not_yet!("ConfigError::NotANumber { value: \"<неизвестно>\".into(), source }");
        }
    }

    // Эта функция СОМНЕНТЕЛЬНА: сначала найди, почему `?` здесь не работает.
    // Правильный ответ: нужно заменить `?` после parse на map_err,
    // потому что parse возвращает ParseIntError, а нам нужен ConfigError
    // С ИМЕНЕМ исходной строки. From тут бессилен — он не знает value.
    fn parse_nested(s: &str) -> Result<u32, ConfigError> {
        if s.is_empty() {
            return Err(ConfigError::Empty);
        }
        let n: u32 = s.parse().map_err(|source| ConfigError::NotANumber {
            value: s.to_string(),
            source,
        })?;
        Ok(n * 2)
    }

    assert_eq!(parse_nested("21"), Ok(42));
    assert_eq!(parse_nested(""), Err(ConfigError::Empty));
    match parse_nested("x") {
        Err(ConfigError::NotANumber { ref value, .. }) => assert_eq!(value, "x"),
        other => panic!("ожидалась NotANumber, получено {other:?}"),
    }

    // source() работает — это и отличает хорошую ошибку от плохой
    let e = ConfigError::Empty;
    assert!(std::error::Error::source(&e).is_none());
    println!("  ConfigError: Display ✓, Error ✓, source() ✓, From ✓");
}

/// ЧАСТЬ 7.5 — ЗАДАНИЕ 7.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Иерархия ошибок движка. Это задание про АРХИТЕКТУРУ ошибок, а не
/// про их печать. Реализуй так, чтобы по ошибке можно было
/// ПРИНЯТЬ РЕШЕНИЕ, а не просто показать текст.
///
/// ```ignore
/// #[derive(Debug)]
/// enum EngineError {
///     Io { op: &'static str, source: std::io::Error },
///     Parse { what: &'static str, source: Box<dyn std::error::Error + Send + Sync> },
///     NotFound(String),
///     Unsupported(String),
/// }
///
/// impl EngineError {
///     fn is_recoverable(&self) -> bool;   // NotFound — да (пропустим ассет),
///                                        // Io/Parse — нет (баг или диск сдох)
///     fn exit_code(&self) -> i32;         // 2 = данные, 3 = баг, 1 = непонятно
/// }
///
/// fn run_stage(name: &str, f: impl FnOnce() -> Result<(), EngineError>) -> Result<(), EngineError>;
/// fn report_all(results: Vec<(&str, Result<(), EngineError>)>) -> i32;
/// ```
///
/// Требования:
///   * `source()` у Io возвращает `Some(&*self.source)`.
///   * `exit_code`: NotFound → 2, Unsupported → 2, остальное → 3.
///   * `run_stage` добавляет контекст: имя стадии должно попасть в
///     сообщение. Реализуй через `Box<dyn Error>`-обёртку `WithContext`.
///   * `report_all` печатает все ошибки и возвращает худший exit code
///     (максимум). Не останавливается на первой.
fn task_7_2() {
    #[derive(Debug)]
    enum EngineError {
        Io { op: &'static str, source: std::io::Error },
        Parse { what: &'static str, source: Box<dyn std::error::Error + Send + Sync> },
        NotFound(String),
        Unsupported(String),
    }

    impl std::fmt::Display for EngineError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            not_yet!("NotFound -> \"не найдено: {0}\"; Unsupported; Io -> op; Parse -> what: source");
        }
    }

    impl std::error::Error for EngineError {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            not_yet!("Io -> Some(&source); Parse -> Some(&*source); иначе None");
        }
    }

    impl EngineError {
        fn is_recoverable(&self) -> bool {
            not_yet!("NotFound | Unsupported => true");
        }
        fn exit_code(&self) -> i32 {
            not_yet!("NotFound/Unsupported => 2, иначе 3");
        }
    }

    fn run_stage(
        name: &str,
        f: impl FnOnce() -> Result<(), EngineError>,
    ) -> Result<(), EngineError> {
        not_yet!("вызови f, при ошибке замени source на контекст со stage name");
    }

    fn report_all(results: Vec<(&str, Result<(), EngineError>)>) -> i32 {
        not_yet!("напечатай все, верни максимум exit_code");
    }

    let ok = run_stage("init", || Ok(()));
    assert!(ok.is_ok());

    let bad = run_stage("load_assets", || Err(EngineError::NotFound("hero.mesh".into())));
    assert!(bad.is_err());
    assert!(bad.as_ref().unwrap_err().is_recoverable());

    let code = report_all(vec![
        ("renderer", Ok(())),
        ("assets", Err(EngineError::Unsupported("d3d12".into()))),
        ("shaders", Err(EngineError::Parse { what: "glsl", source: "syntax error".into() })),
    ]);
    assert_eq!(code, 3, "худший код = 3 (Parse не восстановим)");
    println!("  exit code = {code}");
}

/// ЧАСТЬ 7.6 — ЗАДАНИЕ 7.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Разбор по полочкам: что должно паниковать, а что — возвращать Err.
/// Наполни `classify` и объясни каждую строку комментарием.
///
/// ```ignore
/// enum Classification { Panic, Error }
/// fn classify(case: &str) -> Classification;
/// ```
///
/// Кейсы:
///   "literal_parse"   — "42".parse().unwrap()          → ?
///   "env_var"         — std::env::var("PATH")?          → ?
///   "array_index"     — v[0] где v пустой                → ?
///   "user_input"      — input.parse().unwrap()          → ?
///   "invariant"       — assert!(b != 0) перед a / b      → ?
///   "div_by_zero"     — 1 / 0 целыми                      → ?
///   "unwrap_nested"   — a.b().unwrap().c()               → ?
///   "todo"            — todo!("не дописал")              → ?
///
/// ⚠ ВНИМАНИЕ: `div_by_zero` в Rust НЕ паникует в release!
///   В debug — паника (проверка переполнения/деления включена),
///   в release — тихо: целочисленное деление на 0 = UB? Нет, в Rust
///   деление на ноль ВСЕГДА вызывает панику. А вот ПЕРЕПОЛНЕНИЕ
///   (`i32::MAX + 1`) — в release заворачивается по модулю (wrapping),
///   и это отдельная ловушка, см. модуль 13.
///
/// Реализуй и напиши в комментариях, где Rust защищает, а где полагается
/// на тебя.
fn task_7_3() {
    #[derive(Debug, PartialEq)]
    enum Classification {
        Panic,
        Error,
    }

    fn classify(case: &str) -> Classification {
        // Ответы не выписаны — проверь себя на assert'ах ниже.
        // Подсказка: спрашивай себя «это данные или инвариант?».
        let _ = case;
        not_yet!("классифицируй все 8 кейсов")
    }

    use Classification::{Error, Panic};
    // ожидаемая карта (проверь себя):
    assert_eq!(classify("literal_parse"), Panic);
    assert_eq!(classify("env_var"), Error);
    assert_eq!(classify("array_index"), Panic);
    assert_eq!(classify("user_input"), Error);
    assert_eq!(classify("invariant"), Panic);
    assert_eq!(classify("div_by_zero"), Panic);
    assert_eq!(classify("unwrap_nested"), Panic);
    assert_eq!(classify("todo"), Panic);

    // А теперь покажем разницу debug/release на переполнении.
    let x: u8 = 255;
    let sum_debug = {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let r = std::panic::catch_unwind(|| x + 1);
        std::panic::set_hook(prev);
        r
    };
    assert!(sum_debug.is_err(), "в dev переполнение паникует");
    let sum_release = x.wrapping_add(1);
    assert_eq!(sum_release, 0, "в release переполнение заворачивается в 0");
    println!("  debug: 255u8 + 1 → паника | release: → {sum_release} (тихо и неправильно!)");
    println!("  ⚠ ВСЕГДА используй checked_* или saturating_* на пользовательских числах");
}
