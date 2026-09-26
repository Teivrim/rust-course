//! ============================================================================
//! МОДУЛЬ 04 — СРЕЗЫ, МАССИВЫ, СТРОКИ, БАЙТЫ
//! ============================================================================
//!
//! ЧАСТЬ 4.1 — ТЕОРИЯ
//! ─────────────────────────────────────────────────────────────────────────────
//! Три способа назвать «последовательность T»:
//!
//!   [T; N]   — массив, длина в ТИПЕ, лежит на стеке/в бинарнике.
//!              Rust-аналог `std::array<T, N>`, но встроен в язык.
//!   Vec<T>   — владеющий массив в куче, длина в значении. Может расти.
//!   &[T]    — **срез**: заимствованный кусок где-то в памяти. Это
//!              не отдельный тип данных, а пара (указатель, длина).
//!
//! СРЕЗ = ТОЛСТЫЙ УКАЗАТЕЛЬ. Размер `&[T]` = 2 слова (16 байт на x64):
//! указатель + длина. У `&T` — 1 слово. Отсюда практический вывод:
//! передача среза дешевле копирования массива, но `Vec<T>` дешевле среза
//! (Vec = указатель + len + cap, тоже 3 слова).
//!
//! СРАВНИ С C++:
//!   std::span<T>        ≈ &[T] / &mut [T] — только тем, что встроено в язык
//!   std::vector<T>      ≈ Vec<T>
//!   new T[n] / T arr[n] ≈ [T; N], но без гарантии длины из типа в интерфейсе
//!
//! ПОЧЕМУ ЭТО КРИТИЧНО ДЛЯ ДВИЖКА:
//!   API вида `fn draw(vertices: &[Vertex], texture: &str) -> Result<()>`
//!   не даёт вызывающему ничего сломать: длину подделать нельзя,
//!   освободить раньше времени нельзя, скопировать нельзя (нужно явно
//!   `.to_vec()`). В C++ такой API стоил бы `const Vertex* verts, size_t n`
//!   и был бы полностью непроверяемым.
//!
//! СРАЗЫ `&[T]` vs `&mut [T]` против `&[T; N]`:
//!   &[T]    — длина runtime, границы проверяются в рантайме (паника).
//!   &[T; N] — длина в типе, компилятор может развернуть цикл без проверок.
//!   Для горячего кода (шейдеры, физика) выбор N — это оптимизация
//!   бесплатно, потому что она в типах.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

