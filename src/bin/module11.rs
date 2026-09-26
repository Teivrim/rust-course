//! ============================================================================
//! МОДУЛЬ 11 — ДЖЕНЕРИКИ: УГЛУБЛЕНИЕ, CONST GENERICS, РАСКЛАДКА ПАМЯТИ
//! ============================================================================
//!
//! ЧАСТЬ 11.1 — ТЕОРИЯ: where, Sized, impl Trait
//! ─────────────────────────────────────────────────────────────────────────────
//! Модуль 6 дал «что такое дженерик». Здесь — вещи, которые нужны,
//! когда API становится нетривиальным.
//!
//! ГДЕ ТРЕБОВАТЬ ТИПА ТРАЙТА
//!
//!   fn f<T: Clone>(x: T)                  // короткая форма, только трейты
//!   fn g<T>(x: T) where T: Clone          // where-форма: для сложных случаев
//!
//! Разница не в синтаксисе. `where` позволяет:
//!   * писать несколько ограничений на ОДИН тип: `where Vec<T>: Clone`;
//!   * ограничивать АССОЦИИРОВАННЫЕ ТИПЫ: `where I::Item: Clone`;
//!   * не повторять `T`.
//!
//! ТИПЫ ПО УМОЛЧАНИЮ: Sized
//!
//! Почти все дженерики неявно требуют `T: Sized`. Это значит, что
//! `T` не может быть `[T]` или `str` (это DST — динамический размер).
//! Отключить: `fn f<T: ?Sized>(x: &T)`. В C++ аналог — template
//! с потенциально неполным типом, о котором ты должен помнить сам.
//! Rust сделал это видимым в сигнатуре.
//!
//! `impl Trait` в АРГУМЕНТЕ — это просто анонимный дженерик.
//!   fn f(x: impl Copy)          ≡  fn f<T: Copy>(x: T)
//! НО! `impl Trait` можно использовать ТОЛЬКО в позиции аргумента.
//!
//! `impl Trait` в ВОЗВРАТЕ — ЭТО ДРУГОЕ. Это «невозможный тип»:
//! компилятор выбирает конкретный тип и ЗАПРЕЩАЕТ тебе его называть.
//!   fn make() -> impl Iterator<Item = u32>
//! В C++14 это называется return type deduction. Разница: там тип
//! всё равно известен, здесь — намеренно скрыт (как `auto` в C++).
//!
//! НУЛЕВОРАЗМЕРНЫЕ ТИПЫ (ZST)
//! `struct Marker;` занимает 0 байт. `Vec<Marker>` занимает 24 байта
//! (только дескриптор) и не аллоцирует память вообще. На этом построены
//! `PhantomData`, `()` и единицы измерения.
//!
//! ⚠ `PhantomData<T>` — «притворяется, что владеет T».
//!   Нужно, чтобы компилятор знал про `Drop` и lifetimes.
//!   `PhantomData<fn() -> T>` — «может вернуть T, но не владеет».
//!   Разные варианты означают разные вещи (владение/заимствование/
//!  Send/Sync). Ошибка здесь даёт неправильные auto-трейты.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

use std::marker::PhantomData;

