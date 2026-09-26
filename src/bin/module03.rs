//! ============================================================================
//! МОДУЛЬ 03 — ВРЕМЕНА ЖИЗНИ (LIFETIMES)
//! ============================================================================
//!
//! ЧАСТЬ 3.1 — ТЕОРИЯ
//! ─────────────────────────────────────────────────────────────────────────────
//! Lifetime — это **имя** для диапазона времени жизни значения, нужное
//! компилятору, чтобы доказать, что ссылка не переживёт своё содержимое.
//!
//! ГЛАВНОЕ, ЧТО НУЖНО ПОНЯТЬ: lifetime НЕ УПРАВЛЯЕТ ВРЕМЕНЕМ ЖИЗНИ.
//!   Он не выделяет память, не замедляет код (ноль runtime-эффекта),
//!   не проверяется в рантайме. Это **чисто статическая аннотация**
//!   для проверятеля заимствований. В C++ ничего подобного нет вообще:
//!   `T&` не хранит никакой информации о том, на что указывает.
//!
//! `'static` — особый lifetime: значение живёт до конца программы.
//!   * `&'static str` — строковый литерал, лежит в бинарнике. Поэтому
//!     сигнатура `fn name() -> &'static str` — не «я ленюсь», а «я
//!     обещаю, что строка статична».
//!   * `&'static T` в общем случае — отсылка к «висящему» во времени
//!     программы значению, обычно через `Box::leak` или `OnceLock`.
//!
//! `'a` в сигнатуре — НЕ ИМЯ ПЕРЕМЕННОЙ. Это анонимный lifetime,
//!     «любое время жизни, которое выведет компилятор». Читай
//!     `fn longest<'a>(a: &'a str, b: &'a str) -> &'a str` как:
//!     «результат живёт не дольше самой короткой из входных строк».
//!
//! НАЗВАТЬ — значит зафиксировать связь. Одна буква вместо двух
//! параметров — уже экономия, и заодно документация:
//!
//! ```ignore
//! fn parse<'a>(input: &'a str) -> (&'a str, &'a str)  // два куска входа
//! fn split_at_char(ch: char, s: &str) -> (&str, &str)  // эллизия: 1 вход
//! ```
//!
//! ЧАСТЬ 3.2 — ПРИМЕР
//! ─────────────────────────────────────────────────────────────────────────────
//! Структуры, которые ЗАИМСТВУЮТ, а не владеют. Это половина всех API в
//! стандартной библиотеке: `&str`, `&[T]`, `std::collections::BTreeMap<K, V>`,
//! `Vec<T>` (владеющий, но ссылающийся внутри — опустим детали).
//!
//! Ключевая выгода: `ByteView<'a>` над `&'a [u8]` не копирует данные и
//! не может быть «висячим» — компилятор не даст сохранить его дольше
//! буфера. Ровно та же гарантия, что в C++ даёт `std::string_view`,
//! но проверяемая.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

fn main() {
    harness::module(3, "Времена жизни");

    part_3_1(); // структуры-заимствователи, эллизия
    part_3_2(); // variance: почему Short<'short> ⊂ Long<'long>
    part_3_3(); // 'a: 'b, 'static, и почему self-referential структуры невозможны

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("3.1  Три view-типа для подсистемы ресурсов", task_3_1);
    r.task("3.2  Кэш, который физически не может повесить ссылку", task_3_2);
    r.task("3.3  API подсистемы: минимум lifetime, максимум пользы", task_3_3);
    r.summary(3);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 3.1 — ПРИМЕР
// ─────────────────────────────────────────────────────────────────────────────

/// Заимствующий view над байтами. Ни байта не копирует.
#[derive(Debug, Clone, Copy)]
struct ByteView<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ByteView<'a> {
    fn new(data: &'a [u8], offset: usize) -> Option<Self> {
        if offset > data.len() {
            return None;
        }
        Some(Self { data, offset })
    }

    /// ВАЖНО: возвращаемый срез сужает lifetime до `'a`. Компилятор
    /// не даст использовать буфер после того, как view исчезнет, —
    /// именно потому, что `'a` — единственный входной параметр.
    fn rest(&self) -> &'a [u8] {
        &self.data[self.offset..]
    }

    fn read_u32_le(&self) -> Option<u32> {
        let r = self.rest();
        if r.len() < 4 {
            return None;
        }
        Some(u32::from_le_bytes([r[0], r[1], r[2], r[3]]))
    }
}

