//! ============================================================================
//! МОДУЛЬ 08 — УМНЫЕ УКАЗАТЕЛИ И INTERIOR MUTABILITY
//! ============================================================================
//!
//! ЧАСТЬ 8.1 — ТЕОРИЯ
//! ─────────────────────────────────────────────────────────────────────────────
//! Ответ на вопрос «где лежит значение» и «кто им владеет»:
//!
//!   Box<T>      — владение в куче, ОДИН владелец.
//!                  `unique_ptr` в C++, но без `delete`.
//!   Rc<T>       — разделяемое владение в куче, счётчик НЕ атомарен.
//!                  `shared_ptr` без atomics. Только один поток!
//!   Arc<T>      — то же, но потокобезопасно (`shared_ptr` с atomics).
//!   Weak<T>     — «слабая» ссылка, НЕ увеличивает счётчик сильных.
//!                  Единственный способ разорвать цикл.
//!   Cell<T>      — interior mutability без ссылок.
//!   RefCell<T>   — interior mutability С проверкой заимствований в рантайме.
//!
//! ГЛАВНАЯ ПРОБЛЕМА Rc/Arc — ЦИКЛЫ.
//!   Если A держит Rc<B>, а B держит Rc<A>, счётчики никогда не
//!   упадут в ноль → утечка. В C++ `weak_ptr` решает это, но его
//!   легко забыть. В Rust `Rc::new_cyclic` и `Weak` — то же самое,
//!   только компилятор не напомнит (и это осознанный компромисс).
//!
//! СРАВНИ С C++:
//!   std::shared_ptr<T>  → Rc<T> (поток) / Arc<T> (потоки)
//!   std::weak_ptr<T>    → Weak<T>
//!   std::unique_ptr<T>  → Box<T>
//!   глобальная переменная→ OnceLock<T> / static
//!   mutex вокруг поля   → RefCell<T> (поток) / Mutex<T> (потоки)
//!
//! ПРАВИЛО ВЫБОРА (запомни намертво):
//!   Не нужен разделяемый доступ?      → Box<T> (или вообще без Box)
//!   Разделяемое владение, 1 поток?    → Rc<T>
//!   Разделяемое владение, потоки?     → Arc<T>
//!   Нужна ссылка назад (родитель)?    → Weak<T>
//!   Нужно менять через &self?         → Cell / RefCell
//!   Нужно менять через &self, потоки? → Mutex / RwLock

#![allow(unused_variables, unused_imports, dead_code)]

use curriculum::harness::{self, report};
use curriculum::not_yet;

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