fn main() {
    harness::module(4, "Срезы, массивы, строки, байты");

    part_4_1(); // массив vs Vec vs срез, методы срезов
    part_4_2(); // строки: UTF-8, chars vs bytes, Cow
    part_4_3(); // байты и порядок байтов

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("4.1  Буфер вершин: поиск, скользящее окно, статистика", task_4_1);
    r.task("4.2  Парсер конфига: key = value, # комментарии", task_4_2);
    r.task("4.3  Чтение бинарного заголовка (LE/BE)", task_4_3);
    r.summary(4);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 4.1 — ПРИМЕР
// ─────────────────────────────────────────────────────────────────────────────
fn part_4_1() {
    harness::part("4.1", "ПРИМЕР: массив, Vec, срез — один и тот же API");

    // Массив: длина 3 в ТИПЕ. 12 байт, лежит на стеке.
    let arr: [f32; 3] = [1.0, 2.0, 3.0];
    println!("  [f32; 3] = {arr:?}, size = {} байт", size_of_val(&arr));

    // Vec: длина в значении, данные в куче.
    let mut vec = arr.to_vec();
    vec.push(4.0);
    println!("  Vec<f32>  = {vec:?}, size = {} байт (дескриптор на стеке)", size_of_val(&vec));

    // Срез: заимствование, копирования нет. Точка обзора длиной N.
    let view: &[f32] = &vec[1..3];
    println!("  &[f32]    = {view:?}, size = {} байт (указатель + длина)", size_of::<&[f32]>());

    // ⛔ НЕЛЬЗЯ изменить Vec через &[f32]:
    // let bad: &mut [f32] = &vec[1..3];
    // bad[0] = 99.0;   // ⛔ E0596: cannot assign through `&`

    // ...но можно через &mut [f32], и это тот же тип с другим доступом:
    {
        let writable: &mut [f32] = &mut vec[1..3];
        writable[0] = 99.0;
    }
    println!("  после &mut[..]: {vec:?}");

    // --- Методы срезов: главная рабочая лошадка ----------------------------
    let data = [1u32, 5, 3, 9, 2, 7, 8];
    println!("\n  данные: {data:?}");

    // Окна: скользящее окно размера k. Возвращает итератор — ленивый.
    let windows: Vec<&[u32]> = data.windows(3).collect();
    println!("  windows(3) = {windows:?}");
    let sum_of_windows: Vec<u32> = data.windows(3).map(|w| w.iter().sum()).collect();
    println!("  сумма каждого окна = {sum_of_windows:?}");

    // Чанки: непересекающиеся куски, последний может быть короче.
    let chunks: Vec<&[u32]> = data.chunks(3).collect();
    println!("  chunks(3) = {chunks:?}");

    // Сортировки
    let mut s = data.to_vec();
    s.sort_unstable(); // быстрый, не сохраняет равные элементы порядок
    println!("  sort_unstable: {s:?}");
    s.sort_by(|a, b| b.cmp(a)); // по убыванию
    println!("  sort_by(desc): {s:?}");

    // Бинарный поиск требует отсортированных данных.
    s.sort();
    println!("  binary_search(&5) = {:?}", s.binary_search(&5));
    println!("  binary_search(&4) = {:?} (ошибка = точка вставки)", s.binary_search(&4));

    // split: разрез по предикату
    let (small, big): (Vec<u32>, Vec<u32>) = s.iter().partition(|&&x| x < 5);
    println!("  partition(<5) = {small:?} / {big:?}");

    // ⛔ Выход за границы — паника, а не UB:
    // let x = vec[999];      // panic: index out of bounds
    // let y = &vec[1..999];  // panic: range end index 999 out of range
    // В C++ то же самое — UB. Здесь паника: программа падает контролируемо.

    // --- &[T; N] — длина в типе, проверок нет ----------------------------
    fn sum3(a: &[f32; 3]) -> f32 {
        a[0] + a[1] + a[2] // компилятор знает длину: цикл развернётся
    }
    println!("  sum3(&[1.0, 2.0, 3.0]) = {}", sum3(&[1.0, 2.0, 3.0]));
}

use std::mem::{size_of, size_of_val};

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 4.2 — ТЕОРИЯ: строки
// ─────────────────────────────────────────────────────────────────────────────
//
// ЧЕТЫРЕ ТИПА, И ЧАСТЫЙ ПРОКАРЫ
//
//   str         — заимствованная строка. НЕ в типе, как `&str`.
//   &str        — «view» на строку: указатель + длина. Байты UTF-8.
//   String      — владеющая строка в куче. Всегда UTF-8.
//   Box<str>    — владеющая, но длина заморожена (нет push).
//
// UTF-8 ГАРАНТИРОВАН. `String` не может содержать невалидный UTF-8 —
// это проверяется в типах, а не в соглашениях. В C++ `std::string`
// хранит байты, и «валидность UTF-8» — твоя забота, что является
// источником огромного класса багов (особенно в Windows-строках).
//
// ГЛАВНЫЙ ПОДВОХ — ИНДЕКСАЦИЯ. `s[0]` у `str` — это **ошибка**,
// потому что «символ №0» не имеет фиксированного размера в байтах.
//
//   let s = "héllo";
//   s.len()            == 6   // БАЙТ (é = 2 байта)
//   s.chars().count()  == 5   // СИМВОЛЫ (code points)
//   s.find('é')        == Some(1)
//
// Сравни с C++: `std::u32string::length()` даёт code points,
// `std::string::size()` — байты. Rust просто разделил эти два понятия
// на два метода с ясными именами.
//
// ЧТО ОСТАЁТСЯ НЕЯСНЫМ: пользователь видит не code points, а графемы
// («семь» с комбинирующим диакритикой — это одна буква для человека,
// но три code point'а). Rust сознательно не решает это: `graphemes`
// требует таблиц Unicode (`unicode-segmentation`).
//
// `Cow<'a, str>` — «копируй, если пришлось»: держит `&str`, пока можно,
// иначе делает `String`. Идеально для парсеров: если ввод уже строка,
// результат ссылается на неё (ноль аллокаций).

fn part_4_2() {
    harness::part("4.2", "ПРИМЕР: UTF-8, bytes vs chars, Cow");

    let s = "héllo";
    println!("  s                 = {s:?}");
    println!("  s.len()           = {}  ← БАЙТЫ", s.len());
    println!("  s.chars().count() = {}  ← CODE POINTS", s.chars().count());
    println!("  s.find('é')       = {:?} (байтовая позиция)", s.find('é'));

    // Байты доступны всегда, символы — итерацией:
    let bytes: Vec<u8> = s.bytes().take(3).collect();
    let chars: Vec<char> = s.chars().take(3).collect();
    println!("  bytes[..3] = {bytes:?}, chars[..3] = {chars:?}");

    // Срез строки — ТОЛЬКО по границам UTF-8. 'é' занимает байты 1..3,
    // поэтому 2 — это середина символа, и срез там запрещён.
    let head = &s[..1]; // 'h' — 1 байт, граница
    let tail = &s[3..]; // 'llo' — 'é' закончен
    println!("  &s[..1] = {head:?}, &s[3..] = {tail:?}");
    // let bad = &s[..2];  // ⛔ panic: byte index 2 is not a char boundary
    //                        (это не UB, а контролируемая паника)

    // ── Форматирование ─────────────────────────────────────────────────────
    let name = "player";
    let hp = 87u32;
    let max_hp = 100u32;
    println!("  format!  → {}", format!("{name}: {hp}/{max_hp} ({}%)", hp * 100 / max_hp));
    println!("  padding → [{:>8}] [{:<8}] [{:^8}]", "R", "L", "C");
    println!("  float   → {:.3} | {:.1e} | {:?}", 1.0 / 3.0, 12345.6789, f32::NAN);
    println!("  debug   → {:?}", [1u8, 2, 3]);
    println!("  alternate → {:#x} / {:#?}", 255, "str");
    // Ширина/точность берутся из аргументов:
    let width = 12;
    println!("  dynamic width → [{:>width$}]", "ok");

    // ── Работа с текстом ───────────────────────────────────────────────────
    let text = "  alpha, beta,\ngamma,  delta  ";
    let parts: Vec<&str> = text.split(',').map(str::trim).collect();
    println!("  split(',')+trim = {parts:?}");
    let words: Vec<&str> = text.split_whitespace().collect();
    println!("  split_whitespace = {words:?}");
    println!("  lines = {:?}", text.lines().collect::<Vec<_>>());
    println!("  uppercase = {:?}", text.to_uppercase());
    println!("  starts_with/ends_with = {}/{}", text.starts_with("  a"), text.trim_end().ends_with('a'));
    println!("  replace = {:?}", text.replacen(' ', "-", 2));

    // ── Cow: копируем только если пришлось ────────────────────────────────
    // Путь 1: ввод уже &str — результат ссылается на него, аллокаций нет.
    let borrowed = maybe_upper("abc");
    println!("  Cow из &str: {:?} (borrowed = {})", borrowed, matches!(borrowed, std::borrow::Cow::Borrowed(_)));

    // Путь 2: ввод String — пришлось копировать.
    let owned = maybe_upper(String::from("abc"));
    println!("  Cow из String: {:?} (borrowed = {})", owned, matches!(owned, std::borrow::Cow::Borrowed(_)));

    // ── Ёмкость: with_capacity — одна аллокация вместо log(n) ─────────────
    let mut sc = String::with_capacity(64);
    for i in 0..5 {
        sc.push_str(&format!("{i},"));
    }
    println!("  with_capacity: {sc:?}, capacity = {}", sc.capacity());
}

/// Возвращает верхний регистр, копируя только если это вообще возможно.
fn maybe_upper<'a>(input: impl Into<Cow<'a, str>>) -> Cow<'a, str> {
    // Разбираем варианты отдельно: у Borrowed и Owned оптимальная стратегия
    // разная, и match это выражает лучше, чем проверка через `if`.
    match input.into() {
        Cow::Borrowed(s) if s.chars().any(char::is_lowercase) => Cow::Owned(s.to_uppercase()),
        Cow::Borrowed(s) => Cow::Borrowed(s), // ← 0 аллокаций
        Cow::Owned(s) if s.chars().any(char::is_lowercase) => Cow::Owned(s.to_uppercase()),
        Cow::Owned(s) => Cow::Owned(s), // ← 0 аллокаций: владение просто перекинули
    }
}