fn part_3_1() {
    harness::part("3.1", "ПРИМЕР: структуры-заимствователи и эллизия");

    let data: Vec<u8> = (0..16u8).collect();

    let view = ByteView::new(&data, 4).expect("смещение в границах");
    println!("  ByteView {{ offset: {}, rest.len(): {} }}", view.offset, view.rest().len());
    println!("  read_u32_le() = {:?}", view.read_u32_le());
    assert!(ByteView::new(&data, 999).is_none());

    // --- Lifetime виден и в типах контейнеров ------------------------------
    // Vec<String> — владеет. &str внутри — заимствует.
    let names: Vec<&str> = vec!["instance", "device", "pipeline"];
    println!("  Vec<&str> = {names:?} (строки не скопированы)");

    // --- Копию получить нельзя, если буфер живёт на месте: -----------------
    // fn bad() -> &Vec<u8> { let v = vec![0u8; 4]; &v }  // ⛔ E0515
    // Правильно — принять снаружи:
    println!("  структура View не даст удержать ссылку на локальный Vec — это и есть суть lifetime");

    // --- Эллизия в разных местах -------------------------------------------
    let (head, tail) = split_at_byte(&data, 8);
    println!("  split_at_byte → head={head:?} tail.len()={}", tail.len());

    let s = "первая строка\nвторая строка";
    let (line, rest) = split_at_newline(s);
    println!("  split_at_newline → {line:?} / {rest:?}");
}

/// Два входа-ссылки → эллизия НЕ работает, нужно называть.
fn split_at_byte<'a>(d: &'a [u8], at: usize) -> (&'a [u8], &'a [u8]) {
    (&d[..at], &d[at..])
}

