//! ============================================================================
//! МОДУЛЬ 13 — ПАМЯТЬ И ПРОИЗВОДИТЕЛЬНОСТЬ
//! ============================================================================
//!
//! ЧАСТЬ 13.1 — ТЕОРИЯ: zero-cost abstraction
//! ─────────────────────────────────────────────────────────────────────────────
//! «Бесплатная абстракция» = её стоимость в собранном коде та же,
//! что у кода, который ты написал бы руками. НЕ «бесплатная вообще».
//!
//! ЧТО ДЕЛАЕТ ЯЗЫК БЕСПЛАТНЫМ:
//!   * МОНОМОРФИЗАЦИЯ: каждая копия дженерик-функции получает
//!     конкретные типы, после чего типы исчезают, остаются операции.
//!   * ИНЛАЙНИНГ: компилятор подставляет тело функции в место вызова.
//!   * ДОСТАВКА: компилятор (LLVM) переставляет и убирает код.
//!
//! ГДЕ БЕСПЛАТНОСТЬ ЛОМАЕТСЯ (и это нормально):
//!   * `Box<dyn Trait>` — вызов через vtable, инлайн невозможен;
//!   * `Rc`/`Arc` — атомарный инкремент счётчика (на каждый clone);
//!   * `RefCell` — проверка счётчика заимствований в рантайме;
//!   * `Mutex` — атомарная операция и, возможно, блокировка;
//!   * `.collect()` в середине цепочки — ломает слияние в один цикл;
//!   * `format!` — аллокация строки.
//!
//! ПРОВЕРКА ГРАНИЦ (bounds checks). Индексация массива проверяет
//! границы. В release компилятор often УБИРАЕТ проверку, если может
//! доказать, что она не нужна (по вложенным if, по длине итератора).
//! `get_unchecked` — ручной opt-out, только когда инвариант доказан.
//!
//! ПЕРЕПОЛНЕНИЕ — ГЛАВНАЯ ЛОВУШКА DEBUG vs RELEASE:
//!   * dev  (`debug-assertions = true`): `255u8 + 1` → ПАНИКА.
//!   * release: `255u8 + 1` → 0, МОЛЧА. Никакой диагностики.
//! В C++ с целыми то же самое, но там это нормально; в Rust целые
//! по умолчанию проверяются, и люди забывают, что проверка выключена.
//!
//! ПРАВИЛО: пользовательские числа — всегда `checked_add`, `saturating_add`
//! или `wrapping_*` с явным выбором. Никогда не полагайся на дефолт.
//
//! КНОПКИ, КОТОРЫЕ СТОЯТ ЗНАНИЯ (момент обучения, не рецепт):
//!   opt-level      0 = без оптимизаций, 1 = дешёвые, 2 = обычные,
//!                  3 = агрессивные, s/z = ещё агрессивнее.
//!   lto            link-time optimization: видимость между крейтами.
//!                  Медленная сборка, быстрый код.
//!   codegen-units  сколько «параллельных» единиц компиляции.
//!                  Больше = быстрее собирается, хуже оптимизируется.
//!   panic          unwind vs abort. abort быстрее, но Drop не вызовется.
//!   target-cpu     нативные инструкции (SSE4/AVX2) — перестаёт работать
//!                  на более старых процессорах.

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::alloc;
use curriculum::harness::{self, bench, report, show};
use curriculum::not_yet;