fn main() {
    harness::module(8, "Умные указатели и interior mutability");

    part_8_1(); // Box, Rc, Weak, циклы
    part_8_2(); // Cell, RefCell, проверка в рантайме
    part_8_3(); // Send/Sync, Mutex/RwLock, OnceLock, thread_local

    println!("\n{:-^70}", "ЗАДАНИЯ");
    let mut r = report();
    r.task("8.1  Граф сцены на Rc/Weak: обход, поиск пути, разрыв цикла", task_8_1);
    r.task("8.2  ECS-lite на RefCell: безопасные структурные изменения", task_8_2);
    r.task("8.3  Кэш ресурсов: RwLock + OnceLock + thread_local", task_8_3);
    r.summary(8);
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 8.1 — ПРИМЕР: Rc, Weak и счётчики
// ─────────────────────────────────────────────────────────────────────────────
fn part_8_1() {
    harness::part("8.1", "ПРИМЕР: Rc/Weak, счётчики, разрыв цикла");

    // --- Box: перемещение большого объекта дешёвое -------------------------
    // Мы можем перемещать структуру с Vec внутри, не копируя сам Vec:
    let big = Box::new(SceneNode {
        name: String::from("root"),
        children: RefCell::new(vec![1, 2, 3]), // RefCell<Vec> — дешёвый дескриптор
        pos: (0.0, 0.0, 0.0),
    });
    let moved = big; // move, НЕ копирование! Vec внутри не переаллоцирован
    println!("  Box: {:?} (move не скопировал Vec на 3 элемента)", moved.name);

    // --- Rc: разделяемое владение -------------------------------------------
    let a = Rc::new(SceneNode { name: String::from("shared"), children: RefCell::new(Vec::new()), pos: (0.0, 0.0, 0.0) });
    let b = Rc::clone(&a); // НЕ копирование узла! Только счётчик +1
    println!("  Rc: strong_count = {}", Rc::strong_count(&a));

    let c = Rc::new(SceneNode { name: String::from("parent"), children: RefCell::new(Vec::new()), pos: (0.0, 0.0, 0.0) });
    let child = Rc::new(SceneNode { name: String::from("child"), children: RefCell::new(Vec::new()), pos: (1.0, 0.0, 0.0) });

    // --- ⛔ Rc НЕ даёт изменять содержимое ---------------------------------
    // c_mut.children.push(1);   // E0596: cannot borrow data in an `Rc` as mutable
    //
    // ПОЧЕМУ ИМЕННО ТАК (это защита, а не недоработка): если бы несколько
    // владельцев `Rc<Node>` могли менять `node.children` одновременно,
    // никто бы не мог доказать, что гонки нет. «Разделяемое владение» и
    // «эксклюзивное изменение» — взаимоисключающие вещи. Именно поэтому у
    // `Rc` есть `Deref`, но НЕТ `DerefMut`. У `Box` и `Cell` — оба.
    //
    // ТРИ ВЫХОДА, в порядке предпочтения:
    //   1. Не делиться. Если нужен ровно один владелец — `Box<Node>`:
    //      `DerefMut` есть, лишних счётчиков нет.
    //   2. `RefCell` внутри. Если всё же `Rc` — поле обязано быть
    //      `RefCell<Vec<_>>`, и тогда `Rc<RefCell<..>>` становится
    //      `Clone`, даже если внутри не `Clone`.
    //   3. Мутабельный API: `fn add_child(&mut self, ...)`, а разделять
    //      только то, что действительно общее.
    let c_fixed = Rc::new(SceneNode {
        name: String::from("parent"),
        children: RefCell::new(Vec::new()),
        pos: (0.0, 0.0, 0.0),
    });
    c_fixed.children.borrow_mut().push(7);
    println!("  ⛔ Rc<Node>.children.push → E0596 (демо выше, снят комментом)");
    println!("  ✓ решение 2: RefCell<Vec<_>> → {:?}", c_fixed.children.borrow());
    let _ = Rc::clone(&c);

    let child = Rc::new(SceneNode { name: String::from("child"), children: RefCell::new(Vec::new()), pos: (1.0, 0.0, 0.0) });
    let child_ref: Rc<SceneNode> = Rc::clone(&child);
    println!("  перед: parent strong={} weak={}", Rc::strong_count(&c), Rc::weak_count(&c));

    // --- Создаём цикл осознанно и показываем утечку -------------------------
    // `Leaky` держит СИЛЬНУЮ ссылку на соседа — гарантированный цикл.
    {
        let l1 = Rc::new(Leaky { name: "l1", peer: RefCell::new(None) });
        let l2 = Rc::new(Leaky { name: "l2", peer: RefCell::new(None) });
        let witness: Weak<Leaky> = Rc::downgrade(&l1); // «свидетель» для проверки
        *l1.peer.borrow_mut() = Some(Rc::clone(&l2));
        *l2.peer.borrow_mut() = Some(Rc::clone(&l1));
        println!("  цикл создан: strong(l1)={} strong(l2)={}", Rc::strong_count(&l1), Rc::strong_count(&l2));
        drop(l1);
        drop(l2);
        // Локальных Rc больше нет, но узел ЖИВ: его держат друг на друга.
        assert!(witness.upgrade().is_some(), "узел утек — strong_count не упал в 0");
        println!("  ⚠ после drop(l1), drop(l2) узел всё ещё жив: witness.upgrade() = Some");
        // Чиним утечку: рвём цикл вручную. В C++ на это есть weak_ptr,
        // здесь — Weak с самого начала.
        if let Some(n) = witness.upgrade() {
            *n.peer.borrow_mut() = None;
        }
        assert!(witness.upgrade().is_none(), "цикл разорван — узел освобождён");
        println!("  после разрыва цикла: witness.upgrade() = None");
    }

    // --- Правильный вариант: Weak ------------------------------------------
    let h1 = Rc::new(Cyclic { name: "h1", peer: RefCell::new(None) });
    let h2 = Rc::new(Cyclic { name: "h2", peer: RefCell::new(None) });
    *h1.peer.borrow_mut() = Some(Rc::downgrade(&h2)); // Weak, не Rc!
    *h2.peer.borrow_mut() = Some(Rc::downgrade(&h1));
    println!("  с Weak: strong(h1)={} weak(h1)={} — цикл разорван, узлы умрут",
        Rc::strong_count(&h1), Rc::weak_count(&h1));

    // Weak нельзя разыменовать напрямую: сначала upgrade.
    let upgraded = h1.peer.borrow().as_ref().and_then(|w| w.upgrade());
    println!("  weak.upgrade() = {:?}", upgraded.map(|n| n.name));
    assert!(child_ref.children.borrow().is_empty());
    assert_eq!(a.name, b.name);
}

/// Узел, который ссылается на другого СИЛЬНОЙ ссылкой — источник циклов.
struct Leaky {
    name: &'static str,
    peer: RefCell<Option<Rc<Leaky>>>,
}

struct SceneNode {
    name: String,
    // ⚠ В реальном графе сцены поле почти всегда `RefCell<Vec<Rc<Node>>>`:
    //   `Rc` без `RefCell` менять нельзя (см. E0596 выше).
    children: RefCell<Vec<usize>>,
    pos: (f32, f32, f32),
}

/// Узел, который ссылается на другого — источник циклов.
struct Cyclic {
    name: &'static str,
    peer: RefCell<Option<Weak<Cyclic>>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 8.2 — ТЕОРИЯ: interior mutability
// ─────────────────────────────────────────────────────────────────────────────
//
// Проблема: `&T` не даёт менять значение, а менять нужно (например,
// счётчик кадров внутри объекта, который раздают как `&`).
// В C++ это решается `mutable` и `const_cast`. В Rust — отдельным
// набором типов, и это ПРИНУДИТЕЛЬНО: нельзя случайно получить
// изменение через `&`.
//
// `Cell<T>` — без ссылок вообще. `get()`/`set()`. Для Copy-типов.
//   Нет заимствований → нет проверок → нет паник. Работает с `&self`.
//
// `RefCell<T>` — с проверками в РАНТАЙМЕ. Внутри — `UnsafeCell`.
//   Выдаёт `Ref<T>` / `RefMut<T>`, которые живут, пока жив заём.
//   ⚠ Если взять `borrow_mut()` и не отпустить, следующий `borrow()`
//   ПАНИКУЕТ. Это цена: проверка переносится из компиляции в рантайм.
//
// ПРАВИЛО: `borrow`/`borrow_mut` должны жить НЕ ДОЛЬШЕ одного выражения.
//   Не держи `Ref` в структуре — держи `RefCell`.
//
// `RefCell<Vec<T>>` vs `Vec<RefCell<T>>`:
//   Первое — «список элементов, менять можно только целиком».
//   Второе — «список элементов, каждый менять независимо», но
//   аллокация на каждый элемент и плохая локальность доступа.
//   Для ECS почти всегда первое (SoA-подобная логика).
//
// `Ref<T>` и `RefMut<T>` — RAII-обёртки, которые `Drop`-ают заём.
//   Забыл обработать `RefMut` → заём освободится автоматически.

fn part_8_2() {
    harness::part("8.2", "ПРИМЕР: Cell, RefCell, Ref/RefMut, panics");

    // --- Cell: без проверок -------------------------------------------------
    let counter = Cell::new(0u32);
    let bump = |_: ()| counter.set(counter.get() + 1);
    bump(());
    bump(());
    println!("  Cell: {} (меняется через &self, без RefCell)", counter.get());

    // ⚠ Cell НЕ даёт ссылку. `&counter.get()` — ошибка компиляции.
    //    Для замены значения — `set`, для составных операций — `update`:
    counter.set(counter.get() * 10);
    println!("  Cell::set → {}", counter.get());

    // --- RefCell: проверки в рантайме ---------------------------------------
    let data = RefCell::new(vec![1, 2, 3]);

    // Одновременный immutable-доступ — можно
    let r1 = data.borrow();
    let r2 = data.borrow();
    println!("  RefCell: два borrow() одновременно = {} и {} (ок)", r1.len(), r2.len());
    drop(r1);
    drop(r2); // ⚠ без drop следующая строка запаникует

    // Изменение — только когда других заёмов нет
    {
        let mut m = data.borrow_mut();
        m.push(4);
    }
    println!("  RefCell: после borrow_mut {data:?}");

    // --- Демонстрация паники -----------------------------------------------
    // let m = data.borrow_mut();
    // data.borrow();        // ⛔ паника: already mutably borrowed
    // drop(m);
    // try_borrow вместо borrow — БЕЗ паники:
    assert!(data.try_borrow().is_ok());
    {
        let m = data.borrow_mut();
        let again = data.try_borrow();
        println!("  try_borrow при занятом &mut = {:?} (без паники)", again.is_err());
        assert!(again.is_err());
        drop(m);
    }
    println!("  после drop заём освобождён: try_borrow = {:?}", data.try_borrow().is_ok());

    // --- RefCell<Vec<T>> vs Vec<RefCell<T>> ---------------------------------
    let a: RefCell<Vec<u32>> = RefCell::new(vec![1, 2, 3]);
    let b: Vec<RefCell<u32>> = vec![RefCell::new(1), RefCell::new(2)];
    println!("\n  RefCell<Vec<T>>   : size = {}, элементы лежат подряд", size_of_val(&a));
    println!("  Vec<RefCell<T>>   : size = {} на элемент, аллокация на каждый", size_of::<RefCell<u32>>());

    // --- UnsafeCell: то, внутри чего всё это работает ------------------------
    println!("  UnsafeCell<u32>   : size = {} — компилятор НИЧЕГО не проверяет", size_of::<std::cell::UnsafeCell<u32>>());
    println!("  → RefCell безопасен ровно настолько, насколько ты не злоупотребил Ref/RefMut");

    // --- Обход, меняющий структуру: mem::take — единственный чистый путь ---
    let coll = RefCell::new(vec![1, 2, 3]);
    let mut snapshot = std::mem::take(&mut *coll.borrow_mut());
    snapshot.push(4);
    *coll.borrow_mut() = snapshot;
    println!("  mem::take для структурного изменения: {coll:?}");
}

use std::mem::{size_of, size_of_val};

// ─────────────────────────────────────────────────────────────────────────────
// ЧАСТЬ 8.3 — ТЕОРИЯ: потоки, Mutex, OnceLock
// ─────────────────────────────────────────────────────────────────────────────
//
// `Mutex` в Rust — это НЕ «блокировка» из C++. Это «Box<dyn Any +
// флаг занятости». Никаких сырых данных, никаких гонок: залогин
// система идёт через RwLock<Vec<u8>>. Отсюда и `PoisonError`:
// если поток упал, удерживая блокировку, std помечает mutex как
// «отравленный» и следующий `lock()` возвращает Err. В C++ ничего
// подобного нет: забытый unlock после исключения = deadlock, и
// найти его можно только дебаггером.
//
// `RwLock` — много читателей ИЛИ один писатель. Выигрыш, только
// если чтений НАМНОГО больше записей. Иначе `Mutex` быстрее (тоньше
//lock-path). И ⚠ читатель, который ждёт, может заблокировать
// писателя в std (fairness) — в `parking_lot` это решено.
//
// `OnceLock<T>` — «вычисли один раз, лениво, потокобезопасно».
// Замена C++ magic statics (которые, кстати, в MSVC тоже thread-safe).
//
// `LazyLock<T>` — то же, но требует замыкание; есть с Rust 1.80.
//
// `thread_local!` — «своё значение на каждый поток». Главный
// инструмент для кешей, которые нельзя синхронизировать.

// ⚠ ЗАПУСК ПОТОКОВ ТОЛЬКО В ЭТОЙ ЧАСТИ — остальной модуль однопоточный,
//   чтобы вывод был читаемым.

fn part_8_3() {
    harness::part("8.3", "ПРИМЕР: Mutex, RwLock, OnceLock, thread_local");

    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::{Arc, Mutex, OnceLock, RwLock};

    // --- RwLock: пишем один раз, читаем много ------------------------------
    let cache: Arc<RwLock<Vec<(String, u32)>>> = Arc::new(RwLock::new(Vec::new()));
    {
        let mut w = cache.write().expect("не отравлен");
        w.push(("hero.mesh".into(), 1024));
    }
    let reads: Vec<u32> = (0..3)
        .map(|_| cache.read().expect("не отравлен")[0].1)
        .collect();
    println!("  RwLock: 3 параллельных чтения → {reads:?}");

    // --- Mutex: пишем много -------------------------------------------------
    let total: Arc<Mutex<u64>> = Arc::new(Mutex::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let t = Arc::clone(&total);
        handles.push(std::thread::spawn(move || {
            let mut g = t.lock().expect("не отравлен");
            *g += 100;
        }));
    }
    for h in handles {
        h.join().expect("поток не паникует");
    }
    println!("  Mutex: 4 потока × 100 = {}", *total.lock().expect("не отравлен"));

    // --- Atomics: когда Mutex избыточен -------------------------------------
    // Один счётчик — атомарная операция ДЕШЕВЕЕ блокировки.
    let hits = Arc::new(AtomicU32::new(0));
    let mut handles = Vec::new();
    for _ in 0..4 {
        let h = Arc::clone(&hits);
        handles.push(std::thread::spawn(move || {
            for _ in 0..1000 {
                h.fetch_add(1, Ordering::Relaxed); // ← тут гонки нет
            }
        }));
    }
    for h in handles {
        h.join().expect("поток не паникует");
    }
    println!("  AtomicU32: 4×1000 fetch_add(Relaxed) = {}", hits.load(Ordering::Relaxed));
    println!("    ⚠ Relaxed — «операция не переставляется», но не «видна сразу».");

    // --- OnceLock: ленивая инициализация ------------------------------------
    static INIT: OnceLock<Vec<&'static str>> = OnceLock::new();
    let first = INIT.get_or_init(|| {
        println!("  [OnceLock] инициализация (раз)");
        vec!["vulkan", "d3d12", "metal"]
    });
    let second = INIT.get_or_init(|| unreachable!("не вызовется"));
    println!("  OnceLock: {:?} / повторный get_or_init вернул то же: {:?}", first, second);
    assert!(std::ptr::eq(first, second));

    // --- thread_local -------------------------------------------------------
    std::thread_local! {
        static TID: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    }
    TID.with(|t| t.set(t.get() + 1));
    TID.with(|t| t.set(t.get() + 1));
    let v = TID.with(|t| t.get());
    println!("  thread_local: значение {v} (у каждого потока своё)");

    // --- PoisonError: паника при удержании блокировки -----------------------
    let poisoned: Arc<Mutex<u32>> = Arc::new(Mutex::new(7));
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {})); // глушим: паника здесь намеренная
    {
        let t = Arc::clone(&poisoned);
        let _ = std::thread::spawn(move || {
            let _g = t.lock().expect("берём блокировку");
            panic!("упали, удерживая блокировку");
        })
        .join(); // результат игнорируем: поток упал намеренно
    }
    std::panic::set_hook(prev);
    match poisoned.lock() {
        Ok(g) => println!("  блокировка не отравлена: {g} (не должно так быть)"),
        Err(poison) => {
            // Восстановление — осознанное решение программиста.
            let recovered = poison.into_inner();
            println!("  PoisonError: блокировка отравлена, но значение цело: {recovered}");
            println!("    Это ЛУЧШЕ, чем в C++: последы отравлены явно, а не дедлоком.");
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════════
//                              ЗАДАНИЯ
// ═════════════════════════════════════════════════════════════════════════════

/// ЧАСТЬ 8.4 — ЗАДАНИЕ 8.1
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Граф сцены. Родители — `Rc`, дети — `Vec<Rc<Node>>`, ссылка на
/// родителя — `Weak` (иначе цикл и утечка). Это ровно та структура,
/// которая в C++ всегда течёт, если забыть `weak_ptr`.
///
/// ```ignore
/// struct Node {
///     name: String,
///     children: RefCell<Vec<Rc<Node>>>,
///     parent: RefCell<Weak<Node>>,
/// }
///
/// fn new_node(name: &str) -> Rc<Node>;
/// fn add_child(parent: &Rc<Node>, child: Rc<Node>);   // двусторонняя связка!
/// fn depth(node: &Rc<Node>) -> usize;
/// fn find_path(node: &Rc<Node>, name: &str, path: &mut Vec<String>) -> bool;
/// fn total_nodes(node: &Rc<Node>) -> usize;
/// fn count_strong() -> usize;                          // для самопроверки
/// ```
///
/// ⚠ КЛЮЧЕВАЯ СЛОЖНОСТЬ: `add_child` должен установить И `parent`,
///   И `children`. Но `parent` — `Weak`, и мы уже внутри `&Rc<Node>`,
///   а `Rc::downgrade` требует `&Rc`. Чтобы не словить E0502, продумай
///   порядок вызовов `borrow_mut` — они не должны жить одновременно.
///
/// Требования:
///   * `depth` идёт по `parent` (вверх) и возвращает 0 у корня.
///   * `find_path` — обход вниз, записывает имена в `path` начиная с корня.
///   * `total_nodes` — количество узлов поддерева.
///   * Проверь в конце, что `Rc::strong_count` не растёт: все дети —
///     `Rc`, родители — `Weak`.
fn task_8_1() {
    struct Node {
        name: String,
        children: RefCell<Vec<Rc<Node>>>,
        parent: RefCell<Weak<Node>>,
    }

    fn new_node(name: &str) -> Rc<Node> {
        not_yet!("Rc::new с пустыми RefCell");
    }

    fn add_child(parent: &Rc<Node>, child: Rc<Node>) {
        not_yet!("Weak::from(parent) или Rc::downgrade(parent); ОСТОРОЖНО с двумя borrow_mut");
    }

    fn depth(node: &Rc<Node>) -> usize {
        not_yet!("иди вверх по parent.upgrade(), считай");
    }

    fn find_path(node: &Rc<Node>, name: &str, path: &mut Vec<String>) -> bool {
        not_yet!("рекурсия вниз; borrow() держи КОРОТКО, не на весь обход");
    }

    fn total_nodes(node: &Rc<Node>) -> usize {
        not_yet!("1 + сумма по детям");
    }

    let root = new_node("root");
    let a = new_node("a");
    let b = new_node("b");
    let c = new_node("c");

    // ⚠ add_child ЗАБИРАЕТ владение узлом (move). Чтобы `a` и `c`
    //   остались вscope, передаём копию счётчика — Rc::clone ДЕШЁВ:
    //   это +1 к счётчику, а НЕ копирование узла.
    add_child(&root, Rc::clone(&a));
    add_child(&root, Rc::clone(&b));
    add_child(&a, Rc::clone(&c));

    assert_eq!(depth(&root), 0);
    assert_eq!(depth(&a), 1);
    assert_eq!(depth(&c), 2);
    assert_eq!(total_nodes(&root), 4);

    let mut path = Vec::new();
    assert!(find_path(&root, "c", &mut path));
    assert_eq!(path, vec!["root", "a", "c"]);

    let mut path2 = Vec::new();
    assert!(!find_path(&root, "nope", &mut path2));
    assert!(path2.is_empty(), "путь не найден — не мусор в буфере");

    // Утечки нет: дети — Rc, родители — Weak
    assert_eq!(Rc::strong_count(&root), 1, "у корня нет сильных ссылок от детей");
}

/// ЧАСТЬ 8.5 — ЗАДАНИЕ 8.2
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Мини-ECS. Проблема, которую решает `RefCell`: «дай системе доступ
/// к набору сущностей, позволяя менять структуру», не ломая правила
/// заимствований.
///
/// ```ignore
/// struct World {
///     next_id: u32,
///     positions: RefCell<Vec<Option<(f32, f32)>>>,   // по индексу = id
///     velocities: RefCell<Vec<Option<(f32, f32)>>>,
///     names: RefCell<Vec<Option<String>>>,
/// }
///
/// impl World {
///     fn new() -> Self;
///     fn spawn(&mut self, name: &str) -> u32;                  // выдаёт id
///     fn despawn(&mut self, id: u32);
///     fn set_position(&self, id: u32, p: (f32, f32));          // &self!
///     fn position(&self, id: u32) -> Option<(f32, f32)>;
///     fn integrate(&self, dt: f32);                            // шаг физики
///     fn live_count(&self) -> usize;
///     fn ids_with_position(&self) -> Vec<u32>;
/// }
/// ```
///
/// ТРЕТЬЕ ПРАВИЛО: `set_position(&self)` принимает `&self`, а не `&mut self` —
///   потому что меняется содержимое RefCell, а не сама структура. Это
///   interior mutability. Без неё `&self` был бы невозможен.
///
/// ГЛАВНАЯ ЛОВУШКА: в `integrate` нельзя держать `borrow()` позиций
///   и одновременно писать в скорости. Нужен `mem::take` или
///   `Vec<(id, v)>` для результата. Разберись и напиши без panic.
///
/// Требования:
///   * `ids_with_position` возвращает id **по возрастанию** (сортировка).
///   * `despawn` оставляет `None` в слотах (дырка), а не сдвигает
///     всё влево — иначе id «поедут».
fn task_8_2() {
    struct World {
        next_id: u32,
        positions: RefCell<Vec<Option<(f32, f32)>>>,
        velocities: RefCell<Vec<Option<(f32, f32)>>>,
        names: RefCell<Vec<Option<String>>>,
    }

    impl World {
        fn new() -> Self {
            not_yet!("пустые буферы, next_id = 0");
        }

        fn spawn(&mut self, name: &str) -> u32 {
            not_yet!("запиши в три слота, верни id");
        }

        fn despawn(&mut self, id: u32) {
            not_yet!("поставь None в три слота (не сдвигай!)");
        }

        fn set_position(&self, id: u32, p: (f32, f32)) {
            not_yet!("&self + RefCell — interior mutability");
        }

        fn position(&self, id: u32) -> Option<(f32, f32)> {
            not_yet!("borrow().get(id).copied().flatten()");
        }

        fn integrate(&self, dt: f32) {
            not_yet!("mem::take позиций, посчитай, верни обратно — иначе два borrow_mut");
        }

        fn live_count(&self) -> usize {
            not_yet!("count позиций == Some");
        }

        fn ids_with_position(&self) -> Vec<u32> {
            not_yet!("enumerate + filter_map + collect (порядок и так возрастающий)");
        }
    }

    let mut w = World::new();
    let e0 = w.spawn("hero");
    let e1 = w.spawn("enemy");
    let e2 = w.spawn("coin");

    assert_eq!((e0, e1, e2), (0, 1, 2));
    assert_eq!(w.live_count(), 3);

    w.set_position(e0, (1.0, 2.0));
    w.set_position(e1, (3.0, 4.0));
    assert_eq!(w.position(e0), Some((1.0, 2.0)));
    assert_eq!(w.position(e2), None, "у coin нет позиции");
    assert_eq!(w.ids_with_position(), vec![0, 1]);

    w.despawn(e1);
    assert_eq!(w.live_count(), 2);
    assert_eq!(w.position(e1), None);
    assert_eq!(w.position(e2), None, "id не «поехал»: e2 всё ещё index 2");

    w.integrate(0.5);
    println!("  ECS-lite работает: {} живых, интеграция без паники", w.live_count());
}

/// ЧАСТЬ 8.6 — ЗАДАНИЕ 8.3
/// ─────────────────────────────────────────────────────────────────────────────
///
/// Кэш ресурсов, безопасный для потоков. Здесь `RefCell` уже не годится:
/// он не `Sync`, и компилятор не даст отдать `&` другому потоку.
///
/// ```ignore
/// struct AssetCache {
///     entries: RwLock<HashMap<String, Arc<Vec<u8>>>>,
///     stats: Mutex<CacheStats>,
///     name: OnceLock<String>,
/// }
///
/// #[derive(Default)]
/// struct CacheStats { hits: u64, misses: u64 }
///
/// impl AssetCache {
///     fn new() -> Self;
///     fn get_or_load(&self, name: &str) -> Arc<Vec<u8>>;   // дедлок-ловушка внутри!
///     fn hits(&self) -> u64;
///     fn misses(&self) -> u64;
///     fn cache_name(&self) -> &str;                          // OnceLock
/// }
/// ```
///
/// ⚠ ГЛАВНАЯ ЛОВУШКА ЗАДАНИЯ: нельзя держать `read()` на карте и внутри
///   брать `write()` на той же карте — на `RwLock` в std это
///   DEADLOCK (writer ждёт читателя, а читатель ждёт сам себя).
///   Правильный порядок: сначала `read` + `clone()` замыкание,
///   отпустить, потом `write` и вставить.
///
/// Требования:
///   * `get_or_load` возвращает `Arc`, чтобы копии не было.
///   * Счётчики — через `Mutex`, обновляются в правильном порядке
///     относительно карты (сначала карта, потом статистика — иначе
///     счётчик разъедется с реальностью).
///   * `cache_name` — `OnceLock::get_or_init`.
///   * Реализуй `fn prefetch(&self, names: &[&str])` — загрузить пачкой
///     (одна write-блокировка на всю пачку, а не по одной на файл).
///   * Ответь в комментарии: почему `get_or_load` в `&self`, а не `&mut self`?
fn task_8_3() {
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex, OnceLock, RwLock};

    #[derive(Default, Debug)]
    struct CacheStats {
        hits: u64,
        misses: u64,
    }

    struct AssetCache {
        entries: RwLock<HashMap<String, Arc<Vec<u8>>>>,
        stats: Mutex<CacheStats>,
        name: OnceLock<String>,
    }

    impl AssetCache {
        fn new() -> Self {
            not_yet!("все три поля пустые, OnceLock::new()");
        }

        fn get_or_load(&self, name: &str) -> Arc<Vec<u8>> {
            not_yet!("ВНИМАНИЕ: сначала read+clone, ОТПУСТИТЬ, потом write. Иначе deadlock");
        }

        fn prefetch(&self, names: &[&str]) {
            not_yet!("одна write-блокировка на всю пачку");
        }

        fn hits(&self) -> u64 {
            not_yet!("stats.lock().hits");
        }

        fn misses(&self) -> u64 {
            not_yet!("stats.lock().misses");
        }

        fn cache_name(&self) -> &str {
            not_yet!("name.get_or_init(|| \"asset-cache\")");
        }
    }

    // Реальный параллельный тест: если в get_or_load есть дедлок,
    // программа ВИСНЕТ. Сделай так, чтобы падение было заметно.
    let cache = Arc::new(AssetCache::new());
    let mut handles = Vec::new();
    for t in 0..4u32 {
        let c = Arc::clone(&cache);
        handles.push(std::thread::spawn(move || {
            for i in 0..50 {
                let _ = c.get_or_load(&format!("asset_{}", (t + i) % 8));
            }
        }));
    }
    for h in handles {
        h.join().expect("поток не паникует");
    }
    let total = cache.hits() + cache.misses();
    assert_eq!(total, 200, "каждый вызов учтён");
    assert!(cache.misses() > 0);
    assert_eq!(cache.cache_name(), "asset-cache");

    let prefetched = Arc::clone(&cache);
    std::thread::spawn(move || {
        prefetched.prefetch(&["a", "b", "c"]);
    })
    .join()
    .expect("prefetch без дедлока");

    let h = cache.hits();
    let m = cache.misses();
    println!("  cache: hits={h} misses={m}, имя = {:?}", cache.cache_name());
}