fn main() {
    harness::module(11, "Дженерики: углубление, const generics, раскладка");

    part_11_1(); // where, impl Trait, ZST
    part_11_2(); // const generics + проверки на этапе компиляции
    part_11_3(); // раскладка памяти, padding, repr, DST

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("11.1  Storage<T> с lending-итератором (GAT)", task_11_1);
    r.task("11.2  Статический пул ArrayVec<T, N> на const generics", task_11_2);
    r.task("11.3  Раскладка GPU-структур: padding, align, repr(C)", task_11_3);
    r.summary(11);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 11.1 — ПРИМЕР
// ─────────────────────────────────────────────────────────────────────────────
fn part_11_1() {
    harness::part("11.1", "ПРИМЕР: where, impl Trait, ZST, PhantomData");

    // --- where вместо длинной строки ---------------------------------------
    let v = vec![1, 2, 3];
    println!("  where-форма: duplicate_n = {:?}", duplicate_n(&v));
    println!("  короткая:   duplicate_s = {:?}", duplicate_s(&v));
    // where сильно нужен, когда ограничение на АССОЦИИРОВАННЫЙ тип:
    println!("  assoc where: {}", sum_items(1..=10));

    // --- impl Trait в аргументе = анонимный дженерик ------------------------
    println!("  impl Trait арг: describe_all = {}", describe_all(&[1u8, 2, 3]));

    // --- impl Trait в возврате: тип СКРЫТ ----------------------------------
    let it = make_counter(3);
    let got: Vec<u32> = it.collect();
    println!("  impl Trait возврат: {got:?}");
    // let x: MyIter = make_counter(3);   // ⛔ тип неизвестен, его нельзя назвать

    // --- ZST ----------------------------------------------------------------
    println!("\n  ZST:");
    println!("    size_of::<Unit>()               = {}", size_of::<Unit>());
    println!("    size_of::<NoFields>()           = {}", size_of::<NoFields>());
    println!("    size_of::<Vec<Unit>>()          = {} (только дескриптор)", size_of::<Vec<Unit>>());
    let mut v: Vec<Unit> = Vec::new();
    v.push(Unit);
    v.push(Unit);
    println!("    Vec<Unit> на {v:?}: capacity={}, аллокаций не было", v.capacity());

    // --- PhantomData --------------------------------------------------------
    // Структура НЕ владеет T, но зависит от его свойств.
    //
    //   PhantomData<T>         → «владеет T»: Send/Sync берутся от T
    //   PhantomData<fn(&T)>    → «заимствует T»: владельцем считается кто-то ещё
    //   PhantomData<*const T>  → «сырой указатель на T»: НЕ Send, НЕ Sync
    //
    // Все три занимают 0 байт, но последний запрещает отправлять
    // структуру в поток — потому что указатель скопировать можно,
    // а владения за ним нет.
    println!("\n  PhantomData:");
    println!("    size_of::<PhantomData<fn() -> String>>() = {} (всегда 0)", size_of::<PhantomData<fn() -> String>>());
    assert_send::<Owns<u32>>();
    assert_send::<Borrows<String>>();
    // assert_send::<RawPtr<u32>>();   // ⛔ *const u32 это НЕ Send — и это правильно
    println!("    Owns<T> и Borrows<T> — Send; RawPtr<T> — НЕ Send (сырой указатель)");

    let introspect: Introspect<u32> = Introspect { _marker: PhantomData };
    println!("    Introspect выводит имя типа параметра: {}", introspect.print());

    // --- ?Sized: ссылки на срезы --------------------------------------------
    println!("\n  ?Sized: len_str({:?}) = {}", "abc", len_str("abc"));
    println!("          len_str(Vec) невозможно: Vec это Sized, но можно &Vec");
}

fn duplicate_n<T>(v: &[T]) -> usize
where
    T: Clone + PartialEq,
{
    let mut n = 0;
    for (i, a) in v.iter().enumerate() {
        for b in &v[i + 1..] {
            if a == b {
                n += 1;
            }
        }
    }
    n
}

fn duplicate_s<T: Clone + PartialEq>(v: &[T]) -> usize {
    let mut n = 0;
    for (i, a) in v.iter().enumerate() {
        for b in &v[i + 1..] {
            if a == b {
                n += 1;
            }
        }
    }
    n
}

/// Ограничение на ассоциированный тип — вот где `where` незаменим.
fn sum_items<I>(iter: I) -> f64
where
    I: IntoIterator,
    I::Item: Into<f64>,
{
    iter.into_iter().map(|x| x.into()).sum()
}

fn describe_all(items: &[u8]) -> String {
    items.iter().map(|x| x.to_string()).collect::<Vec<_>>().join("+")
}

/// Тип возврата скрыт: его нельзя назвать. Именно это и называется
/// «opaque type» / RPIT (return position impl trait).
fn make_counter(n: u32) -> impl Iterator<Item = u32> {
    (0..n).rev()
}

/// `?Sized` снимает требование «размер известен на этапе компиляции».
/// Ссылка `&T` всегда имеет известный размер, даже если T — срез.
fn len_str(s: &str) -> usize {
    s.len()
}

#[derive(Debug)]
struct Unit;

struct NoFields {
    _a: (),
    _b: [u8; 0],
}

/// Матрица 4x4 как обычный массив — для примера ассоциированной константы.
struct Matrix4([f32; 16]);

/// PhantomData в трёх ипостасях — все занимают 0 байт, но означают
/// РАЗНОЕ, и компилятор это учитывает при выводе Send/Sync.
struct Owns<T> {
    _m: PhantomData<T>,
}
struct Borrows<T> {
    _m: PhantomData<fn(&T)>,
}
struct RawPtr<T> {
    _m: PhantomData<*const T>,
}

fn assert_send<T: Send>() {}
fn assert_sync<T: Sync>() {}

/// Притворяется нужным трейтом, ничего не занимая.
/// Значение T нам недоступно, но его `Debug`-ность гарантирована
/// типажом — это и называется «фантомным» поведением.
struct Introspect<T> {
    _marker: PhantomData<fn() -> T>,
}

impl<T> Introspect<T> {
    fn print(&self) -> String {
        format!("Introspect<{}>", std::any::type_name::<T>())
    }
}

use std::mem::{align_of, size_of, size_of_val};

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 11.2 — ТЕОРИЯ: const generics
// ─────────────────────────────────────────────────────────────────────────────
//
// Дженерик по ЗНАЧЕНИЮ, известному на этапе компиляции:
//
//   struct ArrayVec<T, const N: usize> { data: [MaybeUninit<T>; N], len: usize }
//
// Что это даёт по сравнению с `Vec<T>`:
//   * НЕТ аллокации вообще — память внутри самой структуры;
//   * размер известен: `size_of::<ArrayVec<u8, 64>>()` = 64 + 8;
//   * компилятор может развернуть циклы и убрать проверки границ.
//
// Цена: нельзя менять ёмкость. Для горячих данных (вершины, матрицы,
// физические тела в пуле) это ровно то, что нужно.
//
// АССОЦИИРОВАННЫЕ КОНСТАНТЫ (более старый способ):
//
//   struct Pool<const MAX: usize> { ... }          // современный
//   trait Limits { const MAX: usize; }             // ассоцированная
//
// И Historical feature `min_const_generics` (теперь стабильно) позволяет
// считать в константах:
//
//   const BUF: usize = 4 * 8;
//   let mut a: [u8; BUF] = [0; BUF];
//
// Ограничение `generic_const_exprs` (что-то вроде `[T; N + 1]`) до сих
// пор нестабильно, и это осознанно: вычисление произвольных выражений
// на этапе компиляции взрывает время компиляции. Ставь `N` на C++-шаблон
// и при необходимости делай вручную.

fn part_11_2() {
    harness::part("11.2", "ПРИМЕР: const generics и проверки на этапе компиляции");

    // --- Простой массив фиксированного размера ------------------------------
    let fixed: [u8; 8] = [0; 8];
    println!("  [u8; 8]: size={}, len={}", size_of_val(&fixed), fixed.len());

    // --- Vec с заранее известной ёмкостью ----------------------------------
    let mut v: Vec<u32> = Vec::with_capacity(16);
    for i in 0..16u32 {
        v.push(i);
    }
    println!("  Vec::with_capacity(16): capacity={} (ни одной реаллокации)", v.capacity());

    // --- Проверка на этапе компиляции ---------------------------------------
    // `const _: () = assert!(...)` — это static_assert из C++.
    // Вычисляется при компиляции; если условие ложно — ошибка сборки.
    const _: () = assert!(size_of::<u32>() == 4);
    const _: () = assert!(1 + 1 == 2);
    println!("  const-asserts прошли: size_of::<u32>() == 4");

    // ⚠ А вот это — ошибка КОМПИЛЯЦИИ, а не паника:
    // const _: () = assert!(size_of::<u32>() == 8);
    //
    // Разница с `assert!()` в рантайме: эта ошибка не исправляется
    // запуском теста — она роняет сборку. Для инвариантов раскладки
    // (модуль 11.3, задание) это то, что нужно.

    // --- Ассоциированная константа ------------------------------------------
    println!("  Limits для Vec<u8>: MAX={}", <Vec<u8> as Limits>::MAX);
    println!("  Limits для Matrix4: SIZE={}", <Matrix4 as Limits>::SIZE);

    // --- Обобщённая функция с const-параметром ----------------------------
    println!("  zeros::<3> = {:?}", zeros::<3>());
    println!("  zeros::<5> = {:?}", zeros::<5>());

    // --- Что НЕЛЬЗЯ (и почему это не баг) -----------------------------------
    // struct Bad<const N: usize> { data: [u8; N + 1] }  // generic_const_exprs
    println!("\n  ⚠ [T; N + 1] в const-параметре — нестабильно (generic_const_exprs).");
    println!("    Обход: сделай N сам и держи массив на N, а длину считай в рантайме.");
}

/// Ассоциированные константы дают «свойства типа».
trait Limits {
    const MAX: usize;
    const SIZE: usize;
}

impl Limits for Vec<u8> {
    const MAX: usize = 1 << 24; // 16 МБ
    const SIZE: usize = 24; // размер дескриптора Vec на 64 битах
}

impl Limits for Matrix4 {
    const SIZE: usize = 16 * 4;
    // У матрицы нет «потолка» — но константа обязана быть,
    // иначе тип не реализует трейт. Ставим usize::MAX.
    const MAX: usize = usize::MAX;
}

/// Функция, возвращающая массив длины N, без единой аллокации.
fn zeros<const N: usize>() -> [u32; N] {
    [0u32; N]
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 11.3 — ТЕОРИЯ: раскладка памяти
// ─────────────────────────────────────────────────────────────────────────────
//
// `size_of` / `align_of` — `const fn`, значит работают в static-assert'ах.
// `repr(Rust)` (по умолчанию) — компилятор МОЖЕТ переставлять поля в
// угоду выравниванию. Значит раскладку нельзя полагаться на.
//
// `#[repr(C)]` — раскладка как в C: поля в порядке объявления,
// выравнивание структуры = максимум выравниваний полей, между полями
// вставляется padding. ЭТО ОБЯЗАТЕЛЬНО для FFI, uniform-буферов и
// любого взаимодействия с GPU, которое ожидает конкретные смещения.
//
// `#[repr(transparent)]` — структура имеет РАЗМЕР и ВЫРАВНИВАНИЕ одного
// поля. Идеально для newtype поверх `extern` типа и для `NonNull<T>`.
//
// `#[repr(packed)]` — убрать padding. ⚠ Тогда поле может оказаться
// НЕВЫРАВНЕННЫМ, и `&field` даст UB. В Rust 1.x доступ к полю packed
// запрещён (E0793) — приходится делать `read_unaligned`. В движке
// packed почти никогда не нужен, но знать полезно.
//
// ПОЧЕМУ ЕСТЬ PADDING, ДАЖЕ В RUST:
//   struct S { a: u8, b: u32 }
//   a по смещению 0 (1 байт), 3 байта padding, b по смещению 4.
//   size = 8, хотя данных 5. Если положить b первым — b@0..4, a@4..5,
//   size = 8 всё равно, но ХОТЯ БЫ ОДНОГО лишнего чтения не будет.
//   Компилятор Rust ПЕРЕСТАВЛЯЕТ поля сам — и это хорошо для скорости,
//   но катастрофа для FFI. Отсюда правило: любую структуру, которая
//   уходит за границу языка, помечай `repr(C)`.
//
// DST (динамический размер): `[T]`, `str`, и всё, что содержит их
// не-срезе. Размер известен только по ссылке. Отсюда — Vec из трёх
// слов (ptr, len, cap) и «толстый указатель» (ptr, len) для срезов.

fn part_11_3() {
    harness::part("11.3", "ПРИМЕР: измеряем раскладку и ищем padding");

    #[derive(Debug)]
    struct Reorder {
        a: u8,
        b: u32,
        c: u8,
    }

    #[derive(Debug)]
    #[repr(C)]
    struct ReorderC {
        a: u8,
        b: u32,
        c: u8,
    }

    #[derive(Debug)]
    struct VertexFancy {
        position: [f32; 3], // 12
        normal: [f32; 3],   // 12
        uv: [f32; 2],       // 8
        color: [u8; 4],     // 4
    }

    #[derive(Debug)]
    #[repr(C)]
    struct VertexC {
        position: [f32; 3],
        normal: [f32; 3],
        uv: [f32; 2],
        color: [u8; 4],
    }

    println!("  size_of::<Reorder>()  = {} (repr(Rust): компилятор переставил поля)",
        size_of::<Reorder>());
    println!("  size_of::<ReorderC>() = {} (repr(C): порядок объявления)", size_of::<ReorderC>());
    println!("  size_of::<VertexFancy>() = {} (данных 36, но выравнивание требует кратного 4)",
        size_of::<VertexFancy>());
    println!("  size_of::<VertexC>()     = {}", size_of::<VertexC>());
    println!("    ^ разница в 4 байта на вершину: на миллионе вершин это 4 МБ лишних");

    // --- Где именно padding? -------------------------------------------------
    println!("\n  раскладка VertexC (ожидаемые смещения):");
    println!("    position @0 size={} align={}", size_of::<[f32; 3]>(), align_of::<[f32; 3]>());
    println!("    normal   @12");
    println!("    uv       @24");
    println!("    color    @32 size=4 align=1");
    println!("    итого    {} (36 данных + 0 padding, потому что порядок удачный)", size_of::<VertexC>());

    // ⚠ А вот «неудачный» порядок:
    #[derive(Debug)]
    #[repr(C)]
    struct BadOrder {
        a: u8,
        b: u32,
        c: u8,
    }
    println!("\n  repr(C) с порядком (u8, u32, u8):");
    println!("    size = {}, align = {}", size_of::<BadOrder>(), align_of::<BadOrder>());
    println!("    a @0, padding 3 байта, b @4, c @8, хвостовой padding 3 байта");
    println!("    ⚠ НИКОГДА не меняй порядок полей в repr(C) структуре, не проверив сдвиги.");

    // --- Проверка смещений без unsafe ----------------------------------------
    // Реальный приём: читаем через ссылки, приводим к usize.
    // Это БЕЗОПАСНО (никакого cast), потому что мы не создаём ссылку
    // на невыровненную память — берём адрес поля, который уже корректен.
    let v = VertexC { position: [0.0; 3], normal: [0.0; 3], uv: [0.0; 2], color: [0; 4] };
    let base = &v as *const VertexC as usize;
    println!("\n  реальные смещения (base = {base:#x}):");
    println!("    position @{}", (&v.position as *const _ as usize) - base);
    println!("    normal   @{}", (&v.normal as *const _ as usize) - base);
    println!("    uv       @{}", (&v.uv as *const _ as usize) - base);
    println!("    color    @{}", (&v.color as *const _ as usize) - base);

    // --- DST: размер «толстого» указателя -----------------------------------
    println!("\n  толстые указатели:");
    println!("    size_of::<&u8>()       = {} (ptr)", size_of::<&u8>());
    println!("    size_of::<&[u8]>()     = {} (ptr + len) — DST!", size_of::<&[u8]>());
    println!("    size_of::<&str>()      = {} (ptr + len) — DST!", size_of::<&str>());
    println!("    size_of::<Vec<u8>>()   = {} (ptr + len + cap)", size_of::<Vec<u8>>());
    println!("    size_of::<Box<[u8]>>() = {} (ptr + len, cap нет — не нашлось бы)", size_of::<Box<[u8]>>());

    // --- Проверка, что порядок полей свободен -------------------------------
    #[derive(Debug)]
    struct Reordered {
        _pad: u32,
        c: u8,
        _a: u8,
    }
    println!("\n  Rust ПЕРЕСТАВЛЯЕТ поля: size_of::<Reordered>() = {} (та же сумма)",
        size_of::<Reordered>());

    use std::mem::{align_of, size_of_val};
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 11.4 — ЗАДАНИЕ 11.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// GAT (Generic Associated Types) — фича, которой не хватает C++.
/// Позволяет контейнеру «одолжить» наружу итератор, который ЗАИМСТВУЕТ
/// сам контейнер, а не владеет копией.
///
/// Зачем: `fn iter(&self) -> impl Iterator<Item = &T> + '_` работает
/// и без GAT. GAT нужна, когда `Item` зависит от параметров самого
/// дженерика трейта, — например, когда метод возвращает ссылку на
/// САМ контейнер, а не на его элемент:
///
/// ```ignore
/// trait Container {
///     type Item;
///     fn get(&self) -> Option<&Self::Item>;
/// }
///
/// struct Storage<T> { data: Vec<T> }
/// impl<T> Container for Storage<T> {
///     type Item = T;
///     fn get(&self) -> Option<&T> { self.data.first() }
/// }
/// ```
///
/// Реализуй ещё `iter_mut`, где элементы отдаются `&mut T`, и
/// `iter_lending` — итератор, живущий не дольше `&mut self`:
///
/// ```ignore
/// trait Container {
///     type Item;
///     fn get(&self) -> Option<&Self::Item>;
///     fn iter_mut(&mut self) -> IterMut<'_, Self::Item>;      // обычный тип
///     fn pop_front(&mut self) -> Option<Self::Item>;
/// }
///
/// struct IterMut<'a, T> { inner: std::slice::IterMut<'a, T> }
/// ```
///
/// Требования:
///   * `Storage::new`, `push`, `len`, `get`, `iter`, `iter_mut`.
///   * `pop_front` — O(n), возвращает Option.
///   * `IterMut` реализует `Iterator<Item = &mut T>`.
///   * Ответь в комментарии: зачем нужен отдельный тип `IterMut`,
///     если можно `impl Iterator + '_`?
fn task_11_1() {
    trait Container {
        type Item;

        fn get(&self) -> Option<&Self::Item>;
        fn iter_mut(&mut self) -> IterMut<'_, Self::Item>;
        fn pop_front(&mut self) -> Option<Self::Item>;
    }

    struct Storage<T> {
        data: Vec<T>,
    }

    struct IterMut<'a, T> {
        inner: std::slice::IterMut<'a, T>,
    }

    // ⚠ Обрати внимание на lifetime: `impl<'a, T>`, а не `impl<T>`.
    //   `type Item = &'a mut T` ссылается на lifetime ИТЕРАТОРА, а не
    //   на тип. Если написать `type Item = &mut T` без имени, компилятор
    //   скажет: «нужно взять lifetime из самого типа, а нельзя».
    impl<'a, T> Iterator for IterMut<'a, T> {
        type Item = &'a mut T;
        fn next(&mut self) -> Option<Self::Item> {
            not_yet!("self.inner.next()");
        }
    }

    impl<T> Storage<T> {
        fn new() -> Self {
            not_yet!("пустой Vec");
        }
        fn push(&mut self, v: T) {
            self.data.push(v);
        }
        fn len(&self) -> usize {
            self.data.len()
        }
    }

    impl<T> Container for Storage<T> {
        type Item = T;

        fn get(&self) -> Option<&Self::Item> {
            not_yet!("self.data.first()");
        }

        fn iter_mut(&mut self) -> IterMut<'_, Self::Item> {
            not_yet!("IterMut { inner: self.data.iter_mut() }");
        }

        fn pop_front(&mut self) -> Option<Self::Item> {
            not_yet!("if empty -> None, иначе remove(0)");
        }
    }

    let mut s: Storage<u32> = Storage::new();
    s.push(1);
    s.push(2);
    s.push(3);

    assert_eq!(s.get(), Some(&1));
    assert_eq!(s.len(), 3);
    for v in s.iter_mut() {
        *v *= 10;
    }
    assert_eq!(s.pop_front(), Some(10));
    assert_eq!(s.len(), 2);
}

/// ЧАСТЬ 11.5 — ЗАДАНИЕ 11.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Статический вектор без единой аллокации. Это то, что в реальном
/// движке называется `SmallVec`, но здесь без зависимостей.
///
/// ```ignore
/// struct ArrayVec<T, const N: usize> {
///     data: [MaybeUninit<T>; N],
///     len: usize,
/// }
///
/// impl<T, const N: usize> ArrayVec<T, N> {
///     const fn new() -> Self;
///     fn with_value(v: T) -> Self;                  // len = 1
///     fn push(&mut self, v: T) -> Result<(), T>;    // Err(v) если полон
///     fn pop(&mut self) -> Option<T>;
///     fn as_slice(&self) -> &[T];                   // unsafe внутри
///     fn as_mut_slice(&mut self) -> &mut [T];
///     fn len(&self) -> usize;
///     fn is_full(&self) -> bool;
/// }
/// impl<T: Copy, const N: usize> Copy for ArrayVec<T, N> {}
/// ```
///
/// Требования:
///   * `MaybeUninit<T>` — потому что инициализировать все N элементов
///     нельзя без затрат, а «мусор» в неиспользуемых слотах допустим
///     ровно до момента `assume_init` (глава 12.3).
///   * `Drop` НЕ реализуем: при `T` без `Drop` копировать MaybeUninit
///     можно, но при `T` с `Drop` нужно сначала «съесть» len элементов.
///     Добавь `unsafe impl<T, const N: usize> Drop for ArrayVec<T, N>`,
///     который при N>0 делает `ptr::drop_in_place` на срез из len
///     элементов. Разберись, почему `assume_init` всего массива здесь
///     был бы UB.
///   * `as_slice` использует `slice::from_raw_parts` — напиши
///     `// SAFETY:` с тремя пунктами (указатель, длина, выравнивание).
///   * Проверь `size_of::<ArrayVec<u8, 64>>() == 65`.
fn task_11_2() {
    use std::mem::MaybeUninit;

    struct ArrayVec<T, const N: usize> {
        data: [MaybeUninit<T>; N],
        len: usize,
    }

    impl<T, const N: usize> ArrayVec<T, N> {
        // `const { expr }` (инлайн-константа, Rust 1.79+) позволяет
        // создать `[MaybeUninit::uninit(); N]` в константном контексте.
        // Без неё пришлось бы писать небезопасное
        // `MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init()`.
        const fn new() -> Self {
            Self {
                data: [const { MaybeUninit::uninit() }; N],
                len: 0,
            }
        }

        fn with_value(v: T) -> Self {
            not_yet!("new() + положить v в data[0], len = 1");
        }

        fn push(&mut self, v: T) -> Result<(), T> {
            not_yet!("if len == N -> Err(v), иначе data[len] = MaybeUninit::new(v), len += 1");
        }

        fn pop(&mut self) -> Option<T> {
            not_yet!("if len == 0 -> None, иначе assume_init(data[len-1]), len -= 1");
        }

        fn len(&self) -> usize {
            self.len
        }

        fn is_full(&self) -> bool {
            self.len == N
        }

        fn as_slice(&self) -> &[T] {
            not_yet!("unsafe { slice::from_raw_parts(...) } + SAFETY-комментарий");
        }

        fn as_mut_slice(&mut self) -> &mut [T] {
            not_yet!("то же, но from_raw_parts_mut");
        }
    }

    impl<T, const N: usize> Drop for ArrayVec<T, N> {
        fn drop(&mut self) {
            if N == 0 {
                return;
            }
            // SAFETY: первые self.len элементов проинициализированы,
            // остальные — MaybeUninit, и Drop по ним не вызывается.
            // Указатель выровнен (мы получили его из массива MaybeUninit<T>),
            // длина self.len корректна, элементы валидны (мы их сами
            // положили через MaybeUninit::write).
            unsafe {
                std::ptr::drop_in_place(std::slice::from_raw_parts_mut(
                    self.data.as_mut_ptr() as *mut T,
                    self.len,
                ));
            }
        }
    }

    // ⚠ РАСКЛАДКА, КОТОРУЮ СТОИТ ПОНЯТЬ, А НЕ УГАДАТЬ.
    //   Данных 64 байта, но размер 72, а не 65:
    //     data: [MaybeUninit<u8>; 64] — align 1, по смещению 0..64
    //     len:  usize                 — align 8, по смещению 64..72
    //   Если хочешь ровно 64 байта — сделай len: u8 (см. ниже) или
    //   #[repr(packed)], но packed ломает выравнивание. Поэтому 72 —
    //   честная цена usize-овго счётчика. Измерь сам и убедись.
    assert_eq!(size_of::<ArrayVec<u8, 64>>(), 72, "64 данных + 8 байт usize-счётчика");
    assert_eq!(align_of::<ArrayVec<u8, 64>>(), 8);

    // А с u8-счётчиком — ровно 64 байта (выравнивание остаётся 1):
    #[derive(Debug)]
    struct ArrayVecU8Len<const N: usize> {
        data: [std::mem::MaybeUninit<u8>; N],
        len: u8,
    }
    assert_eq!(size_of::<ArrayVecU8Len<64>>(), 64, "u8-счётчик влезает в хвост");
    let _ = ArrayVecU8Len::<64> { data: [std::mem::MaybeUninit::uninit(); 64], len: 0 };

    let mut v: ArrayVec<u32, 3> = ArrayVec::new();
    assert!(v.push(10).is_ok());
    assert!(v.push(20).is_ok());
    assert!(v.push(30).is_ok());
    assert!(v.is_full());
    assert_eq!(v.push(40), Err(40));
    assert_eq!(v.as_slice(), &[10, 20, 30]);
    assert_eq!(v.len(), 3);
    assert_eq!(v.pop(), Some(30));

    for x in v.as_mut_slice() {
        *x *= 2;
    }
    assert_eq!(v.as_slice(), &[20, 40]);

    // with_value
    let one: ArrayVec<&str, 4> = ArrayVec::with_value("one");
    assert_eq!(one.as_slice(), &["one"]);

    // Drop срабатывает корректно (String внутри — проверка, что не UB):
    let mut strings: ArrayVec<String, 4> = ArrayVec::new();
    for i in 0..3 {
        strings.push(format!("s{i}")).expect("есть место");
    }
    assert_eq!(strings.as_slice()[2], "s2");
    drop(strings);
}

/// ЧАСТЬ 11.6 — ЗАДАНИЕ 11.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Раскладка GPU-структур. Это задание, где ошибка стоит «тихих»
// артефактов на экране, а не паники компилятора.
///
/// ```ignore
/// #[repr(C)] struct Uniform { ... }
/// #[repr(C)] struct Vertex { ... }
/// #[repr(C, align(16))] struct Aligned { ... }
///
/// fn offsets<T>(v: &T, f: impl Fn(&T) -> *const u8) -> usize;   // не нужно, делай иначе
/// fn assert_layout<T, const SIZE: usize>();
/// ```
///
/// Сделай:
///   * `Uniform` с полем `mat4: [f32; 16]` и `vec4: [f32; 4]`.
///   * `Vertex` в «плохом» порядке: `color: [u8; 4]`, `pos: [f32; 3]`, `uv: [f32; 2]`
///     и посчитай padding. Затем переставь в порядок, где padding = 0,
///     и сравни размеры.
///   * `Aligned` с `#[repr(C, align(16))]` — покажи, что выравнивание
///     округляется вверх до 16.
///
/// Требования:
///   * Измерь всё через `size_of` / `align_of` и напечатай.
///   * Добавь `const _: () = assert!(size_of::<Vertex>() == 36);`
///     после того, как добьёшься 36 байт (или своего числа).
///   * Ответь в комментарии: почему std140 требует align(16) для vec3/vec4,
///     и что будет, если нарушить (ответ: валидатор Vulkan скажет,
///     но в худшем случае — молчаливое искажение и тормоза).
fn task_11_3() {
    #[repr(C)]
    struct Uniform {
        mat4: [f32; 16],
        vec4: [f32; 4],
    }

    #[repr(C)]
    struct BadVertex {
        color: [u8; 4], // @0, size 4
        pos: [f32; 3],   // @4 — тут padding не нужен, всё выровнено
        uv: [f32; 2],    // @16
        _pad: [u8; 8],   // добиваем до кратного 4 — но это руками, а не компилятором
    }

    #[repr(C)]
    #[derive(Debug, Clone, Copy)]
    struct Vertex {
        pos: [f32; 3],  // @0
        nrm: [f32; 3],  // @12
        uv: [f32; 2],   // @24
        color: [u8; 4], // @32, size 4
    }

    #[repr(C, align(16))]
    struct Aligned {
        a: f32,
    }

    // --- Замер --------------------------------------------------------------
    println!("  Uniform: size={} align={}", size_of::<Uniform>(), align_of::<Uniform>());
    println!("  Vertex:  size={} align={}", size_of::<Vertex>(), align_of::<Vertex>());
    println!("  Aligned: size={} align={} (данных 4, выравнивание 16)", size_of::<Aligned>(), align_of::<Aligned>());

    // --- Реальные смещения полей (без unsafe) -------------------------------
    let v = Vertex { pos: [0.0; 3], nrm: [0.0; 3], uv: [0.0; 2], color: [0; 4] };
    let base = &v as *const Vertex as usize;
    println!("  Vertex смещения: pos@{} nrm@{} uv@{} color@{}",
        (&v.pos as *const _ as usize) - base,
        (&v.nrm as *const _ as usize) - base,
        (&v.uv as *const _ as usize) - base,
        (&v.color as *const _ as usize) - base);

    // ⚠ Сделай три измерения. Затем напиши проверку на этапе компиляции:
    //
    //     const _: () = assert!(size_of::<Vertex>() == 36);
    //
    // ⚠ И ВОПРОС: в `BadVertex` есть ручной `_pad: [u8; 8]`. Убери его
    //    и посмотри, каким станет size_of. Объясни в комментарии:
    //    в C++ компилятор САМ вставляет padding, в Rust — нет?
    //    Ответ: в C++ компилятор вставляет хвостовой padding, чтобы
    //    размер был кратен выравниванию. В Rust тоже, но ТОЛЬКО
    //    при условии, что это не ломает совместимость (repr(C) —
    //    сохраняет совместимость с C, значит padding будет).
    //    Проверь экспериментально и поправь комментарий, если не так.

    let b = BadVertex { color: [0; 4], pos: [0.0; 3], uv: [0.0; 2], _pad: [0; 8] };
    println!("  BadVertex: size={} (данных 32 + ручной pad 8)", size_of_val(&b));

    assert_eq!(size_of::<Vertex>(), 36);
}