use std::borrow::Cow;

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 4.3 — ТЕОРИЯ: байты и порядок байтов
// ─────────────────────────────────────────────────────────────────────────────
//
// `&[u8]` и `&str` — РАЗНЫЕ ТИПЫ. Их нельзя смешивать без явного
// преобразования, и это не занудство: `&str` обещает валидный UTF-8,
// `&[u8]` — нет. Смешать их — значит заявить «здесь валидный UTF-8».
//
// ЧТО ТАКОЕ ENDINESS. Порядок байтов в машинном слове:
//   little-endian: 0x1234 хранится как [0x34, 0x12]
//   big-endian:    0x1234 хранится как [0x12, 0x34]
// Файлы в заголовках и в Vulkan-структурах бывают любыми. Rust не
// «помнит» порядок — приходится говорить явно через `from_le_bytes`.
// В C++ ты бы делал `reinterpret_cast` (UB на невыровненных полях) или
// `memcpy` — здесь это одна строка с гарантией.
//
// ⚠ ЕЩЁ ОДНА ЛОВУШКА: `f32::from_le_bytes` и `f32::from_bits` —
// разные вещи! Первая читает LE-байты как f32, вторая берёт
// «сырые биты». Путаешь — получаешь тихие NaN. В C++ `*(f32*)&u` и
// `std::bit_cast` — то же самое, но оба варианта одинаково неявны.