/// Один вход-ссылка → эллизия работает, можно не называть.
fn split_at_newline(s: &str) -> (&str, &str) {
    match s.find('\n') {
        Some(i) => (&s[..i], &s[i + 1..]),
        None => (s, ""),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 3.2 — ТЕОРИЯ: variance
// ─────────────────────────────────────────────────────────────────────────────
//
// Что спрашивает компилятор, когда ты пишешь `&'a T`?
//   «Могу ли я **сократить** `'a` до меньшего времени жизни?»
//   Ответ «да» → `&'a T` **ковариантен** (сокращаемый).
//   Ответ «нет» → **контравариантен**.
//
// Зачем это знать: из этого следует правило подстановки
//
// ```ignore
// Short<'short> <: Long<'long>     // где Short<'a> = &'a str, Long<'a> = &'a mut str
// ```
//
// то есть `&'short str` МОЖНО использовать там, где ждут `&'long str`.
// Именно поэтому `fn f(x: &mut T)` можно вызвать с `&mut` на коротком
// времени жизни, и компилятор не спорит.
//
// ПРАВИЛО:
//   `&'a T`             — ковариантен по `'a` (можно укоротить)
//   `&'a mut T`         — **инвариантен** (нельзя ни укоротить, ни удлинить)
//   `fn(&'a T) -> &'a U` — ковариант по входу
//   `fn(&'a T) -> &'a mut U` — контравариантен по входу
//
// ПОЧЕМУ `&mut` инвариантен: `&mut` даёт право **менять** значение и
// продлевать его внутренние ссылки. Если бы `&'long mut T` можно было
// притвориться `&'short mut T`, ты получил бы способ «нарисовать» в
// структуре ссылку на временный объект. В C++ это называется
// dangling reference и находится только на санитайзере.
//
// ОБРАТНАЯ СОВМЕСТИМОСТЬ: `&'short str` подставляется в `&'long str`.
// Компилятор сам сокращает lifetime — тебе не нужно об этом думать.

fn part_3_2() {
    harness::part("3.2", "ПРИМЕР: ковариантность на практике");

    // Функция требует ДОЛГИЙ lifetime — вызываем с коротким: ок.
    let s = String::from("owned");
    let short = s.as_str();
    let long_ref: &str = short; // сокращение lifetime, разрешено
    println!("  &'short str подставлен в &'long str: {long_ref:?}");

    // Структура с двумя lifetime: длинный «владелец» и короткий «заём».
    let owner = String::from("data");
    let mut pair = Pair {
        owner: &owner,
        borrowed: &owner,
    };
    pair.show();
    println!("  owner: {} / borrowed: {}", pair.owner, pair.borrowed);

    // Сокращается только borrowed, owner живёт дольше — валидно.
    let scoped = String::from("short lived");
    pair.borrowed = &scoped;
    pair.show();
    println!("  после смены borrowed на более короткий: {}", pair.borrowed);
    // pair.owner = &scoped;  // ⛔ нельзя: owner объявлен как &'long
}

/// Разные времена жизни в одном типе. `owner` живёт дольше `borrowed`.
#[derive(Debug)]
struct Pair<'owner, 'borrowed> {
    owner: &'owner str,
    borrowed: &'borrowed str,
}

impl<'owner, 'borrowed> Pair<'owner, 'borrowed> {
    fn show(&self) {
        println!("    Pair {{ owner: {:?}, borrowed: {:?} }}", self.owner, self.borrowed);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 3.3 — ТЕОРИЯ: 'a: 'b, 'static, self-referential
// ─────────────────────────────────────────────────────────────────────────────
//
// ОГРАНИЧЕНИЕ `'a: 'b` читается как «'a живёт не меньше, чем 'b».
// Нужен, когда структура хранит ССЫЛКУ ПО ЧУЖОМУ ВРЕМЕНИ ЖИЗНИ.
//
// ```ignore
// struct Cache<'host, 'item> {
//     host: &'host str,        // живёт дольше
//     item: &'item [u8],       // живёт не дольше
// }
// fn get<'item>(c: &Cache<'_, '_>, i: usize) -> &'item [u8]
// ```
//
// ВНУТРЕННЯЯ ЖИЗНЕНЬ vs ВНЕШНЯЯ (`'_`): в 2018 появился `'_`.
// Позволяет не называть lifetime, когда из типа читается однозначно.
// Это 90% случаев в реальном коде.
//
// ПОЧЕМУ НЕЛЬЗЯ self-referential:
//
// ```ignore
// struct Node {
//     name: String,
//     next: Option<&Node>,   // ⛔ E0506 / E0716
// }
// ```
//
// `Node` должен содержать ссылку на САМ СЕБЯ или на своё поле.
// Компилятор этого не позволяет по двум причинам:
//   1. Он не умеет доказать, что адрес поля не изменится (а он может:
//      перемещение структуры меняет адрес). Rust сам фактически
//      **запрещает** перемещать значение, на которое есть внутренняя
//      ссылка, — но проверить это нельзя, поэтому запрет на уровне типов.
//   2. Само по себе «я ссылаюсь на себя» без unsafe невыразимо.
//
// Обходные пути (все три встречаются в движках):
//   * Индексы вместо ссылок: `Vec<Node>` + `usize` — подход ECS.
//   * `Rc<Node>` внутри и `Weak` снаружи — граф в куче.
//   * `unsafe` + `Pin` (модуль 12) — для низкоуровневых структур.
//
// Вывод: в Rust **индексы вместо ссылок** — нормальный способ
// выразить «ссылка на свой же элемент», и это не компромисс,
// а выигрыш в безопасности.

fn part_3_3() {
    harness::part("3.3", "ПРИМЕР: 'a: 'b, '_ и Rc вместо self-reference");

    // --- '_ : lifetime из типа, без имени -----------------------------------
    let total = total_len(&["a", "bb", "ccc"]);
    println!("  total_len через '_: {total}");

    // --- 'a: 'b ------------------------------------------------------------
    let host = String::from("engine.toml");
    let cache = Cache { host: &host, item: b"payload" };
    println!("  Cache: host={:?} item.len()={}", cache.host, cache.item().len());
    println!("  вот как выглядит 'a: 'b:");
    println!("    struct Cache<'host, 'item> {{ host: &'host str, item: &'item [u8] }}");
    println!("    при 'host: &'long str и 'item: &'short [u8] — компилятор проверяет, что _");

    // --- Вместо self-reference: Rc + индексы -------------------------------
    let mut nodes: Vec<Rc<Node>> = Vec::new();
    nodes.push(Rc::new(Node { name: "root".into(), parent: None }));
    nodes.push(Rc::new(Node { name: "child".into(), parent: Some(0) }));
    let root_name = nodes[nodes[1].parent.unwrap()].name.clone();
    println!("  Vec<Rc<Node>> + usize вместо self-reference: root = {root_name}");
    println!("    (ссылка внутрь заменена на ИНДЕКС — безопасный и кеш-френдливы способ)");

    // --- 'static ------------------------------------------------------------
    let n: &str = counter();
    println!("  counter() -> &'static str = {n:?} (строковый литерал в бинарнике)");
    static ANTI_PATTERN: &str = "лежит в бинарнике, никуда не девается";
    println!("  static: {ANTI_PATTERN}");

    let leaked: &'static mut i32 = Box::leak(Box::new(7));
    println!("  Box::leak дал &'static mut i32 = {leaked} (намеренная утечка)");
}

/// Один вход-ссылка → можно и без `'_`, но `'_` показывает, что ты знаешь,
// что там вообще есть lifetime.
fn total_len(_items: &[&str]) -> usize {
    _items.iter().map(|s| s.len()).sum()
}

struct Cache<'host, 'item> {
    host: &'host str,
    item: &'item [u8],
}

impl<'host, 'item> Cache<'host, 'item> {
    fn item(&self) -> &'item [u8] {
        self.item
    }
}

struct Node {
    name: String,
    /// Индекс в Vec, а не ссылка. Self-reference невозможен, индекс — можно.
    parent: Option<usize>,
}

use std::rc::Rc;

fn counter() -> &'static str {
    "счётчик"
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 3.4 — ЗАДАНИЕ 3.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Подсистема ресурсов должна уметь отдавать «взгляд» на данные, ничего
/// не копируя. Реализуй три структуры и общий трейт.
///
/// ```ignore
/// trait AsBytes { fn as_bytes(&self) -> &[u8]; }
///
/// struct Raw<'a>       { bytes: &'a [u8] }
/// struct StrRef<'a>   { text:  &'a str }
/// struct OwnedBytes   { bytes: Vec<u8> }
///
/// impl<'a> Raw<'a>       { fn new(b: &'a [u8]) -> Self; fn u16(&self, i: usize) -> Option<u16>; }
/// impl<'a> StrRef<'a>   { fn len_chars(&self) -> usize; }   // В СИМВОЛАХ, не байтах
/// impl OwnedBytes         { fn from_str(s: &str) -> Self; }
/// ```
///
/// Реализуй `AsBytes` для `Raw`, `StrRef` и `OwnedBytes` так, чтобы
/// внешний код мог работать с любым из них единообразно.
///
/// Требования:
///   * `StrRef::len_chars` обязан считать `self.text.chars().count()`,
///     а НЕ `len()`. Разница — в следующем assert.
///   * Никаких `.to_vec()` внутри view-типов.
fn task_3_1() {
    trait AsBytes {
        fn as_bytes(&self) -> &[u8];
    }

    struct Raw<'a> {
        bytes: &'a [u8],
    }

    struct StrRef<'a> {
        text: &'a str,
    }

    struct OwnedBytes {
        bytes: Vec<u8>,
    }

    impl<'a> Raw<'a> {
        fn new(b: &'a [u8]) -> Self {
            not_yet!("собери Raw из среза b без копирования");
        }
        fn u16(&self, i: usize) -> Option<u16> {
            not_yet!("i и i+1 в границах -> from_le_bytes");
        }
    }

    impl<'a> StrRef<'a> {
        fn len_chars(&self) -> usize {
            not_yet!("chars().count(), НЕ len()");
        }
    }

    impl OwnedBytes {
        fn from_str(s: &str) -> Self {
            not_yet!("as_bytes().to_vec()");
        }
    }

    impl<'a> AsBytes for Raw<'a> {
        fn as_bytes(&self) -> &[u8] {
            not_yet!("self.bytes");
        }
    }
    impl<'a> AsBytes for StrRef<'a> {
        fn as_bytes(&self) -> &[u8] {
            not_yet!("self.text.as_bytes()");
        }
    }
    impl AsBytes for OwnedBytes {
        fn as_bytes(&self) -> &[u8] {
            not_yet!("&self.bytes");
        }
    }

    let buf = [1u8, 0, 2, 0, 9, 9];
    let raw = Raw::new(&buf);
    assert_eq!(raw.u16(0), Some(1));
    assert_eq!(raw.u16(2), Some(2));
    assert_eq!(raw.u16(4), None);

    let s = "привет";
    let sr = StrRef { text: s };
    assert_eq!(sr.len_chars(), 6);
    assert_eq!(sr.as_bytes().len(), 12, "а в байтах — 12");

    let ob = OwnedBytes::from_str("ok");
    assert_eq!(ob.as_bytes(), b"ok");
}

/// ЧАСТЬ 3.5 — ЗАДАНИЕ 3.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Кэш, который отдаёт `&str` на ключ и физически не может оставить
/// висящую ссылку. Компилятор — твой лучший друг, но только если API
/// назван правильно.
///
/// ```ignore
/// struct Cache { map: std::collections::HashMap<String, Vec<u8>> }
/// impl Cache {
///     fn insert(&mut self, key: &str, value: Vec<u8>);
///     fn get<'a>(&'a self, key: &str) -> Option<&'a [u8]>;
///     fn get_str<'a>(&'a self, key: &str) -> Option<&'a str>;
///     fn key_of(&self, index: usize) -> Option<&str>;
/// }
/// ```
///
/// Требования:
///   * `get_str` верен: UTF-8 валиден, `from_utf8(...).ok()`.
///   * `key_of` — без lifetime в сигнатуре: возвращаемое значение
///     эллизируется по self (П3).
///   * Обрати внимание: изменить `map` после получения ссылки нельзя —
///     `&self` (а не `&mut self`) держит заём. Объясни в комментарии,
///     почему `get` не может быть `&mut self`.
///
/// Добавь в конце комментарий: как изменить API, если нужно и читать,
/// и менять (нужен `RefCell`/`RwLock` — модуль 08, или restructure).
fn task_3_2() {
    use std::collections::HashMap;

    struct Cache {
        map: HashMap<String, Vec<u8>>,
        order: Vec<String>,
    }

    impl Cache {
        fn insert(&mut self, key: &str, value: Vec<u8>) {
            not_yet!("map.insert + order.push, без clone ключа");
        }

        fn get<'a>(&'a self, key: &str) -> Option<&'a [u8]> {
            not_yet!("self.map.get(key).map(|v| v.as_slice())");
        }

        fn get_str<'a>(&'a self, key: &str) -> Option<&'a str> {
            not_yet!("get() + from_utf8().ok()");
        }

        fn key_of(&self, index: usize) -> Option<&str> {
            not_yet!("self.order.get(index).map(|s| s.as_str())");
        }
    }

    let mut c = Cache { map: HashMap::new(), order: Vec::new() };
    c.insert("shader", b"glsl".to_vec());
    c.insert("mesh", vec![0, 1, 2]);

    assert_eq!(c.get("mesh"), Some(&[0u8, 1, 2][..]));
    assert_eq!(c.get_str("shader"), Some("glsl"));
    assert_eq!(c.key_of(1), Some("mesh"));
    assert_eq!(c.key_of(5), None);
}