fn main() {
    harness::module(13, "Память и производительность");

    part_13_1(); // zero-cost, границы, переполнение
    part_13_2(); // аллокации на живых данных
    part_13_3(); // SoA против AoS

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("13.1  Три реализации суммы: где проходит zero-cost", task_13_1);
    r.task("13.2  SmallVec своими руками: инлайн-буфер без аллокаций", task_13_2);
    r.task("13.3  SoA/AoS: перевести вершины в data-oriented раскладку", task_13_3);
    r.summary(13);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 13.1 — ПРИМЕР
// ─────────────────────────────────────────────────────────────────────────────
fn part_13_1() {
    harness::part("13.1", "ПРИМЕР: сколько стоит «бесплатная» абстракция");

    // ⚠ wrapping_mul, а не умножение: i * 2654435761 переполняет u32
    //   уже при i == 2, и в dev это ПАНИКА. Ровно та ловушка, о которой
    //   говорит часть 13.1, только на реальных данных.
    let data: Vec<u32> = (0..4096u32).map(|i| i.wrapping_mul(2654435761) % 65521).collect();

    // --- Цикл vs итераторы --------------------------------------------------
    let b1 = bench("for + += (baseline)", 3000, |_| {
        let mut s = 0u64;
        for &x in &data {
            s += x as u64;
        }
        s
    });
    // Итератор, который компилятор СВЁРНЁТ в тот же цикл.
    let b2 = bench("iter().sum()", 3000, |_| data.iter().map(|x| *x as u64).sum::<u64>());
    // Цепочка адаптеров — тоже один цикл.
    let b3 = bench("map().filter().sum()", 3000, |_| {
        data.iter().filter(|x| **x % 2 == 0).map(|x| *x as u64).sum::<u64>()
    });
    // ⚠ collect В СЕРЕДИНЕ ломает слияние: материализует Vec.
    let b4 = bench("map().collect::<Vec>().sum()", 3000, |_| {
        data.iter().map(|x| *x as u64).collect::<Vec<u64>>().iter().sum::<u64>()
    });
    show(&b1);
    show(&b2);
    show(&b3);
    show(&b4);
    println!("  → map/filter бесплатны; collect() в середине — платная аллокация");

    // --- Границы массива: убраны ли в release? -----------------------------
    let arr = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let i = 3usize;
    // Компилятор ВИДИТ, что i < arr.len() после проверки if, и убирает
    // вторую проверку в индексации. Это и есть zero-cost.
    let mut sum = 0u8;
    if i < arr.len() {
        sum += arr[i];
    }
    println!("\n  проверка границ: sum = {sum} (двойная проверка убрана компилятором)");

    // ⚠ get_unchecked — ручной opt-out. Никогда не пиши его,
    //   пока не доказал инвариант через assert ПЕРЕД этим местом.
    // let danger = unsafe { *arr.get_unchecked(100) };  // UB

    // --- Переполнение: dev против release -----------------------------------
    println!("\n  --- переполнение ---");
    let x: u8 = 255;
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let dev = std::panic::catch_unwind(|| x + 1);
    std::panic::set_hook(prev);
    match dev {
        Ok(v) => println!("  dev (debug-assertions ON): 255u8 + 1 = {v}  (паники не было)"),
        Err(_) => println!("  dev (debug-assertions ON): 255u8 + 1 → ПАНИКА"),
    }
    let rel = x.wrapping_add(1);
    let chk = x.checked_add(1);
    let sat = x.saturating_add(1);
    println!("  release (тот же код):      255u8 + 1 = {rel}  ← ТИХО, неверно");
    println!("  x.checked_add(1)  = {chk:?}   x.saturating_add(1) = {sat}");
    println!("  ⚠ в release переполнение НЕ диагностируется. Всегда выбирай явно.");

    // --- Диспетчеризация: цена vtable ---------------------------------------
    println!("\n  --- static против dynamic dispatch ---");
    let shapes: Vec<Box<dyn Area>> = (0..1000).map(|i| Box::new(Circle(i as f32)) as Box<dyn Area>).collect();
    let b5 = bench("dyn: 1000 virtual-вызовов", 1000, |_| {
        shapes.iter().map(|s| s.area()).sum::<f32>()
    });
    let circles: Vec<Circle> = (0..1000).map(|i| Circle(i as f32)).collect();
    let b6 = bench("static: 1000 инлайновых вызовов", 1000, |_| {
        circles.iter().map(|c| c.area()).sum::<f32>()
    });
    show(&b5);
    show(&b6);
    println!("  → dyn дороже в {} раз. Для горячего кода — только generic.",
        (b5.min_ns / b6.min_ns.max(1.0)).max(1.0));
}

trait Area {
    fn area(&self) -> f32;
}

struct Circle(f32);

impl Area for Circle {
    fn area(&self) -> f32 {
        std::f32::consts::PI * self.0 * self.0
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 13.2 — ТЕОРИЯ: аллокации
// ─────────────────────────────────────────────────────────────────────────────
//
// `malloc` стоит 20-100 наносекунд. В горячем цикле (60 fps = 16 мс)
// это может быть половина времени. Правило: считай аллокации.
//
// ВСТРОЕННЫЙ СЧЁТЧИК: в Curriculum/src/alloc.rs стоит `#[global_allocator]`,
// который считает ВСЕ выделения в программе. Пользуйся им:
//
//   let (v, allocs, bytes) = alloc::measure(|| vec![0u8; 1024]);
//   println!("{allocs} аллокаций, {bytes} байт");
//
// ⚠ Из-за него нельзя использовать настоящие крейты-аллокаторы
//   (`mimalloc`, `snmalloc`) — будет конфликт: `#[global_allocator]`
//   объявляется один раз на программу. В реальном проекте выбирай одно.
//
// ПРАВИЛА, КОТОРЫЕ УМЕНЯЮТ БОЛЬШЕ ВСЕГО:
//
//   1. `Vec::with_capacity(n)` — если знаешь размер. Без него рост
//      идёт 4 → 8 → 16 → ... → n, то есть log2(n) аллокаций и копий.
//   2. `collect()` с известным размером: `Vec::with_capacity(n)` +
//     extend, либо `iter.collect()` (Rust это уже умеет через size_hint).
//   3. `String::with_capacity` — для строк, которые точно вырастут.
//   4. Не храни `Vec<String>` там, где хватит `Vec<&str>` или
//     `String` со срезами.
//   5. `HashMap::with_capacity` + `entry()` — `entry` не делает
//     двойного поиска, в отличие от contains/insert.
//   6. `retain`/`drain` не пересобирают массив, в отличие от
//     «новый Vec + фильтр».
//
// ХЕШ-КАРТА И DoS: `std::collections::HashMap` по умолчанию использует
// SipHash — он УСТОЙЧИВ К АТАКАМ, но медленнее. Для внутренних
// ключей (id, строки из своих ассетов) можно взять FxHash/ahash и
// получить в разы больше. ⚠ Никогда не открывай наружу API с хеш-таблицей
// по недоверенным ключам на нестойком хеше.
//
// SMALLVEC: 90% задач движка — «маленький список, но в куче не хочу».
// `smallvec::SmallVec<[T; N]>` хранит N элементов ВНУТРИ себя и
// переезжает на кучу только при переполнении. Именно это ты напишешь
// в задании 13.2.

fn part_13_2() {
    harness::part("13.2", "ПРИМЕР: считаем аллокации");

    // --- Vec без with_capacity ---------------------------------------------
    let mut v: Vec<u8> = Vec::new();
    for i in 0..1000usize {
        v.push(i as u8);
    }
    println!("  Vec без with_capacity: len={}, capacity={}", v.len(), v.capacity());
    println!("    → вырос в {} раз: аллокаций было ~{}, а не 1",
        v.capacity(), (v.capacity() as f32).log2().ceil() as u32);

    let v2: Vec<u8> = Vec::with_capacity(1000);
    println!("  Vec::with_capacity(1000): capacity={} сразу", v2.capacity());

    // --- Считаем аллокации на реальных операциях -----------------------------
    let cases: Vec<(&str, Box<dyn Fn() -> ()>)> = vec![
        ("vec![0u8; 1024]", Box::new(|| { let _ = vec![0u8; 1024]; })),
        ("Vec::with_capacity + extend", Box::new(|| { let mut v = Vec::with_capacity(1024); v.extend(std::iter::repeat_n(0u8, 1024)); })),
        ("push 1024 раза", Box::new(|| { let mut v: Vec<u8> = Vec::new(); for i in 0..1024usize { v.push(i as u8); } })),
        ("1024 Vec<u8> по 1 элементу", Box::new(|| { let mut v: Vec<Vec<u8>> = Vec::new(); for i in 0..1024usize { v.push(vec![i as u8]); } })),
        ("1024 String::from(\"ab\")", Box::new(|| { let mut v: Vec<String> = Vec::new(); for _ in 0..1024 { v.push(String::from("ab")); } })),
        ("HashMap 1024 insert", Box::new(|| {
            use std::collections::HashMap;
            let mut m: HashMap<u32, u32> = HashMap::with_capacity(1024);
            for i in 0..1024u32 { m.insert(i, i); }
        })),
        ("HashMap без with_capacity", Box::new(|| {
            use std::collections::HashMap;
            let mut m: HashMap<u32, u32> = HashMap::new();
            for i in 0..1024u32 { m.insert(i, i); }
        })),
    ];

    println!("\n  {:<34} {:>8} {:>10}", "операция", "аллокаций", "байт");
    for (name, f) in cases {
        let f = f;
        let _ = f(); // прогрев
        alloc::reset();
        f();
        let s = alloc::snapshot();
        println!("  {:<34} {:>8} {:>10}", name, s.allocs, s.alloc_bytes);
    }

    // --- Форматирование: неявная аллокация ---------------------------------
    let (s, allocs, bytes) = alloc::measure(|| format!("{} {} {}", 1, 2, 3));
    println!("\n  format!(\"{{}} {{}} {{}}\", 1, 2, 3) = {s:?}: {allocs} аллокаций, {bytes} байт");
    println!("    ⚠ format! в горячем цикле — скрытая аллокация. Пиши в &mut String через write!");

    let mut buf = String::with_capacity(128);
    let (_buf2, a2, _) = alloc::measure(|| {
        use std::fmt::Write;
        for i in 0..10 {
            let _ = write!(buf, "{i} ");
        }
    });
    println!("    write! в &mut String с with_capacity: {a2} аллокаций (0 = идеально)");

    // --- clear vs новый Vec --------------------------------------------------
    let (v3, a3, _) = alloc::measure(|| {
        let mut v: Vec<u64> = Vec::with_capacity(1024);
        for i in 0..1024u64 { v.push(i); }
        v.clear();
        v
    });
    println!("  push 1024 + clear: {a3} аллокаций, осталось capacity={}", v3.capacity());
    println!("    ⚠ clear() сохраняет память (в C++ то же), новый Vec() аллоцирует заново");
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 13.3 — ТЕОРИЯ: SoA против AoS
// ─────────────────────────────────────────────────────────────────────────────
//
// ГЛАВНАЯ ИДЕЯ: процессор достаёт данные из кэша блоками (обычно 64
// байта). Если тебе нужны только позиции вершин, а данные лежат
// вперемешку с нормалями и UV, ты грузишь в кэш ВСЁ, а используешь
// треть. Кэш конечен: на типичном CPU это 8-32 МБ L3.
//
//   AoS (Array of Structures): [Vertex { pos, nrm, uv }, ...]
//     → обход всех полей подряд. Идеален, когда нужны ВСЕ поля.
//   SoA (Structure of Arrays): pos[], nrm[], uv[]
//     → обход одного поля по всем вершинам. Идеален, когда нужен
//       ОДИН компонент (шейдер, физика, frustum culling).
//
// РЕАЛЬНЫЙ ПРИМЕР В ДВИЖКАХ: обновление физики трогает только позиции
// и скорости. В AoS это значит читать 32 байта на вершину, из которых
// нужно 12. В SoA — ровно 12. На большой сцене это разница между
// «в кадре» и «не в кадре».
//
// ⚠ КОМПРОМИСС: если обрабатываешь вершину целиком (загрузка, отсечение),
//    SoA проигрывает. Нет «правильной» раскладки — есть правильная
//    ДЛЯ ДАННОГО ПРОХОДА.
//
// FALSE SHARING — когда два потока пишут в разные переменные, но
// они лежат на одной кэш-линии (64 байта). Каждая запись инвалидирует
// всю линию у другого потока, и вместо роста параллелизма получается
// падение в 10-50 раз. Rust защищает лишь от гонок (через Send/Sync),
// но «ложное разделение» — это не гонка, и компилятор его не видит.
// Лечится: `#[repr(align(64))]` на структуре счётчика.
//
// ЧТО ИЗМЕРИТЬ В СВОЁМ КОДЕ (в этом порядке):
//   1. `alloc::measure` — количество аллокаций. Обычно сразу даёт
//      главный выигрыш.
//   2. `harness::bench` — время, с `black_box`.
//   3. Раскладка данных: `size_of`, `align_of`, реальные смещения.
//   4. Только потом — микро-оптимизации вроде unroll.

fn part_13_3() {
    harness::part("13.3", "ПРИМЕР: SoA против AoS на числах");

    const N: usize = 20_000;

    // AoS: вершина 20 байт (pos 12 + uv 8)
    #[derive(Clone, Copy)]
    struct VertexAoS {
        pos: [f32; 3],
        uv: [f32; 2],
    }

    // SoA: те же данные, но раздельно
    struct VertexSoA {
        pos: Vec<[f32; 3]>,
        uv: Vec<[f32; 2]>,
    }

    let aos: Vec<VertexAoS> = (0..N)
        .map(|i| VertexAoS { pos: [i as f32, 1.0, 2.0], uv: [0.5, 0.5] })
        .collect();
    let soa = VertexSoA {
        pos: (0..N).map(|i| [i as f32, 1.0, 2.0]).collect(),
        uv: (0..N).map(|_| [0.5, 0.5]).collect(),
    };

    println!("  AoS: {} вершин × {} байт = {} байт данных",
        aos.len(), size_of::<VertexAoS>(), aos.len() * size_of::<VertexAoS>());
    println!("  SoA: pos={} байт + uv={} байт = {} байт (те же данные)",
        soa.pos.len() * 12, soa.uv.len() * 8, soa.pos.len() * 12 + soa.uv.len() * 8);
    println!("  ⚠ size_of не показывает разницу — разница в ДОСТУПЕ к кэшу");

    // Проход 1: только позиции (как физика или culling)
    let b1 = bench("AoS: только pos, N=20000", 300, |_| {
        let mut s = 0.0f32;
        for v in &aos {
            s += v.pos[0];
        }
        s
    });
    let b2 = bench("SoA: только pos, N=20000", 300, |_| {
        let mut s = 0.0f32;
        for p in &soa.pos {
            s += p[0];
        }
        s
    });
    show(&b1);
    show(&b2);
    println!("  → SoA в {:.2}x быстрее на «только одном поле»", (b1.min_ns / b2.min_ns.max(1.0)).max(1.0));

    // Проход 2: все поля (как загрузка меша)
    let b3 = bench("AoS: все поля", 300, |_| {
        let mut s = 0.0f32;
        for v in &aos {
            s += v.pos[0] + v.uv[0];
        }
        s
    });
    let b4 = bench("SoA: все поля (два прохода)", 300, |_| {
        let mut s = 0.0f32;
        for p in &soa.pos {
            s += p[0];
        }
        for u in &soa.uv {
            s += u[0];
        }
        s
    });
    show(&b3);
    show(&b4);
    println!("  → когда нужны ВСЕ поля, AoS не проигрывает (SoA даже чуть медленнее)");

    // --- false sharing: иллюстрация ------------------------------------------
    println!("\n  --- false sharing ---");
    println!("    Два AtomicU32 рядом в одной кэш-линии: запись в один инвалидирует второй.");
    println!("    Rust не считает это гонкой (это разные переменные), но кэш — считает.");
    println!("    Лечение: #[repr(align(64))] или отдельные аллокации (padding).");

    // Реальная демонстрация: один общий счётчик vs раздельные (align 64).
    let shared = std::sync::Arc::new(SharedCounter::new());
    let split = std::sync::Arc::new(SplitCounters::new());
    let t = bench("2 потока: общий счётчик (false sharing)", 300, |_| bump_pair(&shared));
    let t2 = bench("2 потока: раздельные (align 64)", 300, |_| bump_pair_split(&split));
    show(&t);
    show(&t2);
    println!("  → раздельные счётчики быстрее в {:.1}x (зависит от CPU и потоков)",
        (t.min_ns / t2.min_ns.max(1.0)).max(1.0));
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[repr(align(64))]
struct Padded(AtomicU64);

struct SharedCounter {
    a: AtomicU64,
    b: AtomicU64,
}

impl SharedCounter {
    fn new() -> Self {
        SharedCounter { a: AtomicU64::new(0), b: AtomicU64::new(0) }
    }
}

/// Раздельные счётчики, каждый на своей кэш-линии.
struct SplitCounters {
    a: Padded,
    b: Padded,
}

impl SplitCounters {
    fn new() -> Self {
        SplitCounters { a: Padded(AtomicU64::new(0)), b: Padded(AtomicU64::new(0)) }
    }
}

fn bump_pair(c: &Arc<SharedCounter>) -> u64 {
    let c1 = Arc::clone(c);
    let c2 = Arc::clone(c);
    let h = std::thread::spawn(move || {
        for _ in 0..100_000 {
            c1.a.fetch_add(1, Ordering::Relaxed);
        }
    });
    for _ in 0..100_000 {
        c2.b.fetch_add(1, Ordering::Relaxed);
    }
    h.join().expect("поток");
    c.a.load(Ordering::Relaxed) + c.b.load(Ordering::Relaxed)
}

fn bump_pair_split(c: &Arc<SplitCounters>) -> u64 {
    let c1 = Arc::clone(c);
    let c2 = Arc::clone(c);
    let h = std::thread::spawn(move || {
        for _ in 0..100_000 {
            c1.a.0.fetch_add(1, Ordering::Relaxed);
        }
    });
    for _ in 0..100_000 {
        c2.b.0.fetch_add(1, Ordering::Relaxed);
    }
    h.join().expect("поток");
    c.a.0.load(Ordering::Relaxed) + c.b.0.load(Ordering::Relaxed)
}

use std::mem::size_of;

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 13.4 — ЗАДАНИЕ 13.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Задача «найти, где ломается zero-cost». Три реализации одной
/// суммы, одна замеряется как «эталон».
///
/// ```ignore
/// fn sum_loop(v: &[f32]) -> f32;
/// fn sum_iter(v: &[f32]) -> f32;          // без collect
/// fn sum_collect(v: &[f32]) -> f32;       // с collect в середине
/// fn sum_dyn(v: &[f64]) -> f64;           // через dyn (нужен trait)
/// ```
///
/// Требования:
///   * Все три результата совпадают.
///   * Замерь каждую через `harness::bench` и напечатай.
///   * Напечатай количество аллокаций каждой через `alloc::measure`.
///   * В комментарии сделай вывод: какая из «красивых» версий
///     на самом деле дороже и ПОЧЕМУ.
///   * Добавь свою четвёртую версию, которая быстрее всех
///     (подсказка: `iter().copied().fold()` или ручной цикл с
///     четырьмя аккумуляторами, чтобы убрать зависимость цепочки),
///     и объясни, почему она быстрее.
fn task_13_1() {
    use curriculum::alloc;

    fn sum_loop(v: &[f32]) -> f32 {
        let mut s = 0.0f32;
        for &x in v {
            s += x;
        }
        s
    }

    fn sum_iter(v: &[f32]) -> f32 {
        not_yet!("iter().copied().sum()");
    }

    fn sum_collect(v: &[f32]) -> f32 {
        not_yet!("iter().map(..).collect::<Vec<f32>>().iter().sum()");
    }

    fn sum_unrolled(v: &[f32]) -> f32 {
        not_yet!("четыре аккумулятора, чтобы разорвать цепочку зависимостей");
    }

    let v: Vec<f32> = (0..8192).map(|i| (i % 97) as f32 * 0.5).collect();
    let e = sum_loop(&v);
    assert_eq!(sum_iter(&v), e);
    assert_eq!(sum_collect(&v), e);
    assert_eq!(sum_unrolled(&v), e);

    let b1 = bench("sum_loop (baseline)", 2000, |_| sum_loop(&v));
    let b2 = bench("sum_iter", 2000, |_| sum_iter(&v));
    let b3 = bench("sum_collect", 2000, |_| sum_collect(&v));
    let b4 = bench("sum_unrolled", 2000, |_| sum_unrolled(&v));
    show(&b1);
    show(&b2);
    show(&b3);
    show(&b4);

    let (_, a1, _) = alloc::measure(|| { let _ = sum_loop(&v); });
    let (_, a2, _) = alloc::measure(|| { let _ = sum_iter(&v); });
    let (_, a3, _) = alloc::measure(|| { let _ = sum_collect(&v); });
    println!("  аллокации: loop={a1} iter={a2} collect={a3}");
    assert_eq!(a3, 1, "collect обязан аллоцировать ровно один раз");
    assert_eq!(a1, 0);
    assert_eq!(a2, 0);
}

/// ЧАСТЬ 13.5 — ЗАДАНИЕ 13.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// `SmallVec` своими руками. Это самая полезная структура в движке.
///
/// ```ignore
/// enum SmallVecRepr<T, const N: usize> {
///     Inline { data: [MaybeUninit<T>; N], len: usize },
///     Heap { data: Vec<T> },
/// }
///
/// struct SmallVec<T, const N: usize> { repr: SmallVecRepr<T, N> }
///
/// impl<T, const N: usize> SmallVec<T, N> {
///     fn new() -> Self;
///     fn with_capacity(n: usize) -> Self;
///     fn push(&mut self, v: T);
///     fn pop(&mut self) -> Option<T>;
///     fn len(&self) -> usize;
///     fn iter(&self) -> impl Iterator<Item = &T>;   // через match
///     fn as_slice(&self) -> &[T];
///     fn is_inline(&self) -> bool;
/// }
/// ```
///
/// Требования:
///   * `N == 0` — тоже работает (сразу Heap).
///   * `Drop` реализуй через `match`: для Heap drop не нужен (Vec сам),
///     для Inline нужно `drop_in_place` на заполненной части.
///     ⚠ Тот же UB-ловушка, что в модуле 11: нельзя `assume_init` весь массив.
///   * `iter()` возвращает `impl Iterator` — значит match по `repr`
///     и возвращаешь `impl Iterator` из двух ветвей. Типы должны
///     совпасть: используй `Box<dyn Iterator>` ИЛИ
///     `std::slice::Iter` через `as_slice().iter()` для обоих случаев.
///   * Проверь: 10 элементов при N=8 → `is_inline() == false`,
///     и `alloc::measure` показывает ровно 1 аллокацию.
fn task_13_2() {
    use std::mem::MaybeUninit;

    enum Repr<T, const N: usize> {
        Inline { data: [MaybeUninit<T>; N], len: usize },
        Heap { data: Vec<T> },
    }

    struct SmallVec<T, const N: usize> {
        repr: Repr<T, N>,
    }

    impl<T, const N: usize> SmallVec<T, N> {
        fn new() -> Self {
            not_yet!("N == 0 -> Heap{vec![]}, иначе Inline{uninit, 0}");
        }

        fn with_capacity(n: usize) -> Self {
            not_yet!("если n <= N -> Inline, иначе Heap с Vec::with_capacity(n)");
        }

        fn len(&self) -> usize {
            match &self.repr {
                Repr::Inline { len, .. } => *len,
                Repr::Heap { data } => data.len(),
            }
        }

        fn is_inline(&self) -> bool {
            matches!(self.repr, Repr::Inline { .. })
        }

        fn push(&mut self, v: T) {
            not_yet!("Inline: если len == N -> вытесни в Vec (перенос len элементов), затем push в Heap; иначе data[len].write(v), len += 1");
        }

        fn pop(&mut self) -> Option<T> {
            not_yet!("Inline: len==0 -> None, иначе assume_init(data[len-1]), len -= 1. Heap: data.pop()");
        }

        fn as_slice(&self) -> &[T] {
            not_yet!("unsafe from_raw_parts в обоих вариантах + SAFETY");
        }

        fn iter(&self) -> std::slice::Iter<'_, T> {
            not_yet!("self.as_slice().iter() — и это честно: один тип для обеих ветвей");
        }
    }

    impl<T, const N: usize> Drop for SmallVec<T, N> {
        fn drop(&mut self) {
            not_yet!("Heap drop не трогай; Inline — drop_in_place по len элементам");
        }
    }

    // N == 0 работает
    let mut z: SmallVec<u8, 0> = SmallVec::new();
    z.push(1);
    z.push(2);
    assert_eq!(z.len(), 2);
    assert!(!z.is_inline());
    assert_eq!(z.iter().copied().collect::<Vec<_>>(), vec![1, 2]);

    // До порога — без аллокаций
    let mut small: SmallVec<u32, 8> = SmallVec::new();
    assert!(small.is_inline());
    for i in 0..8u32 {
        small.push(i);
    }
    assert!(small.is_inline(), "8 элементов влезают в Inline[8]");
    assert_eq!(small.len(), 8);
    assert_eq!(small.iter().copied().sum::<u32>(), 28);

    // Через порог — ровно ОДНА аллокация на переезд
    let (_, allocs, _) = curriculum::alloc::measure(|| {
        let mut s: SmallVec<u32, 8> = SmallVec::new();
        for i in 0..10u32 {
            s.push(i);
        }
        assert!(!s.is_inline());
        assert_eq!(s.len(), 10);
        assert_eq!(s.pop(), Some(9));
    });
    assert_eq!(allocs, 1, "переезд 8 элементов = 1 аллокация");

    // with_capacity
    let big: SmallVec<u32, 4> = SmallVec::with_capacity(100);
    assert!(!big.is_inline());
    assert_eq!(big.len(), 0);

    // Drop с типом, у которого есть Drop (String) — проверка на UB
    let mut strings: SmallVec<String, 2> = SmallVec::new();
    for i in 0..5 {
        strings.push(format!("s{i}"));
    }
    assert_eq!(strings.len(), 5);
    assert_eq!(strings.iter().map(String::as_str).collect::<Vec<_>>(), ["s0", "s1", "s2", "s3", "s4"]);
}

/// ЧАСТЬ 13.6 — ЗАДАНИЕ 13.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Перевести вершины в data-oriented раскладку и убедиться замером.
///
/// ```ignore
/// #[derive(Clone, Copy)]
/// struct Vertex { pos: [f32; 3], nrm: [f32; 3], uv: [f32; 2] }
///
/// struct MeshSoA { pos: Vec<[f32; 3]>, nrm: Vec<[f32; 3]>, uv: Vec<[f32; 2]> }
///
/// fn to_soa(v: &[Vertex]) -> MeshSoA;
/// fn average_y_aos(v: &[Vertex]) -> f32;
/// fn average_y_soa(m: &MeshSoA) -> f32;
/// fn center_of_mass(v: &[Vertex]) -> [f32; 3];
/// fn flip_normals(m: &mut MeshSoA);
/// ```
///
/// Требования:
///   * `average_y_*` — среднее Y. На AoS это читает 20 байт на вершину,
///     на SoA — 4. Замерь разницу через `harness::bench` (N = 50_000).
///   * `flip_normals` меняет все нормали на месте через SoA.
///   * `center_of_mass` — массу считаем по площади: площадь
///     пропорциональна |нормали| (она уже нормализована), поэтому
///     центр масс = средневзвешенное по y. Сделай честно и объясни.
fn task_13_3() {
    #[derive(Clone, Copy, Debug)]
    struct Vertex {
        pos: [f32; 3],
        nrm: [f32; 3],
        uv: [f32; 2],
    }

    struct MeshSoA {
        pos: Vec<[f32; 3]>,
        nrm: Vec<[f32; 3]>,
        uv: Vec<[f32; 2]>,
    }

    fn to_soa(v: &[Vertex]) -> MeshSoA {
        not_yet!("три раза пройти по срезу с collect");
    }

    fn average_y_aos(v: &[Vertex]) -> f32 {
        not_yet!("цикл по Vertex, берёшь v.pos[1]");
    }

    fn average_y_soa(m: &MeshSoA) -> f32 {
        not_yet!("цикл по &[f32;3], берёшь p[1] — трогаешь 4 байта вместо 32");
    }

    fn flip_normals(m: &mut MeshSoA) {
        not_yet!("for n in &mut m.nrm { n[0] = -n[0]; ... }");
    }

    const N: usize = 50_000;
    let verts: Vec<Vertex> = (0..N)
        .map(|i| Vertex {
            pos: [i as f32, (i % 977) as f32, 0.0],
            nrm: [0.0, 1.0, 0.0],
            uv: [0.0, 0.0],
        })
        .collect();
    let soa = to_soa(&verts);

    assert_eq!(soa.pos.len(), N);
    assert!((average_y_aos(&verts) - average_y_soa(&soa)).abs() < 1e-3);

    let b1 = bench("average_y AoS (N=50000)", 200, |_| average_y_aos(&verts));
    let b2 = bench("average_y SoA (N=50000)", 200, |_| average_y_soa(&soa));
    show(&b1);
    show(&b2);
    println!("  SoA быстрее в {:.2}x на «только одно поле»", (b1.min_ns / b2.min_ns.max(1.0)).max(1.0));

    let mut s2 = to_soa(&verts);
    flip_normals(&mut s2);
    assert_eq!(s2.nrm[0], [0.0, -1.0, 0.0]);
    assert_eq!(s2.pos[10], verts[10].pos);
}