fn part_4_3() {
    harness::part("4.3", "ПРИМЕР: чтение чисел из байтов, валидация UTF-8");

    let be = [0x12u8, 0x34, 0x56, 0x78];
    let le = [0x78u8, 0x56, 0x34, 0x12];
    println!("  from_be_bytes {be:?} = 0x{:08x}", u32::from_be_bytes(be));
    println!("  from_le_bytes {le:?} = 0x{:08x}", u32::from_le_bytes(le));

    // 1.0 в IEEE-754 = 0x3F800000
    let float_le = 1.0f32.to_le_bytes();
    println!("  1.0f32.to_le_bytes() = {float_le:?}");
    println!("  f32::from_le_bytes  = {}", f32::from_le_bytes(float_le));
    println!("  f32::from_bits      = {}  ← это «сырые биты», НЕ чтение из буфера!", f32::from_bits(1));

    // Чтение с проверкой границ
    let buf = [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06];
    println!("\n  read u16 @0 = {:?}", read_u16(&buf, 0, Endian::Le));
    println!("  read u16 @5 = {:?} (не хватает байт)", read_u16(&buf, 5, Endian::Le));
    println!("  read u32 @3 = {:?}", read_u32(&buf, 3, Endian::Le));
    println!("  read u32 @4 = {:?} (не хватает)", read_u32(&buf, 4, Endian::Le));

    // str vs &[u8]: конверсия только с проверкой
    let raw: &[u8] = &[0xD0, 0xBF, 0xD1, 0x80, 0xD0, 0xB8]; // "При" в UTF-8
    match std::str::from_utf8(raw) {
        Ok(s) => println!("  from_utf8 OK: {s:?} ({} байт → {} символов)", raw.len(), s.chars().count()),
        Err(e) => println!("  from_utf8 FAIL: {e}"),
    }
    // Валидный UTF-8 на 4 байта, потом мусор: valid_up_to() покажет, где сломалось.
    // Вектор, а не массив-константа, чтобы компилятор не «свернул» вызов
    // в константу и не ругался линтом на заведомо невалидный литерал.
    let raw_bad: Vec<u8> = vec![0xF0, 0x9F, 0x92, 0xA9, 0xFF, 0xFE];
    let bad: &[u8] = &raw_bad;
    println!("  from_utf8(bad) = valid_up_to {:?}", std::str::from_utf8(bad).err().map(|e| e.valid_up_to()));

    // ⛔ str -> &[u8] -> str без проверки: from_utf8_unchecked — это UNSAFE,
    //    потому что компилятор поверит тебе на слово.
    // println!("{:?}", std::str::from_utf8_unchecked(bad));  // ⛔ UB

    // Конверсия Vec<u8> -> String: from_utf8 проверяет.
    let bytes = b"config".to_vec();
    match String::from_utf8(bytes) {
        Ok(s) => println!("  String::from_utf8 OK: {s}"),
        Err(_) => println!("  String::from_utf8 FAIL"),
    }
}