/// ЧАСТЬ 3.6 — ЗАДАНИЕ 3.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Спроектируй API подсистемы так, чтобы минимум именованных lifetime'ов
/// и максимум пользы. Цель — научиться задавать вопрос «а нужен ли тут
/// lifetime вообще?».
///
/// ```ignore
/// struct Asset { name: String, bytes: Vec<u8>, hash: u64 }
///
/// struct Registry { assets: Vec<Asset> }
///
/// impl Registry {
///     fn add(&mut self, a: Asset) -> usize;             // -> id
///     fn get(&self, id: usize) -> Option<&Asset>;       // эллизии достаточно
///     fn load<'a>(&'a mut self, names: &[&'a str]) -> Vec<Option<&'a Asset>>;
///     fn longest_name(&self) -> Option<&str>;
/// }
/// ```
///
/// `load` — самая интересная: она и берёт имена, и возвращает на них же
/// ссылки. Разберись, какой lifetime у результата и почему
/// `&'a mut self` здесь необходим (а `&self` — нет).
///
/// Ответь в комментарии: почему в `get` lifetime не указан, а в `load`
/// приходится, хотя оба возвращают `&Asset`?
fn task_3_3() {
    #[derive(Debug, PartialEq)]
    struct Asset {
        name: String,
        bytes: Vec<u8>,
        hash: u64,
    }

    struct Registry {
        assets: Vec<Asset>,
    }

    impl Registry {
        fn add(&mut self, a: Asset) -> usize {
            not_yet!("push, верни id = index");
        }

        fn get(&self, id: usize) -> Option<&Asset> {
            not_yet!("self.assets.get(id) — lifetime выведется по self");
        }

        fn load<'a>(&'a mut self, names: &[&'a str]) -> Vec<Option<&'a Asset>> {
            not_yet!("для каждого имени найди id в self, верни ссылку");
        }

        fn longest_name(&self) -> Option<&str> {
            not_yet!("max_by_key(len) + map(as_str)");
        }
    }

    let mut r = Registry { assets: Vec::new() };
    let id0 = r.add(Asset { name: "a".into(), bytes: vec![1], hash: 1 });
    let id1 = r.add(Asset { name: "longer".into(), bytes: vec![], hash: 2 });
    assert_eq!((id0, id1), (0, 1));
    assert_eq!(r.get(0).map(|a| a.hash), Some(1));
    assert_eq!(r.get(9), None);
    assert_eq!(r.longest_name(), Some("longer"));
    println!("  Registry спроектирована; вопрос про load answered в комментарии");
}