#[derive(Debug, Clone, Copy)]
enum Endian {
    Le,
    Be,
}

fn read_u16(b: &[u8], at: usize, e: Endian) -> Option<u16> {
    let s = b.get(at..at + 2)?;
    let a = [s[0], s[1]];
    Some(match e {
        Endian::Le => u16::from_le_bytes(a),
        Endian::Be => u16::from_be_bytes(a),
    })
}

fn read_u32(b: &[u8], at: usize, e: Endian) -> Option<u32> {
    let s = b.get(at..at + 4)?;
    let a = [s[0], s[1], s[2], s[3]];
    Some(match e {
        Endian::Le => u32::from_le_bytes(a),
        Endian::Be => u32::from_be_bytes(a),
    })
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 4.4 — ЗАДАНИЕ 4.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Буфер вершин: массив `f32`, где вершины идут тройками (x, y, z).
///
/// ```ignore
/// fn normalize_vertices_inplace(v: &mut [f32]);
/// fn max_component(v: &[f32]) -> f32;
/// fn count_faces(v: &[f32]) -> usize;
/// fn axis_bounds(v: &[f32]) -> Option<([f32; 3], [f32; 3])>;
/// fn centroid(v: &[f32]) -> Option<[f32; 3]>;
/// ```
///
/// Требования:
///   * Никаких `Vec` в сигнатурах — только срезы.
///   * `axis_bounds` / `centroid` возвращают `None` на пустом или
///     неполном (не кратном 3) буфере — `checked_exact_chunks(3)`.
///   * Никаких `clone()`.
///   * Никакой арифметики с плавающей точкой в цикле по индексам,
///     если можно итерироваться (`chunks_exact(3)`).
fn task_4_1() {
    fn normalize_vertices_inplace(_v: &mut [f32]) {
        not_yet!("chunks_exact(3) + изменение на месте");
    }

    fn max_component(v: &[f32]) -> f32 {
        not_yet!("iter().copied().fold(f32::MIN, f32::max) — без промежуточных Vec");
    }

    fn count_faces(v: &[f32]) -> usize {
        not_yet!("len / 3");
    }

    fn axis_bounds(v: &[f32]) -> Option<([f32; 3], [f32; 3])> {
        not_yet!("chunks_exact(3), fold по min/max");
    }

    fn centroid(v: &[f32]) -> Option<[f32; 3]> {
        not_yet!("среднее по осям, деление на count");
    }

    let mut v = vec![0.0f32, 3.0, 4.0, -1.0, 0.0, 0.0];
    normalize_vertices_inplace(&mut v);
    assert!((v[0] - 0.0).abs() < 1e-6 && (v[1] - 0.6).abs() < 1e-6);
    assert_eq!(max_component(&v), 0.8);
    assert_eq!(count_faces(&v), 2);
    assert_eq!(axis_bounds(&v), Some(([-1.0, 0.0, 0.0], [0.0, 0.6, 0.8])));
    assert_eq!(axis_bounds(&[1.0, 2.0]), None);
    let c = centroid(&v).expect("непустой");
    assert!((c[0] + 0.5).abs() < 1e-6);
}

/// ЧАСТЬ 4.5 — ЗАДАНИЕ 4.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Парсер конфига. Формат:
///
/// ```text
/// # комментарий
/// renderer = vulkan          # хвостовой комментарий
/// msaa = 4
/// debug
/// ```
///
/// Ключ может быть с пробелами вокруг `=`. Значение — до конца строки,
/// с обрезкой хвостового `#`-комментария (перед обрезкой обязательно
/// trim'ни, чтобы не поймать `//` внутри значения... здесь только `#`).
/// Итог — `Cow<'_, str>`, чтобы для строк без `=` ничего не копировать.
///
/// ```ignore
/// fn parse_config<'a>(src: &'a str) -> Result<Vec<(&'a str, Cow<'a, str>)>, ConfigError>;
/// ```
///
/// Требования:
///   * Строка без `=` — флаг (`debug`).
///   * `#` в начале строки (после trim) — комментарий, пропустить.
///   * Несколько `=` в строке: делить по ПЕРВОМУ (`split_once`).
///   * Значение не должно быть пустым после trim → иначе ошибка.
///   * Определи `ConfigError` сам (минимум: пустое значение).
///
/// Сравни с C++: `std::getline` + ручной `find('=')` + `substr` + `erase`.
/// Здесь `split_once` не может выйти за границы, а ошибка — это значение
/// в типе возврата, а не `assert(false)`.
fn task_4_2() {
    use std::borrow::Cow;

    #[derive(Debug, PartialEq)]
    enum ConfigError {
        EmptyValue { line: usize },
    }

    fn parse_config<'a>(src: &'a str) -> Result<Vec<(&'a str, Cow<'a, str>)>, ConfigError> {
        not_yet!("lines + split_once(':')/split_once('=') + trim + Cow::Borrowed если нечего менять");
    }

    let src = "\
# комментарий
renderer = vulkan   # хвостовой комментарий
msaa=4
debug
";

    let cfg = parse_config(src).expect("валидный конфиг");
    assert_eq!(cfg.len(), 3);
    assert_eq!(cfg[0], ("renderer", Cow::Borrowed("vulkan")));
    assert_eq!(cfg[1], ("msaa", Cow::Borrowed("4")));
    assert_eq!(cfg[2], ("debug", Cow::Borrowed("")));
    assert!(parse_config("k =").is_err());
    println!("  parse_config: {cfg:?}");
}

/// ЧАСТЬ 4.6 — ЗАДАНИЕ 4.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Чтение бинарного заголовка. Соберёшь это для загрузчика форматов.
///
/// ```ignore
/// struct Header { magic: u32, version: u16, count: u16, flags: u32, name: String }
///
/// fn parse_header(b: &[u8], le: bool) -> Option<Header>;
/// ```
///
/// Требования:
///   * Магическое число сверяется: `0x4E4F5645` ("NOVE" в LE) — если
///     не совпало, `None`.
///   * Порядок байтов задаётся флагом `le`, **обязательно** через
///     `from_le_bytes` / `from_be_bytes` (не `transmute`!).
///   * `name` — остаток буфера как UTF-8-строка; при невалидном UTF-8 → `None`.
///   * Никаких `unsafe`, никаких `reinterpret_cast`, никаких `Vec` внутри
///     сверх `String` для имени.
///   * Обязательно проверь, что буфер достаточно длинный ДО чтения полей
///     (одним `get(..)` или ранним `if b.len() < 12`).
fn task_4_3() {
    struct Header {
        magic: u32,
        version: u16,
        count: u16,
        flags: u32,
        name: String,
    }

    fn parse_header(b: &[u8], le: bool) -> Option<Header> {
        not_yet!("проверь длину, magic, потом count/version/flags в нужном порядке, потом from_utf8");
    }

    // Собираем заголовок вручную, как это делает настоящий загрузчик.
    let mut raw: Vec<u8> = Vec::new();
    raw.extend_from_slice(&0x4E4F5645u32.to_le_bytes());
    raw.extend_from_slice(&3u16.to_le_bytes());
    raw.extend_from_slice(&12u16.to_le_bytes());
    raw.extend_from_slice(&0x00FF_00FFu32.to_le_bytes());
    raw.extend_from_slice(b"scene.nov");

    let h = parse_header(&raw, true).expect("валидный заголовок");
    assert_eq!((h.magic, h.version, h.count, h.flags), (0x4E4F5645, 3, 12, 0x00FF00FF));
    assert_eq!(h.name, "scene.nov");

    assert!(parse_header(&[], true).is_none());
    assert!(parse_header(&[0, 1, 2], true).is_none(), "мало байт");
    assert!(parse_header(&[0xFF; 20], true).is_none(), "плохая магия");
    // Тот же буфер, но big-endian — магия не сойдётся:
    assert!(parse_header(&raw, false).is_none());
    println!("  parse_header устойчив к битым входам");
}
